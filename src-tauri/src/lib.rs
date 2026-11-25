use fost_lang::Interp;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};

#[derive(serde::Serialize)]
struct RunResult {
    output: String,
    error: Option<String>,
    /// CLEAR ran: the window should wipe what it has shown so far.
    clear: bool,
}

type Job = (String, mpsc::Sender<RunResult>);

/// Handle to the interpreter, which lives on its own thread because it is not `Send`
/// and a running program must not block the UI.
struct Engine {
    jobs: Mutex<mpsc::Sender<Job>>,
    stop: Arc<AtomicBool>,
}

#[tauri::command]
async fn run_code(code: String, engine: tauri::State<'_, Engine>) -> Result<RunResult, String> {
    let (reply, result) = mpsc::channel();
    engine.jobs.lock().unwrap().send((code, reply)).map_err(|e| e.to_string())?;
    let result = tauri::async_runtime::spawn_blocking(move || result.recv()).await;
    result.map_err(|e| e.to_string())?.map_err(|e| e.to_string())
}

#[tauri::command]
fn stop_code(engine: tauri::State<'_, Engine>) {
    engine.stop.store(true, Ordering::Relaxed);
}

fn start_engine() -> Engine {
    let (jobs, queue) = mpsc::channel::<Job>();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    std::thread::Builder::new()
        .name("fost-interp".into())
        // the interpreter recurses once per nested call, up to 128 levels deep
        .stack_size(64 << 20)
        .spawn(move || {
            let mut interp = Interp::default();
            interp.stop = flag;
            for (code, reply) in queue {
                let error = interp.run(&code).err().map(|e| e.to_string());
                let result = RunResult { output: interp.take_output(), error, clear: std::mem::take(&mut interp.clear) };
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
        .manage(start_engine())
        .invoke_handler(tauri::generate_handler![run_code, stop_code])
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
            engine.jobs.lock().unwrap().send((code.into(), reply)).unwrap();
            result.recv().unwrap()
        };
        assert!(run("x = 20").error.is_none());
        assert_eq!(run("? x * 2").output, "\n40");
        assert_eq!(run("? nope").error.as_deref(), Some("Línea 1: Variable no encontrada: NOPE"));
        assert!(run("CLEAR").clear);
    }
}
