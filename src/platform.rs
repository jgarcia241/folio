//! What differs between the desktop app and the web / installed PWA:
//! choosing files, opening links, saving an export, showing files.

use std::sync::Mutex;

/// What a chosen file is for.
#[derive(Clone, Debug, PartialEq)]
pub enum PickKind {
    /// A picture for an image block.
    BlockImage { page: String, block: u64 },
    /// A cover picture for a page.
    Cover { page: String },
    /// Import notes: .md files, pictures or a .zip of a notes folder.
    Import,
}

pub struct Picked {
    pub kind: PickKind,
    pub name: String,
    pub bytes: Vec<u8>,
}

static PICKED: Mutex<Vec<Picked>> = Mutex::new(Vec::new());

/// Web: the folder the notes live in (see `web/index.html`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FolderState {
    /// Folder name as shown in Files / the file picker ("" = notes only on this device).
    pub name: String,
    /// 0 = none, 1 = linked (changes are saved in the folder), 2 = read from Files
    /// (iPhone/iPad: Safari can't save into it), 3 = linked but needs a tap to reconnect.
    pub mode: u8,
    /// The browser can save straight into a folder (File System Access).
    pub can_write: bool,
    /// "Dropbox", "OneDrive", "Google Drive", or "" for a folder on the device.
    pub provider: String,
    /// Short sync status, e.g. "Synced" or "Offline · 2 changes waiting".
    pub status: String,
}

/// Which cloud services have an app key in `web/config.js` (Dropbox, OneDrive, Google Drive).
static CLOUD_READY: Mutex<[bool; 3]> = Mutex::new([false; 3]);
static UNSAVED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_cloud_ready(r: [bool; 3]) {
    *CLOUD_READY.lock().unwrap() = r;
}
pub fn cloud_ready() -> [bool; 3] {
    *CLOUD_READY.lock().unwrap()
}
/// The app has changes not written yet (asked by the web page before it closes).
pub fn set_unsaved(on: bool) {
    UNSAVED.store(on, std::sync::atomic::Ordering::Relaxed);
}
pub fn unsaved() -> bool {
    UNSAVED.load(std::sync::atomic::Ordering::Relaxed)
}

static FOLDER: Mutex<Option<FolderState>> = Mutex::new(None);
/// Set when a folder's files replaced the notes: the app must re-read them.
static RELOAD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static INCOMING: Mutex<Vec<(String, Vec<u8>, u64)>> = Mutex::new(Vec::new());

pub fn folder_state() -> FolderState {
    FOLDER.lock().unwrap().clone().unwrap_or_default()
}

pub fn set_folder_state(st: FolderState) {
    *FOLDER.lock().unwrap() = Some(st);
}

pub fn take_reload() -> bool {
    RELOAD.swap(false, std::sync::atomic::Ordering::SeqCst)
}

pub fn folder_begin() {
    INCOMING.lock().unwrap().clear();
}

pub fn folder_file(rel: String, bytes: Vec<u8>, mtime: u64) {
    INCOMING.lock().unwrap().push((rel, bytes, mtime));
}

/// All files of the chosen folder arrived: swap them in.
pub fn folder_end(root: &std::path::Path) {
    let files = std::mem::take(&mut *INCOMING.lock().unwrap());
    #[cfg(target_arch = "wasm32")]
    crate::vfs::replace_root(root, files);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (root, files);
    RELOAD.store(true, std::sync::atomic::Ordering::SeqCst);
}
static PENDING: Mutex<Vec<(u32, PickKind)>> = Mutex::new(Vec::new());
/// Screen areas (in points) where a tap should bring up the keyboard.
static KEYBOARD_ZONES: Mutex<Vec<[f32; 4]>> = Mutex::new(Vec::new());

pub fn take_picked() -> Vec<Picked> {
    std::mem::take(&mut *PICKED.lock().unwrap())
}

pub const IMAGE_EXTS: [&str; 5] = ["png", "jpg", "jpeg", "gif", "webp"];

pub fn clear_keyboard_zones() {
    KEYBOARD_ZONES.lock().unwrap().clear();
}

/// Mark a text field's area so the first tap on it opens the iPhone/iPad keyboard.
pub fn keyboard_zone(r: eframe::egui::Rect) {
    if cfg!(target_arch = "wasm32") {
        KEYBOARD_ZONES.lock().unwrap().push([r.min.x, r.min.y, r.max.x, r.max.y]);
    }
}

pub fn in_keyboard_zone(x: f32, y: f32) -> bool {
    KEYBOARD_ZONES.lock().unwrap().iter().any(|r| x >= r[0] && x <= r[2] && y >= r[1] && y <= r[3])
}

