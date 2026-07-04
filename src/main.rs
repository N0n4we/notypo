#![allow(unexpected_cfgs)]

#[link(name = "AppKit", kind = "framework")]
#[link(name = "WebKit", kind = "framework")]
extern "C" {}

#[macro_use]
extern crate objc;

use objc::declare::ClassDecl;
use objc::runtime::{BOOL, Object, Protocol, Sel, NO, YES};
use std::ffi::CString;
use std::path::Path;
use std::ptr;
use std::sync::{LazyLock, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

// Compile-time path to the TypeMark assets inside the cargo project. Used as a
// dev fallback when the binary is run straight from `target/`. Packaged builds
// resolve the bundle copy at runtime via `TYPE_MARK` below.
const TYPE_MARK_DEV: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/TypeMark");

// Resolve the TypeMark assets directory at runtime. In a packaged .app the
// binary lives at `Notypo.app/Contents/MacOS/notypo`, and the assets are copied
// to `Notypo.app/Contents/Resources/assets/TypeMark`, so we look there first and
// fall back to the compile-time project path for `cargo run` / dev builds.
static TYPE_MARK: LazyLock<String> = LazyLock::new(|| {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(contents) = exe.parent().and_then(|macos| macos.parent()) {
            let bundled = contents.join("Resources").join("assets").join("TypeMark");
            if bundled.is_dir() {
                return bundled.to_string_lossy().into_owned();
            }
        }
    }
    TYPE_MARK_DEV.to_string()
});

static mut WEBVIEW: *mut Object = ptr::null_mut();
static mut WINDOW: *mut Object = ptr::null_mut();
static mut MENU_TARGET: *mut Object = ptr::null_mut();

static DOCUMENT: LazyLock<Mutex<DocumentState>> =
    LazyLock::new(|| Mutex::new(DocumentState::untitled()));
static RECENT_FILES: LazyLock<Mutex<Vec<String>>> = LazyLock::new(|| Mutex::new(Vec::new()));
static MOUNT_FOLDER: LazyLock<Mutex<Option<String>>> = LazyLock::new(|| Mutex::new(None));


#[repr(C)]
#[derive(Copy, Clone)]
struct NSPoint {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct NSSize {
    width: f64,
    height: f64,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct NSRect {
    origin: NSPoint,
    size: NSSize,
}

#[derive(Clone, Debug)]
struct DocumentState {
    path: Option<String>,
    content: String,
    encoding: String,
    dirty: bool,
    /// TypeMark change counter. Positive = unedited changes since last save.
    /// Mirrors Typora's `F.changeCounter` via `document.updateChangeCount`.
    change_count: i64,
    /// Whether the document was ever edited (for `updateChangeCountIfUnedited`).
    edited: bool,
}

impl DocumentState {

    fn untitled() -> Self {
        Self {
            path: None,
            content: String::new(),
            encoding: "utf-8".to_string(),
            dirty: false,
            change_count: 0,
            edited: false,
        }
    }

    fn open(path: impl Into<String>) -> std::io::Result<Self> {
        let path = path.into();
        let bytes = std::fs::read(&path)?;
        let content = String::from_utf8_lossy(&bytes).into_owned();
        Ok(Self {
            path: Some(path),
            content,
            encoding: "utf-8".to_string(),
            dirty: false,
            change_count: 0,
            edited: false,
        })
    }

