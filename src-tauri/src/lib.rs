use fost_lang::{Host, Interp};
use serde_json::Value as Json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use tauri::{Emitter, Manager};
use tauri_plugin_opener::OpenerExt;
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
    /// The menus on screen, to draw.
    menus: Json,
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
    /// The user chose a pad of `menu`, or the bar `(popup, number)` reached through it.
    MenuEvent { menu: String, pad: String, bar: Option<(String, String)> },
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
async fn menu_event(menu: String, pad: String, popup: Option<String>, bar: Option<String>, engine: EngineState<'_>) -> Result<RunResult, String> {
    engine.request(Request::MenuEvent { menu, pad, bar: popup.zip(bar) }, None).await
}

#[tauri::command]
fn stop_code(engine: EngineState<'_>) {
    engine.stop.store(true, Ordering::Relaxed);
}

#[tauri::command]
fn class_info() -> Json {
    fost_lang::class_info()
}

/// What the project panel lists for a folder.
#[tauri::command]
fn project_info(dir: String) -> Json {
    fost_lang::project(std::path::Path::new(&dir))
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

/// The bytes of a picture an Image control shows. A relative path is looked up in `dir`.
#[tauri::command]
fn read_binary(path: String, dir: Option<String>) -> Result<tauri::ipc::Response, String> {
    let full = std::path::Path::new(dir.as_deref().unwrap_or(".")).join(&path);
    std::fs::read(full).map(tauri::ipc::Response::new).map_err(|e| format!("No se pudo abrir {path}: {e}"))
}

/// Copies a file the user picked into a project, creating the folder it goes to.
/// A file already there is left alone: it may be a different picture with the same name.
#[tauri::command]
fn copy_file(from: String, to: String) -> Result<(), String> {
    let target = std::path::Path::new(&to);
    if target.exists() {
        return Err(format!("Ya existe {to}"));
    }
    let copy = || {
        std::fs::create_dir_all(target.parent().unwrap_or(std::path::Path::new(".")))?;
        std::fs::copy(&from, target)
    };
    copy().map(|_| ()).map_err(|e| format!("No se pudo copiar {from}: {e}"))
}

/// Renames a file of a project. One already called that is left alone.
#[tauri::command]
fn rename_file(from: String, to: String) -> Result<(), String> {
    if std::path::Path::new(&to).exists() {
        return Err(format!("Ya existe {to}"));
    }
    std::fs::rename(&from, &to).map_err(|e| format!("No se pudo renombrar {from}: {e}"))
}

/// Deletes a file of a project, which the user has confirmed.
#[tauri::command]
fn delete_file(path: String) -> Result<(), String> {
    std::fs::remove_file(&path).map_err(|e| format!("No se pudo eliminar {path}: {e}"))
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

    fn open_url(&mut self, url: &str) -> bool {
        // a failure to launch the browser is not the program's error to handle
        let _ = self.app.opener().open_url(url, None::<&str>);
        true
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
                    Request::MenuEvent { menu, pad, bar } => {
                        interp.menu_event(&menu, &pad, bar.as_ref().map(|(popup, number)| (popup.as_str(), number.as_str())))
                    }
                }
                .err();
                let open = interp.open.take();
                let result = RunResult {
                    output: interp.take_output(),
                    line: error.as_ref().filter(|e| e.file.is_none()).map(|e| e.line),
                    error: error.map(|e| e.to_string()),
                    clear: std::mem::take(&mut interp.clear),
                    forms: interp.forms_json(),
                    menus: interp.menus_json(),
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
            run_code, run_form, ui_event, menu_event, stop_code, answer_prompt, class_info, reference, project_info, read_file, read_binary, write_file, copy_file, rename_file, delete_file
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_file_makes_the_folder_and_never_overwrites() {
        let dir = tempfile_dir("copy");
        let path = |name: &str| dir.join(name).to_string_lossy().into_owned();
        std::fs::write(path("a.png"), "uno").unwrap();
        assert_eq!(copy_file(path("a.png"), path("assets/a.png")), Ok(()));
        std::fs::write(path("a.png"), "dos").unwrap();
        assert!(copy_file(path("a.png"), path("assets/a.png")).unwrap_err().starts_with("Ya existe"));
        assert_eq!(std::fs::read_to_string(path("assets/a.png")).unwrap(), "uno");
        assert!(copy_file(path("nadie.png"), path("assets/b.png")).unwrap_err().starts_with("No se pudo copiar"));
    }

    #[test]
    fn rename_file_never_overwrites_and_delete_file_removes_one_file() {
        let dir = tempfile_dir("rename");
        let path = |name: &str| dir.join(name).to_string_lossy().into_owned();
        std::fs::write(path("a.fost"), "a").unwrap();
        std::fs::write(path("b.fost"), "b").unwrap();
        assert!(rename_file(path("a.fost"), path("b.fost")).unwrap_err().starts_with("Ya existe"));
        assert_eq!(rename_file(path("a.fost"), path("c.fost")), Ok(()));
        assert_eq!(std::fs::read_to_string(path("c.fost")).unwrap(), "a");
        assert_eq!(delete_file(path("c.fost")), Ok(()));
        assert!(delete_file(path("c.fost")).unwrap_err().starts_with("No se pudo eliminar"));
        // a folder is not a file to delete
        assert!(delete_file(dir.to_string_lossy().into_owned()).is_err());
        assert_eq!(std::fs::read_to_string(path("b.fost")).unwrap(), "b");
    }

    /// A folder of its own for a test, new on every run so nothing has to be cleaned up first.
    fn tempfile_dir(tag: &str) -> std::path::PathBuf {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("fost-{tag}-{stamp}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

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

        let defined = run("DEFINE PAD hola OF _MSYSMENU PROMPT 'Hola'\nON SELECTION PAD hola OF _MSYSMENU ? PROMPT()");
        assert_eq!(defined.menus[0]["items"][0]["prompt"], "Hola");
        let chosen = send(Request::MenuEvent { menu: "_MSYSMENU".into(), pad: "HOLA".into(), bar: None });
        assert_eq!(chosen.output, "\nHola");
    }
}