// ------------------------------------------------------------------ desktop

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use super::*;

    pub const IS_WEB: bool = false;

    pub fn pick(kind: PickKind) {
        let dialog = rfd::FileDialog::new();
        let files = match &kind {
            PickKind::Import => dialog.set_title("Import notes").add_filter("Notes, pictures or .zip", &["md", "zip", "png", "jpg", "jpeg", "gif", "webp"]).pick_files().unwrap_or_default(),
            _ => dialog.set_title("Choose a picture").add_filter("Pictures", &IMAGE_EXTS).pick_file().into_iter().collect(),
        };
        let mut q = PICKED.lock().unwrap();
        for f in files {
            if let Ok(bytes) = std::fs::read(&f) {
                let name = f.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                q.push(Picked { kind: kind.clone(), name, bytes });
            }
        }
        let _ = &PENDING;
    }

    pub fn pick_folder() -> Option<std::path::PathBuf> {
        rfd::FileDialog::new().set_title("Choose a folder for your notes (e.g. inside Google Drive or OneDrive)").pick_folder()
    }

    /// Save bytes the person exports (asks where).
    pub fn save_file(name: &str, bytes: &[u8]) -> Result<bool, String> {
        match rfd::FileDialog::new().set_file_name(name).save_file() {
            Some(p) => std::fs::write(&p, bytes).map(|_| true).map_err(|e| e.to_string()),
            None => Ok(false),
        }
    }

    pub fn open_url(url: &str) {
        #[cfg(target_os = "macos")]
        let _ = std::process::Command::new("open").arg(url).spawn();
        #[cfg(target_os = "windows")]
        let _ = std::process::Command::new("cmd").args(["/C", "start", "", url]).spawn();
        #[cfg(all(unix, not(target_os = "macos")))]
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }

    pub fn connect_cloud(_kind: &str) {}
    pub fn sync_now() {}
    pub fn choose_notes_folder() {}
    pub fn reconnect_notes_folder() {}
    pub fn forget_notes_folder() {}

    pub fn reveal(path: &std::path::Path) {
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("open").arg("-R").arg(path).spawn();
        }
        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("explorer").arg(format!("/select,{}", path.display())).spawn();
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            let dir = if path.is_dir() { path } else { path.parent().unwrap_or(path) };
            let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
        }
    }
}

// ------------------------------------------------------------------ web

#[cfg(target_arch = "wasm32")]
mod imp {
    use super::*;
    use wasm_bindgen::prelude::*;

    pub const IS_WEB: bool = true;

    // Functions provided by `web/index.html` (window.folioHost).
    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_namespace = folioHost, js_name = put)]
        fn js_put(path: &str, data: &[u8], mtime: f64);
        #[wasm_bindgen(js_namespace = folioHost, js_name = remove)]
        fn js_remove(path: &str);
        #[wasm_bindgen(js_namespace = folioHost, js_name = pick)]
        fn js_pick(id: u32, accept: &str, multiple: bool);
        #[wasm_bindgen(js_namespace = folioHost, js_name = share)]
        fn js_share(name: &str, data: &[u8], mime: &str);
        #[wasm_bindgen(js_namespace = folioHost, js_name = openUrl)]
        fn js_open_url(url: &str);
        #[wasm_bindgen(js_namespace = folioHost, js_name = pickFolder)]
        fn js_pick_folder();
        #[wasm_bindgen(js_namespace = folioHost, js_name = reconnectFolder)]
        fn js_reconnect_folder();
        #[wasm_bindgen(js_namespace = folioHost, js_name = forgetFolder)]
        fn js_forget_folder();
        #[wasm_bindgen(js_namespace = folioHost, js_name = connectCloud)]
        fn js_connect_cloud(kind: &str);
        #[wasm_bindgen(js_namespace = folioHost, js_name = syncNow)]
        fn js_sync_now();
    }

    /// Sign in to Dropbox / OneDrive / Google Drive ("dropbox", "onedrive", "google").
    pub fn connect_cloud(kind: &str) {
        js_connect_cloud(kind);
    }
    pub fn sync_now() {
        js_sync_now();
    }

    /// Choose the folder that holds the notes (Files app / folder picker).
    pub fn choose_notes_folder() {
        js_pick_folder();
    }
    pub fn reconnect_notes_folder() {
        js_reconnect_folder();
    }
    pub fn forget_notes_folder() {
        js_forget_folder();
    }

    pub fn host_put(path: &str, data: &[u8], mtime: f64) {
        js_put(path, data, mtime);
    }
    pub fn host_remove(path: &str) {
        js_remove(path);
    }

    pub fn pick(kind: PickKind) {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
        let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let (accept, multiple) = match &kind {
            PickKind::Import => (".md,.markdown,.zip,image/*,text/markdown,application/zip", true),
            _ => ("image/*", false),
        };
        PENDING.lock().unwrap().push((id, kind));
        js_pick(id, accept, multiple);
    }

    /// Called by the page's script with each chosen file.
    pub fn deliver(id: u32, name: String, bytes: Vec<u8>) {
        let kind = PENDING.lock().unwrap().iter().find(|(i, _)| *i == id).map(|(_, k)| k.clone());
        if let Some(kind) = kind {
            PICKED.lock().unwrap().push(Picked { kind, name, bytes });
        }
    }

    pub fn pick_folder() -> Option<std::path::PathBuf> {
        None
    }

    /// Hand the file to the share sheet (iPhone/iPad: "Save to Files") or download it.
    pub fn save_file(name: &str, bytes: &[u8]) -> Result<bool, String> {
        let mime = if name.ends_with(".zip") { "application/zip" } else { "application/octet-stream" };
        js_share(name, bytes, mime);
        Ok(true)
    }

    pub fn open_url(url: &str) {
        js_open_url(url);
    }

    pub fn reveal(_path: &std::path::Path) {}
}

pub use imp::*;