    fn display_name(&self) -> String {
        self.path
            .as_deref()
            .and_then(|p| Path::new(p).file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("Untitled")
            .to_string()
    }

    fn folder(&self) -> Option<String> {
        self.path
            .as_deref()
            .and_then(|p| Path::new(p).parent())
            .map(|p| p.to_string_lossy().into_owned())
    }

    fn is_edited(&self) -> bool {
        self.change_count > 0 || self.dirty
    }

    fn typemark_state(&self) -> serde_json::Value {
        serde_json::json!({
            "currentFilePath": self.path,
            "filePath": self.path,
            "originalPath": self.path,
            "fileName": self.display_name(),
            "fileEncode": self.encoding,
            "currentFolderPath": self.folder(),
            "removed": false,
            "useCRLF": false,
            "unsupported": "",
            "hasModified": self.is_edited(),
            "modifiedDate": null,
            "lastSnapDate": null,
            "isLocked": false,
            "oversize": false,
            "fileMissingWhenOpen": false,
            "bundleFile": null,
            "zip": null
        })
    }
}

fn nsstring(s: &str) -> *mut Object {
    let c = CString::new(s).expect("NSString source must not contain NUL");
    unsafe { msg_send![class!(NSString), stringWithUTF8String: c.as_ptr()] }
}

unsafe fn nsstr_to_string(obj: *mut Object) -> Option<String> {
    if obj.is_null() {
        return None;
    }
    let bytes: *const i8 = msg_send![obj, UTF8String];
    if bytes.is_null() {
        return None;
    }
    let len: usize = msg_send![obj, lengthOfBytesUsingEncoding: 4u64];
    let slice = std::slice::from_raw_parts(bytes as *const u8, len);
    Some(String::from_utf8_lossy(slice).into_owned())
}

/// Convert an ObjC object (NSDictionary/NSArray/NSString/NSNumber/NSNull) as received
/// from `WKScriptMessage.body` into a `serde_json::Value`. NSJSONSerialization
/// handles all JSON-compatible Foundation types, so we use it unconditionally.
unsafe fn nsobj_to_json(obj: *mut Object) -> Option<serde_json::Value> {
    if obj.is_null() {
        return None;
    }
    // NSJSONSerialization refuses NSNull; short-circuit to JSON null.
    if msg_send![obj, isKindOfClass: class!(NSNull)] {
        return Some(serde_json::Value::Null);
    }
    let json_cls = class!(NSJSONSerialization);
    let opts: u64 = 0;
    let mut err: *mut Object = ptr::null_mut();
    let data: *mut Object = msg_send![
        json_cls,
        dataWithJSONObject: obj
        options: opts
        error: &mut err as *mut *mut Object
    ];
    if data.is_null() {
        // Not a JSON-serializable container (e.g. bare scalar that the bridge
        // never sends in practice). Fall back to stringifying.
        return nsstr_to_string(obj).map(serde_json::Value::String);
    }
    let bytes: *const i8 = msg_send![data, bytes];
    let len: usize = msg_send![data, length];
    if bytes.is_null() || len == 0 {
        return None;
    }
    let slice = std::slice::from_raw_parts(bytes as *const u8, len);
    serde_json::from_slice(slice).ok()
}

fn with_document<R>(f: impl FnOnce(&mut DocumentState) -> R) -> R {
    let mut doc = DOCUMENT.lock().expect("document mutex poisoned");
    f(&mut doc)
}

/// Apply a TypeMark ChangeType to the document's change counter.
/// Mirrors Typora's `F.changeCounter.updateChangeCount(e)`:
///   0=NSChangeDone, 1=NSChangeUndone, 2=NSChangeCleared,
///   3=NSChangeReadOtherContents, 4=NSChangeAutoSaved, 5=NSChangeRedone
fn apply_change_count(doc: &mut DocumentState, change_type: i64) {
    match change_type {
        0 => { // NSChangeDone
            doc.change_count += 1;
            doc.edited = true;
            doc.dirty = true;
        }
        1 => { // NSChangeUndone
            doc.change_count -= 1;
            if doc.change_count <= 0 {
                doc.dirty = false;
            }
        }
        2 => { // NSChangeCleared
            doc.change_count = 0;
            doc.dirty = false;
        }
        3 => { // NSChangeReadOtherContents
            // Reload from disk — doesn't change edited state.
        }
        4 => { // NSChangeAutoSaved
            doc.change_count = 0;
            doc.dirty = false;
        }
        5 => { // NSChangeRedone
            doc.change_count += 1;
            doc.edited = true;
            doc.dirty = true;
        }
        -1 => { // NodeSaveFailed
            // Save failed — keep dirty state as-is.
        }
        _ => {}
    }
}
/// Synchronous bridge handler — the Rust side of `bridge.callSync(method, data)`.
/// The original TypeMark index.html calls `prompt("__bridge__", JSON.stringify(dict))`
/// where `dict = {name, method, data}`. Our WKUIDelegate intercepts that prompt,
/// extracts `name.method`, dispatches here, and returns the JSON-serialized result
/// string back to JS synchronously via the prompt completion handler.
///
/// Returns a JSON-serialized string (what `prompt()` returns to JS).
/// The JS side does `JSON.parse(res)` on non-null results.
fn call_sync(name: &str, method: &str, data: &serde_json::Value) -> String {
    let full = format!("{name}.{method}");
    match full.as_str() {
        "setting.get" => {
            let key = data.as_str().unwrap_or("");
            setting_get(key).map(|v| serde_json::to_string(&v).unwrap_or_else(|_| "null".to_string()))
                .unwrap_or_else(|| "null".to_string())
        }
        "setting.loadAll" => {
            serde_json::to_string(&settings_all()).unwrap_or_else(|_| "{}".to_string())
        }
        "clipboard.readText" => {
            unsafe { read_pasteboard_string().unwrap_or_default() }
        }
        "path.readText" => {
            let path = data.as_str().unwrap_or("");
            std::fs::read_to_string(path).unwrap_or_default()
        }
        "document.isDocumentEdited" => {
            with_document(|doc| doc.is_edited()).to_string()
        }
        "images.convertFakeUrl" => {
            // Pass through — we don't use fake URLs, just return as-is.
            data.as_str().unwrap_or("").to_string()
        }
        "contextMenu.setItems" => {
            // No-op; context menu is handled natively by AppKit menus.
            "null".to_string()
        }
        _ => {
            eprintln!("[callSync] unhandled: {full}");
            "null".to_string()
        }
    }
}

/// Detect whether macOS is currently in Dark Mode by inspecting the app's
/// effective appearance. Uses `bestMatchFromAppearancesWithNames:` because the
/// effective appearance can be a composite, and matching against the two base
/// appearance names is the officially recommended way to resolve it.
unsafe fn system_is_dark() -> bool {
    let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
    if app.is_null() {
        return false;
    }
    let appearance: *mut Object = msg_send![app, effectiveAppearance];
    if appearance.is_null() {
        return false;
    }
    let names: *mut Object = msg_send![class!(NSMutableArray), array];
    let _: () = msg_send![names, addObject: nsstring("NSAppearanceNameAqua")];
    let _: () = msg_send![names, addObject: nsstring("NSAppearanceNameDarkAqua")];
    let best: *mut Object = msg_send![appearance, bestMatchFromAppearancesWithNames: names];
    match nsstr_to_string(best) {
        Some(name) => name.contains("Dark"),
        None => false,
    }
}

/// Pick the document theme based on the system appearance: Typora's built-in
/// dark theme (`night`) when macOS is in Dark Mode, otherwise `github`.
fn current_theme() -> &'static str {
    if unsafe { system_is_dark() } {
        "night"
    } else {
        "github"
    }
}

/// Push the current system-appropriate theme into the live webview by swapping
/// the `#theme_css` stylesheet (via the injected `__notypoSetTheme` helper).
unsafe fn apply_current_theme_to_webview() {
    let theme = current_theme();
    let js = format!("window.__notypoSetTheme && window.__notypoSetTheme('{theme}');");
    evaluate_js(&js);
}

/// Read a single setting value by key. Matches Typora's native `setting.get`.
fn setting_get(key: &str) -> Option<serde_json::Value> {
    match key {
        "theme" | "curTheme" => Some(serde_json::json!(current_theme())),
        "hasLicense" => Some(serde_json::json!(true)),
        "currentThemeFolder" => Some(serde_json::json!(format!("{}/style/themes", TYPE_MARK.as_str()))),
        "zoomFactor" => Some(serde_json::json!(1.0)),
        "sidebarTab" => Some(serde_json::json!("outline")),
        "sidebarWidth" => Some(serde_json::json!(260)),
        "useTreeStyle" => Some(serde_json::json!(true)),
        "useCRLF" => Some(serde_json::json!(false)),
        "autoPair" => Some(serde_json::json!(true)),
        "autoMatch" => Some(serde_json::json!(true)),
        "exportType" => Some(serde_json::json!("html")),
        "isEncodeUTF8" => Some(serde_json::json!(true)),
        "fontSize" | "editorFontSize" => Some(serde_json::json!(14)),
        "fontFamily" | "editorFontFamily" => Some(serde_json::json!("Menlo")),
        _ => None,
    }
}

/// Return all settings as a JSON object — used by `setting.loadAll`.
fn settings_all() -> serde_json::Value {
    let keys = [
        "theme", "curTheme", "hasLicense", "currentThemeFolder", "zoomFactor",
        "sidebarTab", "sidebarWidth", "useTreeStyle", "useCRLF", "autoPair", "autoMatch",
        "exportType", "isEncodeUTF8", "fontSize", "editorFontSize",
        "fontFamily", "editorFontFamily",
    ];
    let mut map = serde_json::Map::new();
    for k in keys {
        if let Some(v) = setting_get(k) {
            map.insert(k.to_string(), v);
        }
    }
    serde_json::Value::Object(map)
}

/// Read plain text from the system pasteboard (NSPasteboard).
unsafe fn read_pasteboard_string() -> Option<String> {
    let pb: *mut Object = msg_send![class!(NSPasteboard), generalPasteboard];
    if pb.is_null() {
        return None;
    }
    let str_type: *mut Object = msg_send![class!(NSString), stringWithUTF8String: b"public.utf8-plain-text\0".as_ptr() as *const i8];
    let s: *mut Object = msg_send![pb, stringForType: str_type];
    nsstr_to_string(s)
}

/// Write plain text to the system pasteboard.
unsafe fn write_pasteboard_string(text: &str) {
    let pb: *mut Object = msg_send![class!(NSPasteboard), generalPasteboard];
    if pb.is_null() {
        return;
    }
    let _: () = msg_send![pb, clearContents];
    let str_type: *mut Object = nsstring("public.utf8-plain-text");
    let _: () = msg_send![pb, setString: nsstring(text) forType: str_type];
}

/// Open a URL in the default browser via `NSWorkspace openURL:`.
unsafe fn open_url_in_browser(url: &str) {
    let nsurl: *mut Object = msg_send![class!(NSURL), URLWithString: nsstring(url)];
    if nsurl.is_null() {
        return;
    }
    let workspace: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
    let _: () = msg_send![workspace, openURL: nsurl];
}

/// Reveal a file or folder in Finder via `NSWorkspace activateFileViewerSelecting:`.
unsafe fn reveal_in_finder(path: &str, select_file: bool) {
    let url: *mut Object = msg_send![class!(NSURL), fileURLWithPath: nsstring(path)];
    if url.is_null() {
        return;
    }
    let workspace: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
    if select_file {
        let urls: *mut Object = msg_send![class!(NSArray), arrayWithObject: url];
        let _: () = msg_send![workspace, activateFileViewerSelecting: urls];
    } else {
        let _: () = msg_send![workspace, openURL: url];
    }
}

fn record_recent_file(path: &str) {
    let mut recent = RECENT_FILES.lock().expect("recent files mutex poisoned");
    recent.retain(|p| p != path);
    recent.insert(0, path.to_string());
    recent.truncate(20);
}

fn recent_files_json() -> serde_json::Value {
    let recent = RECENT_FILES.lock().expect("recent files mutex poisoned");
    serde_json::json!(recent.iter().map(|path| {
        serde_json::json!({
            "path": path,
            "name": Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(path)
        })
    }).collect::<Vec<_>>())
}

fn set_mount_folder(path: Option<String>) {
    *MOUNT_FOLDER.lock().expect("mount folder mutex poisoned") = path;
}

fn set_mount_folder_if_empty(path: Option<String>) {
    let mut mount = MOUNT_FOLDER.lock().expect("mount folder mutex poisoned");
    if mount.is_none() {
        *mount = path;
    }
}

fn current_mount_folder() -> Option<String> {
    MOUNT_FOLDER
        .lock()
        .expect("mount folder mutex poisoned")
        .clone()
        .or_else(|| with_document(|doc| doc.folder()))
}

fn file_time_millis(time: std::io::Result<SystemTime>) -> u128 {
    time.ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn file_tree_node(path: &Path, include_children: bool) -> Option<serde_json::Value> {
    let metadata = std::fs::metadata(path).ok()?;
    let is_dir = metadata.is_dir();
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| path.to_string_lossy().into_owned());
    let mut subdir = Vec::new();
    let mut content = Vec::new();

    if is_dir && include_children {
        let mut entries = std::fs::read_dir(path)
            .ok()?
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let entry_path = entry.path();
                let entry_name = entry_path.file_name()?.to_str()?;
                if entry_name.starts_with('.') {
                    return None;
                }
                file_tree_node(&entry_path, false)
            })
            .collect::<Vec<_>>();
        entries.sort_by(|a, b| {
            let a_dir = a["isDirectory"].as_bool().unwrap_or(false);
            let b_dir = b["isDirectory"].as_bool().unwrap_or(false);
            b_dir
                .cmp(&a_dir)
                .then_with(|| a["name"].as_str().unwrap_or("").cmp(b["name"].as_str().unwrap_or("")))
        });
        for node in entries {
            if node["isDirectory"].as_bool().unwrap_or(false) {
                subdir.push(node);
            } else {
                content.push(node);
            }
        }
    } else if is_dir {
        let mut has_subdir = false;
        let mut has_content = false;
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.filter_map(Result::ok) {
                if let Ok(child_meta) = entry.metadata() {
                    if child_meta.is_dir() {
                        has_subdir = true;
                    } else if child_meta.is_file() {
                        has_content = true;
                    }
                }
                if has_subdir && has_content {
                    break;
                }
            }
        }
        if has_subdir {
            subdir.push(serde_json::json!({
                "name": "",
                "path": format!("{}/", path.to_string_lossy()),
                "isDirectory": true,
                "isFile": false,
                "subdir": [],
                "content": []
            }));
        }
        if has_content {
            content.push(serde_json::json!({
                "name": "",
                "path": format!("{}/.notypo-placeholder", path.to_string_lossy()),
                "isDirectory": false,
                "isFile": true
            }));
        }
    }

    Some(serde_json::json!({
        "name": name,
        "path": path.to_string_lossy(),
        "isDirectory": is_dir,
        "isFile": metadata.is_file(),
        "subdir": subdir,
        "content": content,
        "lastModified": file_time_millis(metadata.modified()),
        "createDate": file_time_millis(metadata.created()),
    }))
}

fn folder_tree_json(path: &str) -> serde_json::Value {
    file_tree_node(Path::new(path), true).unwrap_or(serde_json::Value::Null)
}


fn document_load_response() -> serde_json::Value {
    with_document(|doc| serde_json::json!([doc.content, null, doc.typemark_state()]))
}

fn set_document_content(content: String) -> bool {
    with_document(|doc| {
        doc.content = content;
        doc.dirty = true;
        doc.change_count += 1;
        doc.edited = true;
    });
    true
}

fn save_document_to(path: Option<String>) -> std::io::Result<String> {
    let (path, content) = with_document(|doc| {
        if let Some(path) = path {
            doc.path = Some(path);
        }
        let path = doc.path.clone().unwrap_or_else(|| "Untitled.md".to_string());
        (path, doc.content.clone())
    });
    std::fs::write(&path, content)?;
    with_document(|doc| {
        doc.path = Some(path.clone());
        doc.dirty = false;
        doc.change_count = 0;
        doc.edited = false;
    });
    record_recent_file(&path);
    set_mount_folder_if_empty(Path::new(&path).parent().map(|p| p.to_string_lossy().into_owned()));
    unsafe { update_window_title(); }
    Ok(path)
}

fn open_document_from(path: String) -> std::io::Result<()> {
    let next = DocumentState::open(path.clone())?;
    with_document(|doc| *doc = next);
    record_recent_file(&path);
    unsafe { update_window_title(); }
    Ok(())
}

unsafe fn update_window_title() {
    if WINDOW.is_null() {
        return;
    }
    let title = with_document(|doc| {
        let dirty = if doc.is_edited() { " •" } else { "" };
        format!("{}{} — notypo", doc.display_name(), dirty)
    });
    let _: () = msg_send![WINDOW, setTitle: nsstring(&title)];
}

unsafe fn move_window_by(dx: f64, dy: f64) {
    if WINDOW.is_null() {
        return;
    }
    let frame: NSRect = msg_send![WINDOW, frame];
    let origin = NSPoint {
        x: frame.origin.x + dx,
        // Browser screen coordinates grow downward; AppKit window origins grow upward.
        y: frame.origin.y - dy,
    };
    let _: () = msg_send![WINDOW, setFrameOrigin: origin];
}

