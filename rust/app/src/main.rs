//! Pete's PDF to MD desktop app on Tauri.
//!
//! The window is the web page in `app/`; `app/tauri-bridge.js` gives it
//! `window.pdfToMdApi`, which calls these commands. (They began as ports of the
//! Electron main process, now in `archive/electron-python/electron/main.cjs`, and
//! keep its names and return shapes.) Conversion runs the converter library
//! in-process, so the app needs neither Python nor a helper program.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use petes_pdf_core::{convert_with_progress, output::Mode, ConvertOptions};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

/// Version with the build revision letter (0.9.0g), from package.json via build.rs.
const DISPLAY_VERSION: &str = env!("PDF_TO_MD_VERSION");
const DEBUG_BUILD: &str = env!("PDF_TO_MD_DEBUG_BUILD");
const PROGRESS_EVENT: &str = "conversion-progress";
const ALLOWED_EXTERNAL_URLS: [&str; 3] = [
    "https://github.com/pbeens/Petes-PDF-to-MD/issues",
    "https://github.com/pbeens/Petes-PDF-to-MD",
    "https://buymeacoffee.com/pbeens",
];

struct AppState {
    started_at: String,
}

/// Debug builds, or PDF_TO_MD_DEBUG=1, write each version's output to its own
/// folder so earlier results stay available for comparison.
fn is_debug() -> bool {
    DEBUG_BUILD == "1" || std::env::var("PDF_TO_MD_DEBUG").as_deref() == Ok("1")
}

fn window_title() -> String {
    format!("Pete's PDF to MD v{DISPLAY_VERSION}{}", if is_debug() { " (debug version)" } else { "" })
}

fn default_output_root<R: Runtime>(app: &AppHandle<R>) -> PathBuf {
    let documents = app.path().document_dir().unwrap_or_else(|_| app.path().home_dir().unwrap_or_default());
    documents.join("Pete's PDF to MD Output")
}

fn output_root<R: Runtime>(app: &AppHandle<R>, given: Option<&str>) -> PathBuf {
    match given.map(str::trim).filter(|s| !s.is_empty()) {
        Some(path) => PathBuf::from(path),
        None => default_output_root(app),
    }
}

/// `<output root>/<pdf name>`, plus the version folder in debug builds.
fn output_dir_for<R: Runtime>(app: &AppHandle<R>, input: &str, root: Option<&str>) -> PathBuf {
    let name = Path::new(input).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let dir = output_root(app, root).join(name);
    if is_debug() {
        dir.join(format!("v{DISPLAY_VERSION}"))
    } else {
        dir
    }
}

fn validate_pdf(candidate: &str) -> Value {
    let raw = candidate.trim();
    if raw.is_empty() {
        return json!({ "ok": false, "message": "Input PDF path is missing." });
    }
    let path = PathBuf::from(raw);
    if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")) {
        return json!({ "ok": false, "message": "Please select a .pdf file." });
    }
    if !path.exists() {
        return json!({ "ok": false, "message": "The selected PDF file could not be found on disk." });
    }
    json!({ "ok": true, "filePath": path.to_string_lossy() })
}

