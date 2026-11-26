#![allow(deprecated)]
//! Folio — a fast, local-first notes app. Pages are plain Markdown files
//! in a folder you choose, so any sync tool can share them. No network.
//!
//! The same code runs as a desktop app and, compiled to WebAssembly, as an
//! installable offline web app (PWA) for iPhone and iPad (see `web/`).

pub mod app;
pub mod covers;
pub mod db;
pub mod editor;
pub mod model;
pub mod platform;
pub mod store;
pub mod theme;
pub mod vfs;
pub mod zip;

pub use app::App;

#[cfg(not(target_arch = "wasm32"))]
pub fn run_native() -> eframe::Result {
    use eframe::egui;
    let args: Vec<String> = std::env::args().collect();
    let shot = args
        .iter()
        .position(|a| a == "--screenshot")
        .and_then(|i| args.get(i + 1))
        .map(std::path::PathBuf::from);

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Folio")
        .with_icon(std::sync::Arc::new(app_icon()))
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([360.0, 400.0]);

    let mut options = eframe::NativeOptions::default();
    if shot.is_some() {
        viewport = viewport.with_active(false);
        #[cfg(target_os = "windows")]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
        #[cfg(target_os = "macos")]
        {
            options.event_loop_builder = Some(Box::new(|b| {
                use winit::platform::macos::EventLoopBuilderExtMacOS;
                b.with_activate_ignoring_other_apps(false);
            }));
        }
        // Test hook: FOLIO_SHOT_SIZE=WxH (e.g. an iPhone-sized window).
        if let Ok(v) = std::env::var("FOLIO_SHOT_SIZE") {
            let mut it = v.split('x').filter_map(|n| n.trim().parse::<f32>().ok());
            if let (Some(w), Some(h)) = (it.next(), it.next()) {
                viewport = viewport.with_inner_size([w, h]);
            }
        }
    }
    options.viewport = viewport;

    eframe::run_native(
        "Folio",
        options,
        Box::new(move |cc| {
            theme::install_fonts(&cc.egui_ctx);
            Ok(Box::new(App::new(cc, shot)))
        }),
    )
}

/// The app icon (drawn by `examples/make_icons.rs`), shown in the Dock / taskbar.
#[cfg(not(target_arch = "wasm32"))]
fn app_icon() -> eframe::egui::IconData {
    let bytes = include_bytes!("../assets/icon-desktop.png");
    match image::load_from_memory(bytes) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            eframe::egui::IconData { width: rgba.width(), height: rgba.height(), rgba: rgba.into_raw() }
        }
        Err(_) => eframe::egui::IconData::default(),
    }
}

// ------------------------------------------------------------------ web entry points

#[cfg(target_arch = "wasm32")]
mod web {
    use wasm_bindgen::prelude::*;

    /// The page's script hands over each saved file before starting the app.
    #[wasm_bindgen]
    pub fn folio_preload(path: String, data: Vec<u8>, mtime: f64) {
        crate::vfs::preload(path, data, mtime as u64);
    }

    /// A file the person chose in a file picker.
    #[wasm_bindgen]
    pub fn folio_picked(id: u32, name: String, data: Vec<u8>) {
        crate::platform::deliver(id, name, data);
    }

    /// The folder picked in the web app: its files arrive one by one, then `folio_folder_end`.
    #[wasm_bindgen]
    pub fn folio_folder_begin() {
        crate::platform::folder_begin();
    }

    #[wasm_bindgen]
    pub fn folio_folder_file(rel: String, data: Vec<u8>, mtime: f64) {
        crate::platform::folder_file(rel, data, mtime as u64);
    }

    #[wasm_bindgen]
    pub fn folio_folder_end() {
        crate::platform::folder_end(&crate::store::default_workspace());
    }

    /// Which folder the notes are in, and whether changes are saved there.
    #[wasm_bindgen]
    pub fn folio_folder_state(name: String, mode: u8, can_write: bool, provider: String, status: String) {
        crate::platform::set_folder_state(crate::platform::FolderState { name, mode, can_write, provider, status });
    }

    /// Which cloud services have an app key in web/config.js.
    #[wasm_bindgen]
    pub fn folio_cloud_config(dropbox: bool, onedrive: bool, google: bool) {
        crate::platform::set_cloud_ready([dropbox, onedrive, google]);
    }

    /// True while some change has not been saved yet (the page warns before closing).
    #[wasm_bindgen]
    pub fn folio_has_unsaved() -> bool {
        crate::platform::unsaved()
    }

    /// A file changed in the linked folder (for example synced by Google Drive).
    #[wasm_bindgen]
    pub fn folio_external_put(path: String, data: Vec<u8>, mtime: f64) {
        crate::vfs::external_put(path, data, mtime as u64);
    }

    #[wasm_bindgen]
    pub fn folio_external_remove(path: String) {
        crate::vfs::external_remove(&path);
    }

    /// True when a tap at (x, y) lands on a text field (to open the keyboard right away).
    #[wasm_bindgen]
    pub fn folio_editable_at(x: f32, y: f32) -> bool {
        crate::platform::in_keyboard_zone(x, y)
    }

    #[wasm_bindgen]
    pub fn folio_start(canvas_id: String, touch: bool) {
        crate::app::set_touch_device(touch);
        wasm_bindgen_futures::spawn_local(async move {
            let document = web_sys::window().and_then(|w| w.document()).expect("no document");
            let canvas = document
                .get_element_by_id(&canvas_id)
                .and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok())
                .expect("canvas not found");
            let options = eframe::WebOptions::default();
            let result = eframe::WebRunner::new()
                .start(
                    canvas,
                    options,
                    Box::new(|cc| {
                        crate::theme::install_fonts(&cc.egui_ctx);
                        Ok(Box::new(crate::App::new(cc, None)))
                    }),
                )
                .await;
            if let Err(e) = result {
                web_sys::console::error_1(&e);
            }
        });
    }
}
