use fost_lang::{Host, Interp};
use serde_json::Value as Json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use tauri::{Emitter, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind, MessageDialogResult};

/// What the UI gets back from every request to the interpreter.
#[derive(serde::Serialize)]
struct RunResult {
    output: String,
    error: Option<String>,
    /// Line of the error when it is in the code that was sent, not in another file.
    line: Option<usize>,
    /// CLEAR ran: the window should wipe what it has shown so far.
    clear: bool,
    /// The visible forms, to draw.
    forms: Json,
    /// Id of the control that asked for the focus.
    focus: Option<u64>,
    /// A file MODIFY COMMAND, MODIFY FORM or CREATE FORM asked to open.
    open: Option<OpenRequest>,
    /// HELP ran: show the reference.
    help: bool,
}

#[derive(serde::Serialize)]
struct OpenRequest {
    form: bool,
    path: Option<String>,
}

enum Request {
    Run { code: String },
    /// The designer's Run: a form that may not be saved yet.
    RunForm { json: String, name: String },
    UiEvent { id: u64, props: Json, event: Option<String> },
}

/// A request, the directory where DO looks for files, and where to send the result.
type Job = (Request, Option<String>, mpsc::Sender<RunResult>);

/// Handle to the interpreter, which lives on its own thread because it is not `Send`
/// and a running program must not block the UI.
struct Engine {
    jobs: Mutex<mpsc::Sender<Job>>,
    stop: Arc<AtomicBool>,
}

impl Engine {
    async fn request(&self, request: Request, dir: Option<String>) -> Result<RunResult, String> {
        let (reply, result) = mpsc::channel();
        self.jobs.lock().unwrap().send((request, dir, reply)).map_err(|e| e.to_string())?;
        let result = tauri::async_runtime::spawn_blocking(move || result.recv()).await;
        result.map_err(|e| e.to_string())?.map_err(|e| e.to_string())
    }
}

type EngineState<'a> = tauri::State<'a, Engine>;

#[tauri::command]
async fn run_code(code: String, dir: Option<String>, engine: EngineState<'_>) -> Result<RunResult, String> {
    engine.request(Request::Run { code }, dir).await
}

#[tauri::command]
async fn run_form(json: String, name: String, dir: Option<String>, engine: EngineState<'_>) -> Result<RunResult, String> {
    engine.request(Request::RunForm { json, name }, dir).await
}

#[tauri::command]
async fn ui_event(id: u64, props: Json, event: Option<String>, engine: EngineState<'_>) -> Result<RunResult, String> {
    engine.request(Request::UiEvent { id, props, event }, None).await
}

#[tauri::command]
fn stop_code(engine: EngineState<'_>) {
    engine.stop.store(true, Ordering::Relaxed);
}

#[tauri::command]
fn class_info() -> Json {
    fost_lang::class_info()
}

#[tauri::command]
fn reference() -> Json {
    fost_lang::reference()
}

/// Where the UI sends the answer to an INPUTBOX() the interpreter is waiting on. None: cancelled.
struct Prompts(Mutex<mpsc::Sender<Option<String>>>);