/// A dropped or pasted path: plain, quoted, or a file:// URL.
fn parse_dropped_path(raw: &str) -> String {
    let value = raw.trim().trim_matches(|c| c == '"' || c == '\'').trim();
    if let Some(rest) = value.strip_prefix("file://") {
        let path = rest.strip_prefix("localhost").unwrap_or(rest);
        return percent_decode(path);
    }
    value.to_string()
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&text[i + 1..i + 3], 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn section_full_path<R: Runtime>(app: &AppHandle<R>, input: &str, root: Option<&str>, relative: &str) -> Result<PathBuf, String> {
    let dir = output_dir_for(app, input, root);
    let full = dir.join(relative);
    let (Ok(dir), Ok(full)) = (dir.canonicalize(), full.canonicalize()) else {
        return Err("Section file not found.".into());
    };
    if !full.starts_with(&dir) {
        return Err("Invalid section path.".into());
    }
    Ok(full)
}

/// outline.json (with links to missing files removed) and outline.md for a converted PDF.
fn load_outline_payload<R: Runtime>(app: &AppHandle<R>, input: &str, root: Option<&str>) -> Result<Value, String> {
    let dir = output_dir_for(app, input, root);
    let outline_json = dir.join("outline.json");
    let raw = std::fs::read_to_string(&outline_json).map_err(|_| "outline.json not found. Run conversion first.".to_string())?;
    let mut items: Value = serde_json::from_str(&raw).map_err(|e| format!("outline.json could not be read: {e}"))?;
    if let Some(list) = items.as_array_mut() {
        for item in list.iter_mut() {
            let relative = item["section_file"].as_str().unwrap_or("").to_string();
            if !relative.is_empty() && !dir.join(&relative).exists() {
                item["section_file"] = Value::Null;
            }
        }
    }
    let outline_markdown = std::fs::read_to_string(dir.join("outline.md")).unwrap_or_default();
    Ok(json!({ "outputDir": dir.to_string_lossy(), "outlineMarkdown": outline_markdown, "outlineItems": items }))
}

#[tauri::command]
async fn pick_pdf<R: Runtime>(app: AppHandle<R>) -> Value {
    match app.dialog().file().set_title("Select PDF").add_filter("PDF", &["pdf"]).blocking_pick_file() {
        Some(file) => match file.into_path() {
            Ok(path) => json!({ "canceled": false, "filePath": path.to_string_lossy() }),
            Err(_) => json!({ "canceled": true }),
        },
        None => json!({ "canceled": true }),
    }
}

#[tauri::command]
fn validate_input_pdf(candidate_path: String) -> Value {
    validate_pdf(&candidate_path)
}

#[tauri::command]
fn resolve_dropped_input_pdf(payload: Value) -> Value {
    let mut candidates: Vec<String> = Vec::new();
    if let Some(files) = payload["files"].as_array() {
        candidates.extend(files.iter().filter_map(|f| f["path"].as_str()).map(parse_dropped_path));
    }
    if let Some(texts) = payload["textByType"].as_object() {
        for text in texts.values().filter_map(Value::as_str) {
            candidates.extend(text.lines().map(parse_dropped_path));
        }
    }
    let mut saw_pdf = false;
    for candidate in candidates.iter().filter(|c| !c.is_empty()) {
        saw_pdf |= candidate.to_lowercase().ends_with(".pdf");
        let check = validate_pdf(candidate);
        if check["ok"] == true {
            return check;
        }
    }
    let message = if saw_pdf { "The selected PDF file could not be found on disk." } else { "Please drop a .pdf file." };
    json!({ "ok": false, "message": message })
}

#[tauri::command]
async fn pick_output_dir<R: Runtime>(app: AppHandle<R>, current_path: Option<String>) -> Value {
    let start = output_root(&app, current_path.as_deref());
    match app.dialog().file().set_title("Select Output Folder").set_directory(start).blocking_pick_folder() {
        Some(dir) => match dir.into_path() {
            Ok(path) => json!({ "canceled": false, "dirPath": path.to_string_lossy() }),
            Err(_) => json!({ "canceled": true }),
        },
        None => json!({ "canceled": true }),
    }
}

#[tauri::command]
fn get_default_output_root<R: Runtime>(app: AppHandle<R>) -> Value {
    let root = default_output_root(&app);
    let _ = std::fs::create_dir_all(&root);
    json!({ "outputRoot": root.to_string_lossy() })
}

#[tauri::command]
fn get_app_meta(state: State<AppState>) -> Value {
    json!({
        "version": DISPLAY_VERSION,
        "startedAt": state.started_at,
        "isPackaged": !cfg!(debug_assertions),
        "title": window_title(),
        "debug": is_debug(),
    })
}

#[tauri::command]
async fn run_conversion<R: Runtime>(app: AppHandle<R>, input_pdf_path: String, output_root_path: Option<String>, options: Option<Value>) -> Result<Value, String> {
    let checked = validate_pdf(&input_pdf_path);
    let Some(input) = checked["filePath"].as_str().map(str::to_string) else {
        return Err(checked["message"].as_str().unwrap_or("Invalid input PDF.").to_string());
    };
    let options = options.unwrap_or_default();
    let mode = options["outputMode"].as_str().and_then(Mode::parse).unwrap_or(Mode::Sections);
    let include_section_metadata = options["includeSectionMetadata"].as_bool().unwrap_or(true);
    let root = output_root(&app, output_root_path.as_deref());
    std::fs::create_dir_all(&root).map_err(|e| format!("Could not create the output folder: {e}"))?;

    let convert_opts = ConvertOptions {
        out_dir: root,
        out_subdir: is_debug().then(|| format!("v{DISPLAY_VERSION}")),
        mode,
        include_section_metadata,
    };
    let emitter = app.clone();
    let input_for_task = input.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let progress = |message: &str| {
            let _ = emitter.emit(PROGRESS_EVENT, json!({ "message": message }));
        };
        progress("Starting conversion");
        convert_with_progress(Path::new(&input_for_task), &convert_opts, &progress)
    })
    .await
    .map_err(|e| format!("Conversion failed: {e}"))?
    .map_err(|e| format!("Conversion failed: {e}"))?;

    let _ = app.emit(PROGRESS_EVENT, json!({ "message": "Loading outline and sections metadata" }));
    let mut payload = load_outline_payload(&app, &input, output_root_path.as_deref())?;
    payload["warnings"] = json!(result.warnings);
    let _ = app.emit(PROGRESS_EVENT, json!({ "message": "Conversion complete" }));
    Ok(payload)
}