unsafe fn run_open_panel(select_folder: bool) -> Option<String> {
    let panel: *mut Object = msg_send![class!(NSOpenPanel), openPanel];
    let _: () = msg_send![panel, setCanChooseFiles: if select_folder { NO } else { YES }];
    let _: () = msg_send![panel, setCanChooseDirectories: if select_folder { YES } else { NO }];
    let _: () = msg_send![panel, setAllowsMultipleSelection: NO];
    let result: i64 = msg_send![panel, runModal];
    if result != 1 {
        return None;
    }
    let url: *mut Object = msg_send![panel, URL];
    let path: *mut Object = msg_send![url, path];
    nsstr_to_string(path)
}

unsafe fn run_save_panel() -> Option<String> {
    let panel: *mut Object = msg_send![class!(NSSavePanel), savePanel];
    let default_name = with_document(|doc| doc.display_name());
    let _: () = msg_send![panel, setNameFieldStringValue: nsstring(&default_name)];
    let result: i64 = msg_send![panel, runModal];
    if result != 1 {
        return None;
    }
    let url: *mut Object = msg_send![panel, URL];
    let path: *mut Object = msg_send![url, path];
    nsstr_to_string(path)
}

unsafe fn evaluate_js(js: &str) {
    if WEBVIEW.is_null() {
        return;
    }
    let _: () = msg_send![
        WEBVIEW,
        evaluateJavaScript: nsstring(js)
        completionHandler: ptr::null::<Object>()
    ];
}

unsafe fn call_js_handler(handler: &str, data: serde_json::Value) {
    let message = serde_json::json!({
        "handlerName": handler,
        "data": data,
    });
    let Ok(message_json) = serde_json::to_string(&message) else {
        return;
    };
    let Ok(arg) = serde_json::to_string(&message_json) else {
        return;
    };
    evaluate_js(&format!("_handleMessageFromObjC({arg})"));
}

unsafe fn push_recent_files_to_typemark() {
    let files = {
        let recent = RECENT_FILES.lock().expect("recent files mutex poisoned");
        recent.clone()
    };
    call_js_handler("quickOpen.setRecentFiles", serde_json::json!(files));
    call_js_handler("quickOpen.setRecentFolders", serde_json::json!([]));
}

unsafe fn reload_webview_document() {
    if !WEBVIEW.is_null() {
        let _: () = msg_send![WEBVIEW, reload];
    }
}

unsafe fn push_mount_folder_to_typemark(show_sidebar: bool) {
    let folder = current_mount_folder();
    let folder_json = serde_json::to_string(&folder).unwrap_or_else(|_| "null".to_string());
    let show = if show_sidebar { "true" } else { "false" };
    evaluate_js(&format!(
        r#"(function () {{
            var folder = {folder_json};
            if (window.File) {{
                File.mountFolder_ = folder;
            }}
            var l = File.editor && File.editor.library;
            if (l) {{
                l.root = null;
                if (folder) {{
                    if ({show}) {{
                        l.showSidebar('file-tree');
                    }} else if (l.isFileTabShown && l.isFileTabShown()) {{
                        l.switch('file-tree', true);
                        if (l.fileTree && l.fileTree.render) l.fileTree.render(true);
                    }}
                }}
            }}
        }})()"#
    ));
}

unsafe fn sync_editor_then(handler: &str) {
    let script = format!(
        r#"(function () {{
            var content = "";
            try {{
                if (window.File && File.sync) {{
                    content = File.sync(true, false, true);
                }} else {{
                    var write = document.querySelector('#write') || document.querySelector('[contenteditable="true"]');
                    content = write ? (write.innerText || write.textContent || "") : "";
                }}
            }} catch (err) {{
                console.error(err);
            }}
            window.bridge && window.bridge.callHandler("document.setContent", content);
            window.bridge && window.bridge.callHandler("{handler}", null);
        }})()"#
    );
    evaluate_js(&script);
}

extern "C" fn menu_open(_this: &Object, _cmd: Sel, _sender: *mut Object) {
    unsafe {
        if let Some(path) = run_open_panel(false) {
            let _ = open_path_and_reload(path);
        }
    }
}

extern "C" fn menu_save(_this: &Object, _cmd: Sel, _sender: *mut Object) {
    unsafe { sync_editor_then("document.save"); }
}

extern "C" fn menu_save_as(_this: &Object, _cmd: Sel, _sender: *mut Object) {
    unsafe { sync_editor_then("document.saveAs"); }
}

extern "C" fn menu_new(_this: &Object, _cmd: Sel, _sender: *mut Object) {
    with_document(|doc| *doc = DocumentState::untitled());
    unsafe {
        update_window_title();
        reload_webview_document();
    }
}

extern "C" fn menu_toggle_outline(_this: &Object, _cmd: Sel, _sender: *mut Object) {
    // Toggle the sidebar TOC. If the sidebar is already showing the outline
    // tab, hide it; otherwise reveal it with the outline tab active.
    let js = "var l=File.editor&&File.editor.library; \
              if(l){ \
                l.isSidebarShown() && l.getActiveTab()==='outline' \
                  ? l.hideSidebar() \
                  : l.showSidebar('outline'); \
              }";
    unsafe { evaluate_js(js); }
}

fn register_menu_target() {
    let Some(mut cls) = ClassDecl::new("NotypoMenuTarget", class!(NSObject)) else {
        return;
    };
    unsafe {
        cls.add_method(sel!(newDocument:), menu_new as extern "C" fn(&Object, Sel, *mut Object));
        cls.add_method(sel!(openDocument:), menu_open as extern "C" fn(&Object, Sel, *mut Object));
        cls.add_method(sel!(saveDocument:), menu_save as extern "C" fn(&Object, Sel, *mut Object));
        cls.add_method(sel!(saveDocumentAs:), menu_save_as as extern "C" fn(&Object, Sel, *mut Object));
        cls.add_method(sel!(toggleOutline:), menu_toggle_outline as extern "C" fn(&Object, Sel, *mut Object));
    }
    cls.register();
}

unsafe fn add_menu_item(
    menu: *mut Object,
    title: &str,
    action: Sel,
    key: &str,
    target: *mut Object,
) {
    let item: *mut Object = msg_send![class!(NSMenuItem), alloc];
    let item: *mut Object =
        msg_send![item, initWithTitle: nsstring(title) action: action keyEquivalent: nsstring(key)];
    let _: () = msg_send![item, setTarget: target];
    let _: () = msg_send![menu, addItem: item];
}

unsafe fn install_main_menu(app: *mut Object) {
    let target: *mut Object = msg_send![class!(NotypoMenuTarget), new];
    MENU_TARGET = target;

    let main_menu: *mut Object = msg_send![class!(NSMenu), new];
    let app_item: *mut Object = msg_send![class!(NSMenuItem), new];
    let file_item: *mut Object = msg_send![class!(NSMenuItem), new];
    let edit_item: *mut Object = msg_send![class!(NSMenuItem), new];
    let view_item: *mut Object = msg_send![class!(NSMenuItem), new];
    let _: () = msg_send![main_menu, addItem: app_item];
    let _: () = msg_send![main_menu, addItem: file_item];
    let _: () = msg_send![main_menu, addItem: edit_item];
    let _: () = msg_send![main_menu, addItem: view_item];
    let app_menu: *mut Object = msg_send![class!(NSMenu), alloc];
    let app_menu: *mut Object = msg_send![app_menu, initWithTitle: nsstring("notypo")];
    let quit: *mut Object = msg_send![class!(NSMenuItem), alloc];
    let quit: *mut Object = msg_send![
        quit,
        initWithTitle: nsstring("Quit notypo")
        action: sel!(terminate:)
        keyEquivalent: nsstring("q")
    ];
    let _: () = msg_send![app_menu, addItem: quit];
    let _: () = msg_send![app_item, setSubmenu: app_menu];

    let file_menu: *mut Object = msg_send![class!(NSMenu), alloc];
    let file_menu: *mut Object = msg_send![file_menu, initWithTitle: nsstring("File")];
    add_menu_item(file_menu, "New", sel!(newDocument:), "n", target);
    add_menu_item(file_menu, "Open…", sel!(openDocument:), "o", target);
    let separator: *mut Object = msg_send![class!(NSMenuItem), separatorItem];
    let _: () = msg_send![file_menu, addItem: separator];
    add_menu_item(file_menu, "Save", sel!(saveDocument:), "s", target);
    add_menu_item(file_menu, "Save As…", sel!(saveDocumentAs:), "S", target);
    let _: () = msg_send![file_item, setSubmenu: file_menu];

    // Edit menu — standard selectors with nil target so the responder chain
    // (WKWebView) handles cut/copy/paste/delete/selectAll for the editor.
    let edit_menu: *mut Object = msg_send![class!(NSMenu), alloc];
    let edit_menu: *mut Object = msg_send![edit_menu, initWithTitle: nsstring("Edit")];
    add_menu_item(edit_menu, "Undo", sel!(undo:), "z", ptr::null_mut());
    add_menu_item(edit_menu, "Redo", sel!(redo:), "Z", ptr::null_mut());
    let edit_sep1: *mut Object = msg_send![class!(NSMenuItem), separatorItem];
    let _: () = msg_send![edit_menu, addItem: edit_sep1];
    add_menu_item(edit_menu, "Cut", sel!(cut:), "x", ptr::null_mut());
    add_menu_item(edit_menu, "Copy", sel!(copy:), "c", ptr::null_mut());
    add_menu_item(edit_menu, "Paste", sel!(paste:), "v", ptr::null_mut());
    add_menu_item(edit_menu, "Delete", sel!(delete:), "", ptr::null_mut());
    let edit_sep2: *mut Object = msg_send![class!(NSMenuItem), separatorItem];
    let _: () = msg_send![edit_menu, addItem: edit_sep2];
    add_menu_item(edit_menu, "Select All", sel!(selectAll:), "a", ptr::null_mut());
    let _: () = msg_send![edit_item, setSubmenu: edit_menu];

    // View menu - toggle the sidebar TOC (outline).
    let view_menu: *mut Object = msg_send![class!(NSMenu), alloc];
    let view_menu: *mut Object = msg_send![view_menu, initWithTitle: nsstring("View")];
    add_menu_item(view_menu, "Toggle Outline", sel!(toggleOutline:), "\\", target);
    let _: () = msg_send![view_item, setSubmenu: view_menu];

    let _: () = msg_send![app, setMainMenu: main_menu];
}