#[tauri::command]
fn answer_prompt(value: Option<String>, prompts: tauri::State<'_, Prompts>) {
    let _ = prompts.0.lock().unwrap().send(value);
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

/// The dialogs a program can open. They run on the interpreter thread, so blocking is fine.
struct Dialogs {
    app: tauri::AppHandle,
    answers: mpsc::Receiver<Option<String>>,
}

impl Host for Dialogs {
    fn message_box(&mut self, text: &str, buttons: i64, title: &str) -> Option<i64> {
        // FoxPro packs the buttons in the low 4 bits and the icon in the next 4
        let (set, yes_no) = match buttons & 15 {
            1 | 5 => (MessageDialogButtons::OkCancel, false),
            3 => (MessageDialogButtons::YesNoCancel, false),
            4 => (MessageDialogButtons::YesNo, true),
            // shortcut: Abort/Retry/Ignore (2) shows a plain OK, upgrade if a program needs the three answers
            _ => (MessageDialogButtons::Ok, false),
        };
        let kind = match buttons & 240 {
            16 => MessageDialogKind::Error,
            48 => MessageDialogKind::Warning,
            _ => MessageDialogKind::Info,
        };
        let title = if title.is_empty() { "Fost Pro" } else { title };
        let answer = self.app.dialog().message(text).title(title).kind(kind).buttons(set).blocking_show_with_result();
        Some(match answer {
            MessageDialogResult::Yes => 6,
            MessageDialogResult::No => 7,
            MessageDialogResult::Ok => 1,
            // closing a Yes/No dialog has no Cancel to map to: it counts as No
            _ if yes_no => 7,
            _ => 2,
        })
    }

    /// INPUTBOX(): the UI draws the question and answers through `answer_prompt`.
    fn input_box(&mut self, prompt: &str, title: &str, default: &str) -> Option<String> {
        // an answer nobody was waiting for must not be taken as this one
        while self.answers.try_recv().is_ok() {}
        let question = serde_json::json!({ "prompt": prompt, "title": title, "default": default });
        self.app.emit("fost-prompt", question).ok()?;
        Some(self.answers.recv().ok()?.unwrap_or_default())
    }
}

fn start_engine(host: Box<dyn Host + Send>) -> Engine {
    let (jobs, queue) = mpsc::channel::<Job>();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    std::thread::Builder::new()
        .name("fost-interp".into())
        .stack_size(fost_lang::STACK_SIZE)
        .spawn(move || {
            let mut interp = Interp::default();
            interp.stop = flag;
            interp.host = host;
            for (request, dir, reply) in queue {
                if let Some(dir) = dir {
                    interp.dir = dir.into();
                }
                let error = match request {
                    Request::Run { code } => interp.run(&code),
                    Request::RunForm { json, name } => interp.run_form(&json, &name),
                    Request::UiEvent { id, props, event } => interp.ui_event(id, &props, event.as_deref()),
                }
                .err();
                let open = interp.open.take();
                let result = RunResult {
                    output: interp.take_output(),
                    line: error.as_ref().filter(|e| e.file.is_none()).map(|e| e.line),
                    error: error.map(|e| e.to_string()),
                    clear: std::mem::take(&mut interp.clear),
                    forms: interp.forms_json(),
                    focus: interp.focus.take(),
                    help: std::mem::take(&mut interp.help),
                    open: open.map(|o| OpenRequest { form: o.form, path: o.path.map(|p| p.to_string_lossy().into_owned()) }),
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
        .setup(|app| {
            let (answer, answers) = mpsc::channel();
            app.manage(Prompts(Mutex::new(answer)));
            app.manage(start_engine(Box::new(Dialogs { app: app.handle().clone(), answers })));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            run_code, run_form, ui_event, stop_code, answer_prompt, class_info, reference, read_file, write_file
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_keeps_state_between_jobs() {
        let engine = start_engine(Box::new(()));
        let send = |request: Request| {
            let (reply, result) = mpsc::channel();
            engine.jobs.lock().unwrap().send((request, None, reply)).unwrap();
            result.recv().unwrap()
        };
        let run = |code: &str| send(Request::Run { code: code.into() });
        assert!(run("x = 20").error.is_none());
        assert_eq!(run("? x * 2").output, "\n40");
        let failed = run("x = 1\n? nope");
        assert_eq!(failed.error.as_deref(), Some("Línea 2: Variable no encontrada: NOPE"));
        assert_eq!(failed.line, Some(2));
        assert!(run("CLEAR").clear);
        assert_eq!(run("MODIFY COMMAND").open.map(|o| (o.form, o.path)), Some((false, None)));

        // a form opened by one job is drawn, and reacts to an event sent by a later one
        let form = r#"{ "controls": [{ "class": "CommandButton", "props": { "Name": "b" }, "methods": { "Click": "THISFORM.Caption = 'pulsado'" } }] }"#;
        let shown = send(Request::RunForm { json: form.into(), name: "f".into() });
        assert_eq!(shown.error, None);
        let id = shown.forms[0]["children"][0]["id"].as_u64().unwrap();
        let clicked = send(Request::UiEvent { id, props: Json::Null, event: Some("Click".into()) });
        assert_eq!(clicked.forms[0]["props"]["CAPTION"], "pulsado");
    }
}