#[tauri::command]
fn open_external_url<R: Runtime>(app: AppHandle<R>, url: String) -> Result<Value, String> {
    let url = url.trim();
    if !ALLOWED_EXTERNAL_URLS.contains(&url) {
        return Err("Unsupported external URL.".into());
    }
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())?;
    Ok(json!({ "ok": true }))
}

#[tauri::command]
fn load_outline<R: Runtime>(app: AppHandle<R>, input_pdf_path: String, output_root_path: Option<String>) -> Result<Value, String> {
    if input_pdf_path.trim().is_empty() || !Path::new(&input_pdf_path).exists() {
        return Err("Input PDF path is missing or invalid.".into());
    }
    load_outline_payload(&app, &input_pdf_path, output_root_path.as_deref())
}

#[tauri::command]
fn load_section<R: Runtime>(app: AppHandle<R>, input_pdf_path: String, output_root_path: Option<String>, section_relative_path: String) -> Result<Value, String> {
    let full = section_full_path(&app, &input_pdf_path, output_root_path.as_deref(), &section_relative_path)?;
    let content = std::fs::read_to_string(&full).map_err(|e| e.to_string())?;
    Ok(json!({ "path": full.to_string_lossy(), "content": content }))
}

#[tauri::command]
fn open_section_in_folder<R: Runtime>(app: AppHandle<R>, input_pdf_path: String, output_root_path: Option<String>, section_relative_path: String) -> Result<Value, String> {
    let full = section_full_path(&app, &input_pdf_path, output_root_path.as_deref(), &section_relative_path)?;
    app.opener().reveal_item_in_dir(&full).map_err(|e| e.to_string())?;
    Ok(json!({ "ok": true }))
}

#[tauri::command]
fn open_section_default<R: Runtime>(app: AppHandle<R>, input_pdf_path: String, output_root_path: Option<String>, section_relative_path: String) -> Result<Value, String> {
    let full = section_full_path(&app, &input_pdf_path, output_root_path.as_deref(), &section_relative_path)?;
    app.opener().open_path(full.to_string_lossy(), None::<&str>).map_err(|e| e.to_string())?;
    Ok(json!({ "ok": true }))
}

#[tauri::command]
fn copy_section_path<R: Runtime>(app: AppHandle<R>, input_pdf_path: String, output_root_path: Option<String>, section_relative_path: String) -> Result<Value, String> {
    let full = section_full_path(&app, &input_pdf_path, output_root_path.as_deref(), &section_relative_path)?;
    let text = full.to_string_lossy().to_string();
    app.clipboard().write_text(text.clone()).map_err(|e| e.to_string())?;
    Ok(json!({ "ok": true, "path": text }))
}

#[tauri::command]
fn open_output_dir<R: Runtime>(app: AppHandle<R>, input_pdf_path: String, output_root_path: Option<String>) -> Result<Value, String> {
    let root = output_root(&app, output_root_path.as_deref());
    let _ = std::fs::create_dir_all(&root);
    let dir = output_dir_for(&app, &input_pdf_path, output_root_path.as_deref());
    let target = if dir.exists() { dir } else { root };
    app.opener().open_path(target.to_string_lossy(), None::<&str>).map_err(|e| e.to_string())?;
    Ok(json!({ "ok": true }))
}