fn mime_from_path(path: &str) -> &'static str {
    if path.ends_with(".html") {
        "text/html; charset=utf-8"
    } else if path.ends_with(".js") {
        "application/javascript; charset=utf-8"
    } else if path.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if path.ends_with(".json") {
        "application/json; charset=utf-8"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".jpg") || path.ends_with(".jpeg") {
        "image/jpeg"
    } else if path.ends_with(".gif") {
        "image/gif"
    } else if path.ends_with(".woff2") {
        "font/woff2"
    } else if path.ends_with(".woff") {
        "font/woff"
    } else if path.ends_with(".ttf") {
        "font/ttf"
    } else {
        "application/octet-stream"
    }
}

// WKURLSchemeHandler ---------------------------------------------------------

extern "C" fn start_task(_this: &Object, _cmd: Sel, _webview: *mut Object, task: *mut Object) {
    unsafe {
        let req: *mut Object = msg_send![task, request];
        let url: *mut Object = msg_send![req, URL];
        let path_obj: *mut Object = msg_send![url, path];
        let Some(path) = nsstr_to_string(path_obj) else {
            let _: () = msg_send![task, didFinish];
            return;
        };

        eprintln!("[notypo] serve {path}");
        let rel = path.trim_start_matches('/');
        let file_path = format!("{}/{}", TYPE_MARK.as_str(), rel);
        let (data, ok) = match std::fs::read(&file_path) {
            Ok(data) => (data, true),
            Err(err) => {
                eprintln!("[notypo] missing asset {path}: {err}");
                (b"not found".to_vec(), false)
            }
        };

        let nsurl: *mut Object =
            msg_send![class!(NSURL), URLWithString: nsstring(&format!("notypo://app{path}"))];
        let response: *mut Object = msg_send![class!(NSURLResponse), alloc];
        let response: *mut Object = msg_send![
            response,
            initWithURL: nsurl
            MIMEType: nsstring(if ok { mime_from_path(&path) } else { "text/plain; charset=utf-8" })
            expectedContentLength: data.len() as i64
            textEncodingName: ptr::null::<Object>()
        ];
        let _: () = msg_send![response, autorelease];
        let nsdata: *mut Object =
            msg_send![class!(NSData), dataWithBytes: data.as_ptr() length: data.len() as u64];
        let _: () = msg_send![task, didReceiveResponse: response];
        let _: () = msg_send![task, didReceiveData: nsdata];
        let _: () = msg_send![task, didFinish];
    }
}

extern "C" fn stop_task(_this: &Object, _cmd: Sel, _webview: *mut Object, _task: *mut Object) {}

fn register_url_handler() -> bool {
    let Some(proto) = Protocol::get("WKURLSchemeHandler") else {
        eprintln!("[notypo] WKURLSchemeHandler protocol not found");
        return false;
    };
    let Some(mut cls) = ClassDecl::new("NotypoURLHandler", class!(NSObject)) else {
        return true;
    };
    cls.add_protocol(proto);
    unsafe {
        cls.add_method(
            sel!(webView:startURLSchemeTask:),
            start_task as extern "C" fn(&Object, Sel, *mut Object, *mut Object),
        );
        cls.add_method(
            sel!(webView:stopURLSchemeTask:),
            stop_task as extern "C" fn(&Object, Sel, *mut Object, *mut Object),
        );
    }
    cls.register();
    true
}

// WKScriptMessageHandler -----------------------------------------------------

fn bridge_response(handler: &str, msg: &serde_json::Value) -> serde_json::Value {
    let data = &msg["data"];
    match handler {
        "document.loadData" => document_load_response(),
        "document.switchDocument" => {
            if let Some(path) = data.as_str() {
                if let Err(err) = open_document_from(path.to_string()) {
                    eprintln!("[bridge] failed to open {path}: {err}");
                }
            }
            document_load_response()
        }
        "document.getContent" => with_document(|doc| serde_json::json!(doc.content)),
        "document.setContent" => {
            let content = data.as_str().unwrap_or_default().to_string();
            serde_json::json!(set_document_content(content))
        }
        "document.syncFullContent" | "controller.syncFullContent" => {
            if let Some(content) = data.as_str() {
                set_document_content(content.to_string());
            } else if let Some(content) = data.get(1).and_then(|v| v.as_str()) {
                set_document_content(content.to_string());
            }
            serde_json::json!(true)
        }
        "document.updateChangeCount" => {
            // TypeMark sends the ChangeType enum value as data.
            // 0=NSChangeDone, 1=NSChangeUndone, 2=NSChangeCleared,
            // 3=NSChangeReadOtherContents, 4=NSChangeAutoSaved, 5=NSChangeRedone
            let change_type = data.as_i64().unwrap_or(0);
            let prev_edited = with_document(|doc| doc.is_edited());
            with_document(|doc| apply_change_count(doc, change_type));
            // Refresh title if edited state changed.
            let now_edited = with_document(|doc| doc.is_edited());
            if prev_edited != now_edited {
                unsafe { update_window_title(); }
            }
            serde_json::json!(null)
        }
        "document.updateChangeCountIfUnedited" => {
            // Only marks as edited if the document was never edited before.
            // TypeMark sends NSChangeDone (0) as data.
            let change_type = data.as_i64().unwrap_or(0);
            let prev_edited = with_document(|doc| doc.is_edited());
            with_document(|doc| {
                if !doc.edited {
                    apply_change_count(doc, change_type);
                }
            });
            let now_edited = with_document(|doc| doc.is_edited());
            if prev_edited != now_edited {
                unsafe { update_window_title(); }
            }
            serde_json::json!(null)
        }
        "document.syncChangeIfNeeded" => {
            // No-op: we don't maintain a separate snap/change sync mechanism.
            // TypeMark uses this to check if content needs re-syncing before save.
            serde_json::json!(null)
        }
        "document.addSnapAndLastSync"
        | "document.addSnap"
        | "document.setLastSync"
        | "document.noOtherWindow"
        | "document.needsUpdateSnap"
        | "document.requestBuildSnap"
        | "document.recordUnusedAssets"
        | "document.hasDuplicateName"
        | "document.checkIfMoveOnSave" => serde_json::json!(null),
        "path.selectFolderOrFile" => unsafe {
            let select_folder = data["dir"].as_bool().unwrap_or(false);
            serde_json::json!(run_open_panel(select_folder))
        },
        "path.showSaveDialog" | "dialog.showSaveDialog" => unsafe {
            serde_json::json!(run_save_panel())
        },
        "app.openFile" | "path.openFile" => {
            if let Some(path) = data.as_str() {
                let _ = open_path_and_reload(path.to_string());
            }
            serde_json::json!(true)
        }
        "app.openFileOrFolder" => unsafe {
            if let Some(path) = run_open_panel(false) {
                let _ = open_path_and_reload(path);
                document_load_response()
            } else {
                serde_json::json!(null)
            }
        },
        "document.save" => {
            serde_json::json!(save_document_to(None).ok())
        }
        "document.saveAs" => unsafe {
            let path = data.as_str().map(ToOwned::to_owned).or_else(|| run_save_panel());
            serde_json::json!(path.and_then(|p| save_document_to(Some(p)).ok()))
        },
        "controller.shouldLoadFolder" => serde_json::json!(current_mount_folder().is_some()),
        "window.dragBy" => {
            if let Some(delta) = data.as_array() {
                let dx = delta.get(0).and_then(|v| v.as_f64()).unwrap_or(0.0);
                let dy = delta.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0);
                unsafe { move_window_by(dx, dy); }
            }
            serde_json::json!(true)
        }
        "window.updateMenuForIsAlwaysOnTop" => serde_json::json!(false),
        "window.loadFinished" => serde_json::json!(true),
        "window.refreshFullContentState"
        | "window.setBackground"
        | "window.setTitlebarTextMarginLeft"
        | "window.setInSourceMode"
        | "window.checkAsFocus"
        | "menu.updateMenu"
        | "touchBar.setBlockStyle"
        | "touchBar.setInlineEnabled"
        | "touchBar.setInlineStates"
        | "word.updateWordCount" => serde_json::json!(null),
        "setting.get" | "setting.readSetting" => serde_json::json!(null),
        "setting.getRecentFiles" => serde_json::json!({
            "files": recent_files_json(),
            "folders": []
        }),
        "setting.updateRecentFile" => {
            if let Some(path) = data["path"].as_str().or_else(|| data.as_str()) {
                record_recent_file(path);
                unsafe { push_recent_files_to_typemark(); }
            }
            serde_json::json!(true)
        }
        "setting.removeRecentDocument" => {
            if let Some(path) = data.as_str() {
                let mut recent = RECENT_FILES.lock().expect("recent files mutex poisoned");
                recent.retain(|p| p != path);
                unsafe { push_recent_files_to_typemark(); }
            }
            serde_json::json!(true)
        }
        "quickOpen.cacheRecentFiles" => unsafe {
            push_recent_files_to_typemark();
            serde_json::json!(true)
        },
        "logger.debug" | "logger.error" | "logger.info" => serde_json::json!(null),
        // --- window.* ---
        "window.execForAll" => {
            // Execute a command on all windows. Single-window: just eval.
            if let Some(js) = data.as_str() {
                unsafe { evaluate_js(js); }
            }
            serde_json::json!(null)
        }
        "window.focus" => {
            unsafe {
                if !WINDOW.is_null() {
                    let _: () = msg_send![WINDOW, makeKeyAndOrderFront: ptr::null::<Object>()];
                    let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
                    let _: () = msg_send![app, activateIgnoringOtherApps: YES];
                }
            }
            serde_json::json!(null)
        }
        "window.resetZoom" => {
            unsafe { evaluate_js("document.body.style.zoom='1.0';"); }
            serde_json::json!(null)
        }
        "window.zoomIn" => {
            unsafe { evaluate_js("document.body.style.zoom=(parseFloat(document.body.style.zoom||1)+0.1).toFixed(1);"); }
            serde_json::json!(null)
        }
        "window.zoomOut" => {
            unsafe { evaluate_js("document.body.style.zoom=(Math.max(0.5,parseFloat(document.body.style.zoom||1)-0.1)).toFixed(1);"); }
            serde_json::json!(null)
        }
        "window.showDialog" => {
            // TypeMark sends {type, message, title, buttons, ...}.
            // For now, log and return null — native dialog implementation later.
            eprintln!("[bridge] showDialog: {}", data);
            serde_json::json!(null)
        }
        "window.pasteAsPlainText" => {
            unsafe { evaluate_js("document.execCommand('paste');"); }
            serde_json::json!(null)
        }
        "window.updateUIAfterExitSourceMode"
        | "window.previewFile"
        | "window.open"
        | "window.selectNextTab"
        | "window.selectPreviousTab"
        | "window.setIsSchemeAwareness" => serde_json::json!(null),
        // --- path.* ---
        "path.openURL" => {
            if let Some(url) = data.as_str() {
                unsafe { open_url_in_browser(url); }
            }
            serde_json::json!(null)
        }
        "path.openFolderInFinder" => {
            if let Some(path) = data.as_str() {
                unsafe { reveal_in_finder(path, false); }
            }
            serde_json::json!(null)
        }
        "path.showInFinder" => {
            if let Some(path) = data.as_str() {
                unsafe { reveal_in_finder(path, true); }
            }
            serde_json::json!(null)
        }
        "path.isDirectory" => {
            let path = data.as_str().unwrap_or("");
            serde_json::json!(Path::new(path).is_dir())
        }
        "path.moveTo" | "path.moveFile" => {
            // data is [src, dst] array.
            if let Some(arr) = data.as_array() {
                let src = arr.get(0).and_then(|v| v.as_str());
                let dst = arr.get(1).and_then(|v| v.as_str());
                if let (Some(s), Some(d)) = (src, dst) {
                    let _ = std::fs::rename(s, d);
                }
            }
            serde_json::json!(true)
        }
        "path.removeFiles" => {
            if let Some(path) = data.as_str() {
                let _ = std::fs::remove_file(path);
            } else if let Some(arr) = data.as_array() {
                for p in arr {
                    if let Some(s) = p.as_str() {
                        let _ = std::fs::remove_file(s);
                    }
                }
            }
            serde_json::json!(true)
        }
        // --- quickOpen.* ---
        "quickOpen.query" => {
            // TypeMark asks native to search for files. No-op (no file tree).
            serde_json::json!([])
        }
        "quickOpen.stopQuery" | "quickOpen.reindexFolderIfNeeded" => {
            serde_json::json!(null)
        }
        // --- controller.* ---
        "controller.openFolder" | "controller.switchFolder" => {
            if let Some(path) = data.as_str() {
                if Path::new(path).is_dir() {
                    set_mount_folder(Some(path.to_string()));
                    unsafe {
                        push_mount_folder_to_typemark(true);
                    }
                } else {
                    let _ = open_path_and_reload(path.to_string());
                }
            }
            serde_json::json!(null)
        }
        "controller.runCommand" => {
            // Execute a File.editor command string.
            if let Some(cmd) = data.as_str() {
                unsafe { evaluate_js(cmd); }
            }
            serde_json::json!(null)
        }
        "controller.selectFolder" => unsafe {
            if let Some(path) = run_open_panel(true) {
                set_mount_folder(Some(path));
                push_mount_folder_to_typemark(true);
            }
            serde_json::json!(null)
        }
        "controller.calcMountFolder" => serde_json::json!(current_mount_folder()),
        "controller.openInNewWindow" | "controller.openInTypora" => {
            if let Some(path) = data.as_str() {
                let _ = open_path_and_reload(path.to_string());
            }
            serde_json::json!(null)
        }
        "controller.showErrorDialog" => {
            eprintln!("[bridge] error dialog: {}", data);
            serde_json::json!(null)
        }
        "controller.switchDocumentTarget" => {
            let opened = data
                .as_str()
                .map(|path| open_path_and_reload(path.to_string()))
                .unwrap_or(false);
            serde_json::json!(opened)
        }
        "controller.startDrag" | "controller.bindFolderMonitor" => serde_json::json!(null),
        // --- clipboard.* ---
        "clipboard.read" => {
            // TypeMark expects [text, html, rtf] array.
            let text = unsafe { read_pasteboard_string().unwrap_or_default() };
            serde_json::json!([text, "", ""])
        }
        "clipboard.write" => {
            // data is {text, html, rtf} — write text to pasteboard.
            if let Some(text) = data["text"].as_str().or_else(|| data.as_str()) {
                unsafe { write_pasteboard_string(text); }
            }
            serde_json::json!(null)
        }
        "clipboard.postCopy" => {
            // Notify that a copy operation completed. No-op.
            serde_json::json!(null)
        }
        "clipboard.readImage" | "clipboard.writeImage" => serde_json::json!(null),
        // --- setting.* ---
        "setting.put" => {
            // TypeMark saves a setting. We don't persist yet — no-op.
            serde_json::json!(null)
        }
        "setting.fetchAnalytics" => serde_json::json!(null),
        "setting.showAndHighlight" => serde_json::json!(null),
        // --- library.* ---
        "library.fetchAllDocs" => {
            data.as_str()
                .map(folder_tree_json)
                .unwrap_or_else(|| current_mount_folder().map(|p| folder_tree_json(&p)).unwrap_or(serde_json::json!([])))
        }
        "library.listDocsUnder" => {
            data.as_str()
                .map(folder_tree_json)
                .unwrap_or(serde_json::Value::Null)
        }
        "library.search" => serde_json::json!([]),
        "library.getRecentFolders" => serde_json::json!({ "folders": [] }),
        "library.newFile" | "library.newFileUnder" | "library.newFolder"
        | "library.renameFile" | "library.moveFile" | "library.duplicate"
        | "library.trashItem" | "library.showProperty" | "library.updateListItem"
        | "library.updateListItemIfIsOpen" => serde_json::json!(null),
        // --- images.* ---
        "images.insertLocalImage" | "images.copyImage" | "images.savePNG"
        | "images.upload" | "images.startCopyAllImages" | "images.addToBundle"
        | "images.prepImageMove" | "images.getScale"
        | "images.iPicValidateBeforeUpload" => serde_json::json!(null),
        // --- touchBar.* (no-op, no native touch bar UI) ---
        "touchBar.setFencesOnSetLang" | "touchBar.showDialogTouchBar"
        | "touchBar.showFindReplaceTouchBar" | "touchBar.showInsertTableTouchBar"
        | "touchBar.updateFindReplaceMode" | "touchBar.updateInsertTableTouchBar" => serde_json::json!(null),
        // --- editor.* ---
        "editor.jumpToAnchor" => {
            if let Some(anchor) = data.as_str() {
                unsafe {
                    let js = format!("File.editor && File.editor.tryOpenUrl('#{}')", anchor.replace('\'', "\\'"));
                    evaluate_js(&js);
                }
            }
            serde_json::json!(null)
        }
        "editor.showZoomPanel" | "editor.getSpeechText"
        | "editor.resetPasteboard" | "editor.setPasteboard"
        | "editor.insertText" => serde_json::json!(null),
        // --- word.* ---
        "word.getSpellSuggestionInContext" | "word.getTextReplacement" => serde_json::json!(null),
        _ => {
            eprintln!("[bridge] unhandled callback: {handler}");
            serde_json::json!(null)
        }
    }
}

