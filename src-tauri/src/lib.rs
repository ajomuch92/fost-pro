use fost_lang::Interp;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};

#[derive(serde::Serialize)]
struct RunResult {
    output: String,
    error: Option<String>,
    /// Line of the error when it is in the code that was sent, not in another file.
    line: Option<usize>,
    /// CLEAR ran: the window should wipe what it has shown so far.
    clear: bool,
}

/// Code to run, the directory where DO looks for programs, and where to send the result.
type Job = (String, Option<String>, mpsc::Sender<RunResult>);

/// Handle to the interpreter, which lives on its own thread because it is not `Send`
/// and a running program must not block the UI.
struct Engine {
    jobs: Mutex<mpsc::Sender<Job>>,
    stop: Arc<AtomicBool>,
}

#[tauri::command]
async fn run_code(code: String, dir: Option<String>, engine: tauri::State<'_, Engine>) -> Result<RunResult, String> {
    let (reply, result) = mpsc::channel();
    engine.jobs.lock().unwrap().send((code, dir, reply)).map_err(|e| e.to_string())?;
    let result = tauri::async_runtime::spawn_blocking(move || result.recv()).await;
    result.map_err(|e| e.to_string())?.map_err(|e| e.to_string())
}

#[tauri::command]
fn stop_code(engine: tauri::State<'_, Engine>) {
    engine.stop.store(true, Ordering::Relaxed);
}

// The editor opens and saves wherever the user points the file dialog,
// so these take any path instead of going through a scoped fs plugin.
#[tauri::command]
fn read_file(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| format!("No se pudo abrir {path}: {e}"))
}

#[tauri::command]
fn write_file(path: String, text: String) -> Result<(), String> {
    std::fs::write(&path, text).map_err(|e| format!("No se pudo guardar {path}: {e}"))
}

fn start_engine() -> Engine {
    let (jobs, queue) = mpsc::channel::<Job>();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    std::thread::Builder::new()
        .name("fost-interp".into())
        .stack_size(fost_lang::STACK_SIZE)
        .spawn(move || {
            let mut interp = Interp::default();
            interp.stop = flag;
            for (code, dir, reply) in queue {
                if let Some(dir) = dir {
                    interp.dir = dir.into();
                }
                let error = interp.run(&code).err();
                let result = RunResult {
                    output: interp.take_output(),
                    line: error.as_ref().filter(|e| e.file.is_none()).map(|e| e.line),
                    error: error.map(|e| e.to_string()),
                    clear: std::mem::take(&mut interp.clear),
                };
                let _ = reply.send(result);
            }
        })
        .expect("failed to start the interpreter thread");
    Engine { jobs: Mutex::new(jobs), stop }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(start_engine())
        .invoke_handler(tauri::generate_handler![run_code, stop_code, read_file, write_file])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_keeps_state_between_jobs() {
        let engine = start_engine();
        let run = |code: &str| {
            let (reply, result) = mpsc::channel();
            engine.jobs.lock().unwrap().send((code.into(), None, reply)).unwrap();
            result.recv().unwrap()
        };
        assert!(run("x = 20").error.is_none());
        assert_eq!(run("? x * 2").output, "\n40");
        let failed = run("x = 1\n? nope");
        assert_eq!(failed.error.as_deref(), Some("Línea 2: Variable no encontrada: NOPE"));
        assert_eq!(failed.line, Some(2));
        assert!(run("CLEAR").clear);
    }
}