/// The launch time as an ISO-8601 UTC string (for the build stamp).
fn started_at() -> String {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

/// Passes files dragged onto the window to the page as `file-drag` events
/// (`enter`, `leave`, or `drop` with the paths). A drop anywhere in the window
/// counts, so the page does not need the drop position, which macOS reports in
/// different units from the other platforms.
fn forward_file_drops<R: Runtime>(window: &tauri::Window<R>, event: &tauri::WindowEvent) {
    use tauri::{DragDropEvent, WindowEvent};
    let payload = match event {
        WindowEvent::DragDrop(DragDropEvent::Enter { .. }) => json!({ "type": "enter" }),
        WindowEvent::DragDrop(DragDropEvent::Leave) => json!({ "type": "leave" }),
        WindowEvent::DragDrop(DragDropEvent::Drop { paths, .. }) => json!({ "type": "drop", "paths": paths }),
        _ => return,
    };
    let _ = window.emit("file-drag", payload);
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(AppState { started_at: started_at() })
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_title(&window_title());
            }
            Ok(())
        })
        .on_window_event(forward_file_drops)
        .invoke_handler(tauri::generate_handler![
            pick_pdf,
            validate_input_pdf,
            resolve_dropped_input_pdf,
            pick_output_dir,
            get_default_output_root,
            get_app_meta,
            run_conversion,
            open_external_url,
            load_outline,
            load_section,
            open_section_in_folder,
            open_section_default,
            copy_section_path,
            open_output_dir,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Pete's PDF to MD");
}

#[cfg(test)]
mod tests {
    //! Commands called the way tauri-bridge.js calls them (same names, same
    //! camelCase argument JSON), through Tauri's mock runtime.
    use super::*;
    use tauri::ipc::{CallbackFn, InvokeBody};
    use tauri::test::{get_ipc_response, mock_builder, mock_context, noop_assets, MockRuntime, INVOKE_KEY};
    use tauri::webview::InvokeRequest;

    fn app() -> tauri::App<MockRuntime> {
        mock_builder()
            .manage(AppState { started_at: started_at() })
            .invoke_handler(tauri::generate_handler![validate_input_pdf, resolve_dropped_input_pdf, get_app_meta, run_conversion, load_outline, load_section])
            .build(mock_context(noop_assets()))
            .expect("mock app")
    }

    fn call(webview: &tauri::WebviewWindow<MockRuntime>, cmd: &str, body: Value) -> Result<Value, Value> {
        get_ipc_response(
            webview,
            InvokeRequest {
                cmd: cmd.into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: "tauri://localhost".parse().unwrap(),
                body: InvokeBody::Json(body),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .map(|b| b.deserialize::<Value>().unwrap())
    }

    fn fixture() -> Option<String> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/pdfs/Digital-Technology-and-Innovations-in-the-Changing-World_Grade 10_Open_2023.pdf");
        path.exists().then(|| path.to_string_lossy().to_string())
    }

    #[test]
    fn bridge_calls_reach_the_commands() {
        let app = app();
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default()).build().unwrap();
        let meta = call(&webview, "get_app_meta", json!({})).unwrap();
        assert_eq!(meta["version"], DISPLAY_VERSION);
        let bad = call(&webview, "validate_input_pdf", json!({ "candidatePath": "/nope/missing.txt" })).unwrap();
        assert_eq!(bad["ok"], false);
        let dropped = call(&webview, "resolve_dropped_input_pdf", json!({ "payload": { "files": [{ "path": "file:///nope/a%20b.pdf" }] } })).unwrap();
        assert_eq!(dropped["message"], "The selected PDF file could not be found on disk.");
    }

    #[test]
    fn file_drops_reach_the_page() {
        use std::sync::{Arc, Mutex};
        use tauri::Listener;
        let app = app();
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default()).build().unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        app.listen("file-drag", move |event| sink.lock().unwrap().push(event.payload().to_string()));
        let window = webview.as_ref().window();
        let position = tauri::PhysicalPosition::new(0.0, 0.0);
        let paths = vec![PathBuf::from("/tmp/a b.pdf")];
        forward_file_drops(&window, &tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Enter { paths: paths.clone(), position }));
        forward_file_drops(&window, &tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, position }));
        let seen: Vec<Value> = seen.lock().unwrap().iter().map(|p| serde_json::from_str(p).unwrap()).collect();
        assert_eq!(seen, vec![json!({ "type": "enter" }), json!({ "type": "drop", "paths": ["/tmp/a b.pdf"] })]);
    }

    #[test]
    fn conversion_round_trip() {
        let Some(pdf) = fixture() else { return };
        let out = std::env::temp_dir().join(format!("pdf-to-md-tauri-test-{}", std::process::id()));
        let app = app();
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default()).build().unwrap();
        let root = out.to_string_lossy().to_string();
        let result = call(&webview, "run_conversion", json!({
            "inputPdfPath": pdf, "outputRootPath": root, "options": { "outputMode": "sections", "includeSectionMetadata": true }
        }))
        .expect("conversion");
        let items = result["outlineItems"].as_array().expect("outline items");
        assert!(items.len() > 10, "outline has {} items", items.len());
        let section = items.iter().find_map(|i| i["section_file"].as_str()).expect("a section file").to_string();
        let loaded = call(&webview, "load_section", json!({ "inputPdfPath": pdf, "outputRootPath": root, "sectionRelativePath": section })).unwrap();
        assert!(loaded["content"].as_str().unwrap().starts_with('#'));
        let outline = call(&webview, "load_outline", json!({ "inputPdfPath": pdf, "outputRootPath": root })).unwrap();
        assert_eq!(outline["outlineItems"].as_array().unwrap().len(), items.len());
        let _ = std::fs::remove_dir_all(out);
    }
}