extern "C" fn on_script_msg(_this: &Object, _cmd: Sel, _controller: *mut Object, message: *mut Object) {
    unsafe {
        let body_obj: *mut Object = msg_send![message, body];
        // The original TypeMark index.html sends a JS object via
        // `webkit.messageHandlers["_bridge"].postMessage(message)`, so `body`
        // arrives as an NSDictionary (or scalar). Convert directly to JSON.
        let Some(msg) = nsobj_to_json(body_obj) else {
            eprintln!("[bridge] empty/unparsable ipc body");
            return;
        };

        let handler = msg["handlerName"].as_str().unwrap_or("?");
        if let Some(cb) = msg["callbackId"].as_str() {
            let response = serde_json::json!({
                "responseId": cb,
                "responseData": bridge_response(handler, &msg),
            });
            let response_json = serde_json::to_string(&response).unwrap_or_else(|_| "{}".to_string());
            let js = format!("_handleMessageFromObjC({})", serde_json::to_string(&response_json).unwrap());
            if !WEBVIEW.is_null() {
                let _: () = msg_send![
                    WEBVIEW,
                    evaluateJavaScript: nsstring(&js)
                    completionHandler: ptr::null::<Object>()
                ];
            }
        } else if handler == "notypo.smoke" {
            eprintln!("[SMOKE] {}", msg["data"]);
        } else {
            let _ = bridge_response(handler, &msg);
        }
    }
}

fn register_script_handler() -> bool {
    let Some(proto) = Protocol::get("WKScriptMessageHandler") else {
        eprintln!("[notypo] WKScriptMessageHandler protocol not found");
        return false;
    };
    let Some(mut cls) = ClassDecl::new("NotypoScriptHandler", class!(NSObject)) else {
        return true;
    };
    cls.add_protocol(proto);
    unsafe {
        cls.add_method(
            sel!(userContentController:didReceiveScriptMessage:),
            on_script_msg as extern "C" fn(&Object, Sel, *mut Object, *mut Object),
        );
    }
    cls.register();
    true
}

// WKUIDelegate — intercepts `prompt("__bridge__", json)` for synchronous callSync

/// Minimal layout of an Objective-C block (Apple's Block ABI). We only need the
/// fields up to and including `invoke`, which is the C function pointer that
/// runs the block's body. The first argument passed to `invoke` is always the
/// block itself; any declared block parameters follow.
///
/// Blocks are NOT ordinary objects that respond to a `call`/`call:` selector —
/// sending them such a message raises "unrecognized selector" and crashes the
/// process. WKWebView's `WKUIDelegate` panel methods hand us a completion
/// handler *block*, so we must invoke it through this ABI instead.
#[repr(C)]
struct BlockLayout {
    isa: *const std::ffi::c_void,
    flags: i32,
    reserved: i32,
    invoke: *const std::ffi::c_void,
}

/// Invoke a `void (^)(void)` completion block.
unsafe fn invoke_block_void(block: *mut Object) {
    if block.is_null() {
        return;
    }
    let layout = &*(block as *const BlockLayout);
    let invoke: extern "C" fn(*mut Object) = std::mem::transmute(layout.invoke);
    invoke(block);
}

/// Invoke a `void (^)(id)` completion block (e.g. prompt's `NSString *result`).
unsafe fn invoke_block_obj(block: *mut Object, arg: *mut Object) {
    if block.is_null() {
        return;
    }
    let layout = &*(block as *const BlockLayout);
    let invoke: extern "C" fn(*mut Object, *mut Object) = std::mem::transmute(layout.invoke);
    invoke(block, arg);
}

