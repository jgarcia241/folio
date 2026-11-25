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
