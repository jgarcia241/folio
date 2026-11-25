//! Desktop entry point. The app itself lives in the library (`src/lib.rs`).

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    folio::run_native()
}

#[cfg(target_arch = "wasm32")]
fn main() {}