/// Invoke a `void (^)(BOOL)` completion block (e.g. confirm's result).
unsafe fn invoke_block_bool(block: *mut Object, arg: BOOL) {
    if block.is_null() {
        return;
    }
    let layout = &*(block as *const BlockLayout);
    let invoke: extern "C" fn(*mut Object, BOOL) = std::mem::transmute(layout.invoke);
    invoke(block, arg);
}

extern "C" fn run_prompt(
    _this: &Object,
    _cmd: Sel,
    _webview: *mut Object,
    prompt: *mut Object,
    default_text: *mut Object,
    _frame: *mut Object,
    completion: *mut Object,
) {
    unsafe {
        let prompt_str = nsstr_to_string(prompt).unwrap_or_default();
        if prompt_str == "__bridge__" {
            // callSync path: default_text is JSON.stringify({name, method, data})
            let payload = nsstr_to_string(default_text).unwrap_or_default();
            let result = handle_call_sync(&payload);
            invoke_block_obj(completion, nsstring(&result));
        } else if prompt_str == "__alert__" {
            // alert() fallback — just dismiss
            invoke_block_obj(completion, nsstring(""));
        } else if prompt_str == "__confirm__" {
            // confirm() fallback — return true
            invoke_block_obj(completion, nsstring("true"));
        } else {
            // Regular prompt — return default text (or empty)
            let default = nsstr_to_string(default_text).unwrap_or_default();
            invoke_block_obj(completion, nsstring(&default));
        }
    }
}

fn handle_call_sync(payload: &str) -> String {
    let Ok(dict) = serde_json::from_str::<serde_json::Value>(payload) else {
        return "null".to_string();
    };
    let name = dict["name"].as_str().unwrap_or("");
    let method = dict["method"].as_str().unwrap_or("");
    let data = &dict["data"];
    call_sync(name, method, data)
}

extern "C" fn run_alert(
    _this: &Object,
    _cmd: Sel,
    _webview: *mut Object,
    _message: *mut Object,
    _frame: *mut Object,
    completion: *mut Object,
) {
    unsafe { invoke_block_void(completion); }
}

extern "C" fn run_confirm(
    _this: &Object,
    _cmd: Sel,
    _webview: *mut Object,
    _message: *mut Object,
    _frame: *mut Object,
    completion: *mut Object,
) {
    unsafe { invoke_block_bool(completion, YES); }
}

fn register_ui_delegate() -> bool {
    let Some(proto) = Protocol::get("WKUIDelegate") else {
        eprintln!("[notypo] WKUIDelegate protocol not found");
        return false;
    };
    let Some(mut cls) = ClassDecl::new("NotypoUIDelegate", class!(NSObject)) else {
        return true;
    };
    cls.add_protocol(proto);
    unsafe {
        cls.add_method(
            sel!(webView:runJavaScriptTextInputPanelWithPrompt:defaultText:initiatedByFrame:completionHandler:),
            run_prompt as extern "C" fn(&Object, Sel, *mut Object, *mut Object, *mut Object, *mut Object, *mut Object),
        );
        cls.add_method(
            sel!(webView:runJavaScriptAlertPanelWithMessage:initiatedByFrame:completionHandler:),
            run_alert as extern "C" fn(&Object, Sel, *mut Object, *mut Object, *mut Object, *mut Object),
        );
        cls.add_method(
            sel!(webView:runJavaScriptConfirmPanelWithMessage:initiatedByFrame:completionHandler:),
            run_confirm as extern "C" fn(&Object, Sel, *mut Object, *mut Object, *mut Object, *mut Object),
        );
    }
    cls.register();
    true
}

// App delegate ---------------------------------------------------------------

extern "C" fn did_finish_launching(_this: &Object, _cmd: Sel, _notification: *mut Object) {
    eprintln!("[notypo] launch");
    unsafe {
        if !register_url_handler() || !register_script_handler() || !register_ui_delegate() {
            eprintln!("[notypo] bridge class registration failed");
            return;
        }

        let rect = NSRect {
            origin: NSPoint { x: 200.0, y: 200.0 },
            size: NSSize { width: 1200.0, height: 800.0 },
        };

        // Style mask: Titled(1) | Closable(2) | Miniaturizable(4) | Resizable(8)
        // | FullSizeContentView(1<<15 = 32768) = 32783.
        //
        // FullSizeContentView is the key bit that "merges the titlebar into the
        // UI" (Typora-style seamless mode): it lets the content view — and thus
        // our WKWebView — extend up *behind* the titlebar, all the way to the top
        // edge of the window, instead of being inset below a separate titlebar
        // strip. Combined with `setTitlebarAppearsTransparent:YES`, the traffic
        // lights float over the web content and TypeMark's `.mac-seamless-mode`
        // layout (which reserves `--title-bar-height` at the top of `content` and
        // `padding-top` on the sidebar) lines up under them.
        const FULL_SIZE_CONTENT_VIEW: u64 = 1 << 15;
        let style_mask: u64 = 15 | FULL_SIZE_CONTENT_VIEW; // 32783
        let win: *mut Object = msg_send![class!(NSWindow), alloc];
        let win: *mut Object =
            msg_send![win, initWithContentRect: rect styleMask: style_mask backing: 2u64 defer: NO];
        WINDOW = win;
        update_window_title();
        let _: () = msg_send![win, setTitlebarAppearsTransparent: YES];
        let _: () = msg_send![win, setMovableByWindowBackground: YES];
        // Keep the native title *visible* (NSWindowTitleVisible = 0). The release
        // TypeMark DOM has no `#title-text`/`#top-titlebar` element (that's only
        // for the Windows/Linux custom chrome), so the document name can only be
        // shown via the native window title. With the full-size content view it is
        // drawn centered over the web content — matching Typora, where the file
        // name sits in the middle of the merged titlebar. It won't collide with
        // the left-hand outline sidebar, which is far from the centered title.
        let _: () = msg_send![win, setTitleVisibility: 0u64];
        let _: () = msg_send![win, center];
        let _: () = msg_send![win, setReleasedWhenClosed: NO];

        let cfg: *mut Object = msg_send![class!(WKWebViewConfiguration), new];
        let pool: *mut Object = msg_send![class!(WKProcessPool), new];
        let _: () = msg_send![cfg, setProcessPool: pool];

        // Allow file://→file:// XHR/fetch. TypeMark loads its diagram engine
        // (mermaid, flowchart, etc.) with jQuery `getScript`, which issues an
        // XMLHttpRequest. WKWebView treats every file:// document as a unique
        // opaque origin and blocks such requests as cross-origin by default, so
        // without these preferences `mermaid.min.js` never loads and Mermaid
        // code fences never render. These KVC keys are the standard way to relax
        // that policy for a trusted local app bundle.
        let yes_num: *mut Object = msg_send![class!(NSNumber), numberWithBool: YES];
        let prefs: *mut Object = msg_send![cfg, preferences];
        let _: () = msg_send![prefs, setValue: yes_num forKey: nsstring("allowFileAccessFromFileURLs")];
        let _: () = msg_send![cfg, setValue: yes_num forKey: nsstring("allowUniversalAccessFromFileURLs")];

        let url_handler: *mut Object = msg_send![class!(NotypoURLHandler), new];
        let _: () = msg_send![cfg, setURLSchemeHandler: url_handler forURLScheme: nsstring("notypo")];

        let content_controller: *mut Object = msg_send![cfg, userContentController];
        let script_handler: *mut Object = msg_send![class!(NotypoScriptHandler), new];
        // Register under "_bridge" to match the original TypeMark index.html,
        // which calls `webkit.messageHandlers["_bridge"].postMessage(message)`.
        let _: () = msg_send![content_controller, addScriptMessageHandler: script_handler name: nsstring("_bridge")];

        // Inject host globals at document start, before TypeMark's inline
        // fastSetup script reads _options. This keeps assets/TypeMark/index.html
        // byte-identical to the original Typora resource.
        //
        // We intentionally do NOT inject window._bridge — without it, the
        // original index.html `callSync` falls back to `prompt("__bridge__", json)`,
        // which our WKUIDelegate intercepts synchronously. This is the same
        // mechanism Typora uses when _bridge is not ready, and the only way to
        // get synchronous JS↔native calls on WKWebView.
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let theme = current_theme();
        let tm = TYPE_MARK.as_str();
        let init_js = format!(
            "window.reqnode=undefined;\
             window.require=undefined;\
             window._options={{\
             theme:'{theme}',\
             curTheme:'{theme}',\
             hasLicense:true,\
             enableDiagram:true,\
             currentThemeFolder:'{tm}/style/themes',\
             appPath:'{tm}',\
             userDataPath:'{home}/Library/Application Support/notypo',\
             documentsPath:'{home}/Documents',\
             mountFolder:null,\
             initMountFolder:null,\
             locale:'en',\
             userLocale:'en',\
             osVersion:'macOS',\
             buildTime:'',\
             zoomFactor:1.0,\
             sidebarTab:'outline',\
             sidebarWidth:260,\
             useTreeStyle:true,\
             searchService:'',\
             tooOldToReport:false,\
             onFirstLaunch:false\
             }};\n\
             window.dirname='';"
        );
        let user_script: *mut Object = msg_send![class!(WKUserScript), alloc];
        let user_script: *mut Object = msg_send![
            user_script,
            initWithSource: nsstring(&init_js)
            injectionTime: 0u64
            forMainFrameOnly: YES
        ];
        let _: () = msg_send![content_controller, addUserScript: user_script];

        // Inject Mac seamless mode classes at document end, matching Typora's
        // native behavior. `mac-supports-vibrant` enables vibrant appearance on
        // the document element; `mac-seamless-mode` gives the `<titlebar>`
        // element its height and enables the seamless titlebar layout.
        // Also define __notypoSetTheme, a small helper that swaps the active
        // theme stylesheet at runtime (mirroring the inline `fastSetup` logic in
        // index.html). Native calls this when the system appearance changes so
        // the editor follows Light/Dark Mode live.
        let seamless_js = "\
            document.documentElement.classList.add('mac-supports-vibrant');\n\
            document.body.classList.add('mac-seamless-mode');\n\
            (function () {\n\
              var dragging = false;\n\
              var lastX = 0;\n\
              var lastY = 0;\n\
              function stopDrag() { dragging = false; }\n\
              document.addEventListener('mousedown', function (event) {\n\
                if (event.button !== 0 || !event.target || event.target.tagName !== 'TITLEBAR') return;\n\
                dragging = true;\n\
                lastX = event.screenX;\n\
                lastY = event.screenY;\n\
                event.preventDefault();\n\
              }, true);\n\
              document.addEventListener('mousemove', function (event) {\n\
                if (!dragging) return;\n\
                var dx = event.screenX - lastX;\n\
                var dy = event.screenY - lastY;\n\
                lastX = event.screenX;\n\
                lastY = event.screenY;\n\
                if ((dx || dy) && window.bridge) bridge.callHandler('window.dragBy', [dx, dy]);\n\
                event.preventDefault();\n\
              }, true);\n\
              document.addEventListener('mouseup', stopDrag, true);\n\
              window.addEventListener('blur', stopDrag);\n\
            })();\n\
            window.__notypoSetTheme = function (theme) {\n\
              try {\n\
                var opts = window._options || {};\n\
                var folder = 'file://' + opts.currentThemeFolder;\n\
                var css = document.getElementById('theme_css');\n\
                if (css) css.setAttribute('href', folder + '/' + theme + '.css');\n\
                var themeUser = document.getElementById('theme_user_css');\n\
                if (themeUser) themeUser.setAttribute('href', folder + '/' + theme + '.user.css');\n\
                opts.theme = theme;\n\
                opts.curTheme = theme;\n\
              } catch (e) { console.error(e); }\n\
            };";
        let seamless_script: *mut Object = msg_send![class!(WKUserScript), alloc];
        let seamless_script: *mut Object = msg_send![
            seamless_script,
            initWithSource: nsstring(seamless_js)
            injectionTime: 1u64
            forMainFrameOnly: YES
        ];
        let _: () = msg_send![content_controller, addUserScript: seamless_script];

        if std::env::var_os("NMP_SMOKE").is_some() {
            let smoke_js = r#"
                window.addEventListener('DOMContentLoaded', function () {
                    setTimeout(function () {
                        var write = document.querySelector('#write') || document.querySelector('[contenteditable="true"]');
                        var inserted = false;
                        if (write) {
                            write.focus();
                            var sel = window.getSelection && window.getSelection();
                            if (sel && document.createRange) {
                                var range = document.createRange();
                                range.selectNodeContents(write);
                                range.collapse(false);
                                sel.removeAllRanges();
                                sel.addRange(range);
                            }
                            inserted = document.execCommand && document.execCommand('insertText', false, 'notypo-smoke');
                        }
                        setTimeout(function () {
                            window.bridge && window.bridge.callHandler('notypo.smoke', {
                                hasWrite: !!write,
                                activeElement: document.activeElement && (document.activeElement.id || document.activeElement.className || document.activeElement.tagName),
                                inserted: !!inserted,
                                text: write ? (write.innerText || write.textContent || '').slice(0, 120) : ''
                            });
                        }, 100);
                    }, 1000);
                });
            "#;
            let smoke_script: *mut Object = msg_send![class!(WKUserScript), alloc];
            let smoke_script: *mut Object = msg_send![
                smoke_script,
                initWithSource: nsstring(smoke_js)
                injectionTime: 0u64
                forMainFrameOnly: YES
            ];
            let _: () = msg_send![content_controller, addUserScript: smoke_script];
        }

        if std::env::var_os("NMP_LAYOUT_DUMP").is_some() {
            let layout_js = r#"
                window.addEventListener('load', function () {
                    setTimeout(function () {
                        var win = {innerW: window.innerWidth, innerH: window.innerHeight, outerW: window.outerWidth, outerH: window.outerHeight, scrollY: window.scrollY};
                        var body = document.body.getBoundingClientRect();
                        var content = (document.querySelector('content') || {}).getBoundingClientRect ? document.querySelector('content').getBoundingClientRect() : null;
                        var write = (document.querySelector('#write') || {}).getBoundingClientRect ? document.querySelector('#write').getBoundingClientRect() : null;
                        var titlebar = (document.querySelector('titlebar') || {}).getBoundingClientRect ? document.querySelector('titlebar').getBoundingClientRect() : null;
                        var html = document.documentElement.getBoundingClientRect();
                        var sidebar = document.querySelector('#typora-sidebar');
                        var bodyClasses = document.body.className;
                        var sidebarClasses = sidebar ? sidebar.className : 'null';
                        var sidebarRect = sidebar ? sidebar.getBoundingClientRect() : null;
                        window.bridge && window.bridge.callHandler('notypo.smoke', {
                            layout: {
                                win: win,
                                html: {x: html.x, y: html.y, w: html.width, h: html.height},
                                body: {x: body.x, y: body.y, w: body.width, h: body.height},
                                content: content ? {x: content.x, y: content.y, w: content.width, h: content.height} : null,
                                write: write ? {x: write.x, y: write.y, w: write.width, h: write.height} : null,
                                titlebar: titlebar ? {x: titlebar.x, y: titlebar.y, w: titlebar.width, h: titlebar.height} : null,
                                sidebar: sidebarRect ? {x: sidebarRect.x, y: sidebarRect.y, w: sidebarRect.width, h: sidebarRect.height, classes: sidebarClasses} : null,
                                bodyClasses: bodyClasses
                            }
                        });
                    }, 2000);
                });
            "#;
            let layout_script: *mut Object = msg_send![class!(WKUserScript), alloc];
            let layout_script: *mut Object = msg_send![
                layout_script,
                initWithSource: nsstring(layout_js)
                injectionTime: 0u64
                forMainFrameOnly: YES
            ];
            let _: () = msg_send![content_controller, addUserScript: layout_script];
        }

        if std::env::var_os("NMP_SAVE_SMOKE").is_some() {
            let save_smoke_js = r#"
                window.addEventListener('DOMContentLoaded', function () {
                    setTimeout(function () {
                        window.bridge && window.bridge.callHandler('document.setContent', '# Save Smoke\n\nSaved through fire-and-forget IPC.\n');
                        window.bridge && window.bridge.callHandler('document.save', null);
                        window.bridge && window.bridge.callHandler('notypo.smoke', { saveRequested: true });
                    }, 1200);
                });
            "#;
            let save_smoke_script: *mut Object = msg_send![class!(WKUserScript), alloc];
            let save_smoke_script: *mut Object = msg_send![
                save_smoke_script,
                initWithSource: nsstring(save_smoke_js)
                injectionTime: 0u64
                forMainFrameOnly: YES
            ];
            let _: () = msg_send![content_controller, addUserScript: save_smoke_script];
        }

        if std::env::var_os("NMP_TOGGLE_SMOKE").is_some() {
            let toggle_js = r#"
                window.addEventListener('load', function () {
                    setTimeout(function () {
                        var l = File.editor && File.editor.library;
                        function snap(){ return l ? {shown: l.isSidebarShown(), tab: l.getActiveTab()} : {lib:false}; }
                        var before = snap();
                        if (l) {
                            l.isSidebarShown() && l.getActiveTab()==='outline' ? l.hideSidebar() : l.showSidebar('outline');
                        }
                        setTimeout(function () {
                            var after = snap();
                            window.bridge && window.bridge.callHandler('notypo.smoke', { toggleTest: { before: before, after: after } });
                        }, 300);
                    }, 2500);
                });
            "#;
            let ts: *mut Object = msg_send![class!(WKUserScript), alloc];
            let ts: *mut Object = msg_send![ts, initWithSource: nsstring(toggle_js) injectionTime: 0u64 forMainFrameOnly: YES];
            let _: () = msg_send![content_controller, addUserScript: ts];
        }

        // The webview frame is expressed in the content view's coordinate
        // space (origin at bottom-left), NOT in screen coordinates. Reusing
        // the window's screen `rect` (origin 200,200) here would offset the
        // webview inside the content view, leaving an empty band at the bottom
        // of the window and clipping the top of the document. The content area
        // is `size` at the origin, so anchor the webview at (0, 0).
        let webview_frame = NSRect {
            origin: NSPoint { x: 0.0, y: 0.0 },
            size: rect.size,
        };
        let webview: *mut Object = msg_send![class!(WKWebView), alloc];
        let webview: *mut Object = msg_send![webview, initWithFrame: webview_frame configuration: cfg];
        WEBVIEW = webview;
        let ui_delegate: *mut Object = msg_send![class!(NotypoUIDelegate), new];
        let _: () = msg_send![webview, setUIDelegate: ui_delegate];
        let _: () = msg_send![webview, setAutoresizingMask: 18u64];

        let content: *mut Object = msg_send![win, contentView];
        let _: () = msg_send![content, addSubview: webview];

        let index_url: *mut Object =
            msg_send![class!(NSURL), fileURLWithPath: nsstring(&format!("{}/index.html", TYPE_MARK.as_str()))];
        let access_url: *mut Object = msg_send![class!(NSURL), fileURLWithPath: nsstring(&TYPE_MARK)];
        let _: () = msg_send![webview, loadFileURL: index_url allowingReadAccessToURL: access_url];
        let _: () = msg_send![win, makeKeyAndOrderFront: ptr::null::<Object>()];

        // Follow macOS Light/Dark Mode changes at runtime. The system posts
        // `AppleInterfaceThemeChangedNotification` on the distributed center when
        // the user toggles appearance; we re-resolve the theme and swap the
        // stylesheet in the webview.
        let this_ptr = _this as *const Object as *mut Object;
        let center: *mut Object = msg_send![class!(NSDistributedNotificationCenter), defaultCenter];
        let _: () = msg_send![
            center,
            addObserver: this_ptr
            selector: sel!(systemThemeChanged:)
            name: nsstring("AppleInterfaceThemeChangedNotification")
            object: ptr::null::<Object>()
        ];
    }
}

extern "C" fn system_theme_changed(_this: &Object, _cmd: Sel, _notification: *mut Object) {
    unsafe {
        apply_current_theme_to_webview();
    }
}

fn open_path_and_reload(path: String) -> bool {
    if Path::new(&path).is_dir() {
        set_mount_folder(Some(path));
        unsafe {
            push_mount_folder_to_typemark(true);
        }
        return true;
    }
    set_mount_folder_if_empty(Path::new(&path).parent().map(|p| p.to_string_lossy().into_owned()));
    match open_document_from(path) {
        Ok(()) => {
            unsafe {
                if WEBVIEW.is_null() {
                    // First launch: TypeMark will request document.loadData on init.
                    reload_webview_document();
                } else {
                    // WebView already loaded: push file content to TypeMark directly,
                    // matching Typora's native→JS push pattern.
                    push_file_to_typemark();
                }
            }
            true
        }
        Err(err) => {
            eprintln!("[notypo] open failed: {err}");
            false
        }
    }
}

/// Push the current document to TypeMark via `File.loadFile` + `File.setFilePath` +
/// `File.setDocumentState`. This mirrors Typora's native→JS push: instead of
/// reloading the webview, native calls the JS handlers directly to load content
/// and update file state.
unsafe fn push_file_to_typemark() {
    let (content, path, name, folder, state) = with_document(|doc| {
        (
            doc.content.clone(),
            doc.path.clone(),
            doc.display_name(),
            doc.folder(),
            doc.typemark_state(),
        )
    });
    // Load content directly as File.loadFile's third argument. The Mac branch
    // expects the same tuple returned by document.loadData: [content, snap, state].
    let load_data = serde_json::json!([content, null, state.clone()]);
    if let Ok(load_arg) = serde_json::to_string(&load_data) {
        evaluate_js(&format!(
            "if (window.File && File.loadFile) {{ File.loadFile(null, true, {load_arg}); }}"
        ));
    }
    // File.setFilePath([filePath, fileName, folderPath])
    if let Some(p) = path {
        call_js_handler(
            "File.setFilePath",
            serde_json::json!([p, name, folder]),
        );
    }
    // File.setDocumentState(state) — JS handler updates bundle state.
    call_js_handler("File.setDocumentState", state);
    // Update change count to cleared (fresh document).
    call_js_handler(
        "document.updateChangeCount",
        serde_json::json!(2), // NSChangeCleared
    );
    // Push recent files.
    push_recent_files_to_typemark();
    push_mount_folder_to_typemark(false);
}

/// Reload document content from disk and push to TypeMark.
/// Called when an external file change is detected. Reads the file,
/// updates DocumentState, and pushes `File.reloadContent` to JS.
///
/// No-ops when there is no backing file or when the on-disk content already
/// matches what we have in memory, so it's safe to call speculatively (e.g.
/// every time the app regains focus).
unsafe fn reload_content_from_disk() {
    let (path, current) = with_document(|doc| (doc.path.clone(), doc.content.clone()));
    let Some(path) = path else { return };
    let Ok(bytes) = std::fs::read(&path) else { return };
    let content = String::from_utf8_lossy(&bytes).into_owned();
    // Nothing changed on disk — avoid a pointless editor reload (which would
    // otherwise disrupt scroll/cursor position).
    if content == current {
        return;
    }
    with_document(|doc| {
        doc.content = content.clone();
        doc.dirty = false;
        doc.change_count = 0;
        doc.edited = false;
    });
    // The JS handler calls File.reloadContent.apply(File, data), so data must
    // be an argument array. Passing a bare string would spread it by character.
    call_js_handler("File.reloadContent", serde_json::json!([content]));
    call_js_handler(
        "document.updateChangeCount",
        serde_json::json!(4), // NSChangeAutoSaved
    );
    update_window_title();
}

extern "C" fn app_did_become_active(_this: &Object, _cmd: Sel, _notification: *mut Object) {
    // When the app regains focus, pick up any external edits to the current
    // file — but only if there are no unsaved changes, so we never clobber the
    // user's in-flight work. `reload_content_from_disk` itself no-ops when the
    // on-disk content is unchanged.
    let has_unsaved = with_document(|doc| doc.is_edited());
    if !has_unsaved {
        unsafe { reload_content_from_disk(); }
    }
}

extern "C" fn app_open_file(
    _this: &Object,
    _cmd: Sel,
    _app: *mut Object,
    filename: *mut Object,
) -> BOOL {
    unsafe {
        if let Some(path) = nsstr_to_string(filename) {
            if open_path_and_reload(path) {
                return YES;
            }
        }
        NO
    }
}

extern "C" fn app_open_files(
    _this: &Object,
    _cmd: Sel,
    app: *mut Object,
    filenames: *mut Object,
) {
    unsafe {
        let count: usize = msg_send![filenames, count];
        if count > 0 {
            let first: *mut Object = msg_send![filenames, objectAtIndex: 0usize];
            if let Some(path) = nsstr_to_string(first) {
                let _ = open_path_and_reload(path);
            }
        }
        let _: () = msg_send![app, replyToOpenOrPrint: 0u64];
    }
}

fn register_app_delegate() {
    let Some(mut cls) = ClassDecl::new("NotypoAppDelegate", class!(NSObject)) else {
        return;
    };
    unsafe {
        cls.add_method(
            sel!(applicationDidFinishLaunching:),
            did_finish_launching as extern "C" fn(&Object, Sel, *mut Object),
        );
        cls.add_method(
            sel!(applicationDidBecomeActive:),
            app_did_become_active as extern "C" fn(&Object, Sel, *mut Object),
        );
        cls.add_method(
            sel!(systemThemeChanged:),
            system_theme_changed as extern "C" fn(&Object, Sel, *mut Object),
        );
        cls.add_method(
            sel!(application:openFile:),
            app_open_file as extern "C" fn(&Object, Sel, *mut Object, *mut Object) -> BOOL,
        );
        cls.add_method(
            sel!(application:openFiles:),
            app_open_files as extern "C" fn(&Object, Sel, *mut Object, *mut Object),
        );
    }
    cls.register();
}

/// Set the Dock/app icon at runtime. In a packaged bundle the icon comes from
/// `Contents/Resources/app-icon.png` (the .icns is used by LaunchServices for
/// Finder/Dock); in dev (`cargo run`) it falls back to
/// `<manifest>/assets/app-icon.png`. This keeps the Dock icon consistent in
/// both modes and overrides the default Rust/Hornbeam icon.
unsafe fn set_app_icon(app: *mut Object) {
    let candidate = std::env::current_exe().ok().and_then(|exe| {
        exe.parent().and_then(|macos| macos.parent()).map(|contents| {
            contents.join("Resources").join("app-icon.png")
        })
    }).or_else(|| {
        Path::new(TYPE_MARK_DEV).parent().map(|assets| assets.join("app-icon.png"))
    });
    let Some(path) = candidate else { return; };
    if !path.is_file() {
        return;
    }
    let path_str = path.to_string_lossy();
    let img: *mut Object = msg_send![class!(NSImage), alloc];
    let img: *mut Object = msg_send![img, initWithContentsOfFile: nsstring(&path_str)];
    if img.is_null() {
        let _: () = msg_send![img, release];
        return;
    }
    let _: () = msg_send![app, setApplicationIconImage: img];
    let _: () = msg_send![img, release];
}

fn load_document_from_cli() {
    let Some(path) = std::env::args().nth(1) else {
        return;
    };
    if let Err(err) = open_document_from(path.clone()) {
        eprintln!("[notypo] failed to open CLI path {path}: {err}");
    }
}

fn main() {
    load_document_from_cli();
    unsafe {
        register_app_delegate();
        register_menu_target();
        let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        set_app_icon(app);
        let _: () = msg_send![app, setActivationPolicy: 0u64];
        install_main_menu(app);
        let delegate: *mut Object = msg_send![class!(NotypoAppDelegate), new];
        let _: () = msg_send![app, setDelegate: delegate];
        let _: () = msg_send![app, run];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    #[test]
    fn document_load_shape_and_save_write_disk() {
        let _guard = TEST_LOCK.lock().expect("test lock poisoned");
        let path = std::env::temp_dir().join(format!("notypo-doc-{}.md", std::process::id()));
        with_document(|doc| {
            doc.path = Some(path.to_string_lossy().into_owned());
            doc.content = "# Saved by notypo\n".to_string();
            doc.encoding = "utf-8".to_string();
            doc.dirty = true;
        });

        let load = document_load_response();
        assert_eq!(load[0], "# Saved by notypo\n");
        assert_eq!(load[1], serde_json::Value::Null);
        assert_eq!(load[2]["currentFilePath"], path.to_string_lossy().to_string());
        assert_eq!(load[2]["fileEncode"], "utf-8");

        let saved = save_document_to(None).expect("save current document");
        assert_eq!(saved, path.to_string_lossy().to_string());
        assert_eq!(
            std::fs::read_to_string(&path).expect("saved markdown"),
            "# Saved by notypo\n"
        );
        let _ = std::fs::remove_file(path);
        let recent = recent_files_json();
        assert_eq!(recent[0]["path"], saved);
    }

    #[test]
    fn folder_tree_json_lists_files_and_folders() {
        let _guard = TEST_LOCK.lock().expect("test lock poisoned");
        let root = std::env::temp_dir().join(format!("notypo-tree-{}", std::process::id()));
        let child = root.join("child");
        std::fs::create_dir_all(&child).expect("create child directory");
        std::fs::write(root.join("note.md"), "# Note\n").expect("create markdown file");
        std::fs::write(child.join("nested.md"), "# Nested\n").expect("create nested markdown file");

        let tree = folder_tree_json(&root.to_string_lossy());
        assert_eq!(tree["isDirectory"], true);
        assert_eq!(tree["path"], root.to_string_lossy().to_string());
        assert!(tree["subdir"].as_array().unwrap().iter().any(|node| node["name"] == "child"));
        assert!(tree["content"].as_array().unwrap().iter().any(|node| node["name"] == "note.md"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn switch_document_target_opens_file() {
        let _guard = TEST_LOCK.lock().expect("test lock poisoned");
        let path = std::env::temp_dir().join(format!("notypo-open-{}.md", std::process::id()));
        std::fs::write(&path, "# Opened from tree\n").expect("create markdown file");

        let response = bridge_response(
            "controller.switchDocumentTarget",
            &serde_json::json!({ "data": path.to_string_lossy().to_string() }),
        );

        assert_eq!(response, true);
        let load = document_load_response();
        assert_eq!(load[0], "# Opened from tree\n");
        assert_eq!(load[2]["currentFilePath"], path.to_string_lossy().to_string());

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn opening_nested_file_keeps_existing_mount_folder() {
        let _guard = TEST_LOCK.lock().expect("test lock poisoned");
        let root = std::env::temp_dir().join(format!("notypo-mount-{}", std::process::id()));
        let child = root.join("child");
        let path = child.join("nested.md");
        std::fs::create_dir_all(&child).expect("create child directory");
        std::fs::write(&path, "# Nested\n").expect("create nested markdown file");
        set_mount_folder(Some(root.to_string_lossy().into_owned()));

        assert!(open_path_and_reload(path.to_string_lossy().to_string()));
        assert_eq!(current_mount_folder(), Some(root.to_string_lossy().into_owned()));
        assert_eq!(document_load_response()[0], "# Nested\n");

        set_mount_folder(None);
        let _ = std::fs::remove_dir_all(root);
    }
}
