//! The app's state and its frame loop (shared by desktop and web).

use crate::{editor, model, store, theme};
use eframe::egui;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

static TOUCH_DEVICE: AtomicBool = AtomicBool::new(false);

/// Set by the web start-up code when the device has a touch screen.
pub fn set_touch_device(on: bool) {
    TOUCH_DEVICE.store(on, Ordering::Relaxed);
}

/// Below this width (points) the layout switches to the phone layout.
pub const NARROW: f32 = 760.0;

pub struct Shot {
    pub path: PathBuf,
    pub frames: u32,
    pub requested: bool,
    pub done: Arc<AtomicBool>,
}

pub struct App {
    pub ws: store::Workspace,
    pub settings: store::Settings,
    pub current: Option<String>,
    pub ed: editor::EditorState,
    pub expanded: std::collections::HashSet<String>,
    pub search: Option<(String, usize)>,
    pub show_trash: bool,
    pub sidebar_open: bool,
    pub full_width: bool,
    pub last_poll: f64,
    pub shot: Option<Shot>,
    pub status: Option<(String, f64)>,
    /// Pictures loaded from the workspace (None = could not be read).
    pub textures: std::collections::HashMap<String, Option<egui::TextureHandle>>,
    /// Touch screen (iPhone/iPad): no hover, tap to edit, drag to scroll.
    pub touch: bool,
    /// Phone-sized window: sidebar slides over the page, tighter margins.
    pub narrow: bool,
    first_frame: bool,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, shot: Option<PathBuf>) -> Self {
        let mut settings = store::Settings::load();
        let ws = if shot.is_some() {
            // Screenshot runs use a throw-away sample workspace so nothing real is touched.
            let base = std::env::temp_dir().join(format!("folio-shot-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&base);
            let dir = base.join("My Notes");
            let mut ws = store::Workspace::open(dir);
            ws.seed_welcome();
            settings.dark = false;
            ws
        } else {
            let root = settings.workspace.clone().unwrap_or_else(store::default_workspace);
            let mut ws = store::Workspace::open(root.clone());
            if ws.entries.is_empty() {
                ws.seed_welcome();
            }
            settings.remember(&root);
            settings.save();
            ws
        };
        theme::apply(&cc.egui_ctx, settings.dark);
        let current = ws.children("").first().map(|p| p.id.clone());
        let mut app = App {
            ws,
            settings,
            current: None,
            ed: editor::EditorState::default(),
            expanded: Default::default(),
            search: None,
            show_trash: false,
            sidebar_open: true,
            full_width: false,
            last_poll: 0.0,
            shot: shot.map(|path| Shot { path, frames: 0, requested: false, done: Arc::new(AtomicBool::new(false)) }),
            status: None,
            textures: Default::default(),
            touch: TOUCH_DEVICE.load(Ordering::Relaxed) || std::env::var("FOLIO_SHOT_TOUCH").is_ok(),
            narrow: false,
            first_frame: true,
        };
        if let Some(id) = current {
            app.open_page(&id);
        }
        // Test hooks for screenshot runs: open another sample page or the board view.
        if app.shot.is_some() {
            if let Ok(title) = std::env::var("FOLIO_SHOT_PAGE") {
                if let Some(id) = app.ws.entries.iter().find(|e| e.page.title == title).map(|e| e.page.id.clone()) {
                    app.open_page(&id);
                }
            }
            if std::env::var("FOLIO_SHOT_TAIL").is_ok() {
                // Show only the end of the page (the screenshot workspace is a throw-away copy).
                if let Some(e) = app.current.clone().and_then(|id| app.ws.get_mut(&id)) {
                    let n = e.page.blocks.len();
                    e.page.blocks.drain(..n.saturating_sub(7));
                    e.page.cover = 0;
                }
            }
            if let Ok(which) = std::env::var("FOLIO_SHOT_MENU") {
                let pos = egui::pos2(300.0, 260.0);
                let kind = match which.as_str() {
                    "page" => app.current.clone().map(editor::MenuKind::Page),
                    "cover" => Some(editor::MenuKind::Cover),
                    "filter" => Some(editor::MenuKind::DbFilter),
                    "block" => app.current.as_ref().and_then(|id| app.ws.get(id)).and_then(|e| e.page.blocks.get(1)).map(|b| editor::MenuKind::Block(b.id)),
                    "paste" => app.current.as_ref().and_then(|id| app.ws.get(id)).and_then(|e| e.page.blocks.first()).map(|b| editor::MenuKind::Paste { block: b.id, url: "https://example.com".into(), start: 0 }),
                    _ => None,
                };
                if let Some(kind) = kind {
                    app.ed.menu = Some(editor::Menu { kind, pos, frame: 1_000_000 });
                }
            }
            if std::env::var("FOLIO_SHOT_FILTER").is_ok() {
                for e in &mut app.ws.entries {
                    if let Some(db) = &mut e.page.db {
                        db.filters.push(model::Filter { col: "Priority".into(), op: "is".into(), value: "High".into() });
                        db.sorts.push(("Due".into(), true));
                    }
                }
            }
            if std::env::var("FOLIO_SHOT_VIEW").as_deref() == Ok("board") {
                for e in &mut app.ws.entries {
                    if let Some(db) = &mut e.page.db {
                        db.view = model::View::Board;
                    }
                }
            }
        }
        // Expand the tree a little so sub-pages are visible.
        let ids: Vec<String> = app.ws.children("").iter().map(|p| p.id.clone()).collect();
        for id in ids {
            if !app.ws.children(&id).is_empty() {
                app.expanded.insert(id);
            }
        }
        app
    }

    pub fn open_page(&mut self, id: &str) {
        self.current = Some(id.to_string());
        self.show_trash = false;
        self.ed = editor::EditorState::default();
        for anc in self.ws.path_to(id) {
            if anc != id {
                self.expanded.insert(anc);
            }
        }
    }

    pub fn new_page(&mut self, parent: &str, now: f64) -> String {
        let id = self.ws.create("", parent, now);
        if !parent.is_empty() {
            self.expanded.insert(parent.to_string());
        }
        self.open_page(&id);
        self.ed.focus_title = true;
        id
    }

    pub fn new_page_opened(&mut self, id: &str) {
        self.open_page(id);
        self.ed.focus_title = true;
    }

    pub fn switch_workspace(&mut self, root: PathBuf) {
        self.ws.flush(0.0, true);
        let mut ws = store::Workspace::open(root.clone());
        if ws.entries.is_empty() {
            ws.seed_welcome();
        }
        self.ws = ws;
        self.settings.remember(&root);
        self.settings.save();
        self.current = None;
        if let Some(id) = self.ws.children("").first().map(|p| p.id.clone()) {
            self.open_page(&id);
        }
    }

    /// Files chosen in a file picker (desktop dialog or iPhone/iPad Files app).
    fn handle_picked(&mut self, now: f64) {
        use crate::platform::PickKind;
        let picked = crate::platform::take_picked();
        if picked.is_empty() {
            return;
        }
        let mut imports = Vec::new();
        for p in picked {
            match p.kind {
                PickKind::BlockImage { page, block } => match self.ws.import_asset(&p.name, &p.bytes) {
                    Ok(rel) => {
                        if let Some(e) = self.ws.get_mut(&page) {
                            if let Some(b) = e.page.blocks.iter_mut().find(|b| b.id == block) {
                                b.extra = rel;
                            }
                        }
                        self.ws.touch(&page, now);
                    }
                    Err(e) => self.set_status(format!("Could not add picture: {e}"), now),
                },
                PickKind::Cover { page } => match self.ws.import_asset(&p.name, &p.bytes) {
                    Ok(rel) => {
                        if let Some(e) = self.ws.get_mut(&page) {
                            e.page.cover_image = rel;
                            if e.page.cover == 0 {
                                e.page.cover = 1;
                            }
                        }
                        self.ws.touch(&page, now);
                    }
                    Err(e) => self.set_status(format!("Could not add picture: {e}"), now),
                },
                PickKind::Import => imports.push((p.name, p.bytes)),
            }
        }
        if !imports.is_empty() {
            match self.ws.import_files(imports) {
                Ok(n) => self.set_status(format!("Imported {n} file{}", if n == 1 { "" } else { "s" }), now),
                Err(e) => self.set_status(format!("Import failed: {e}"), now),
            }
        }
    }

    pub fn set_status(&mut self, s: impl Into<String>, now: f64) {
        self.status = Some((s.into(), now));
    }

    fn shortcuts(&mut self, ctx: &egui::Context, now: f64) {
        use egui::{Key, KeyboardShortcut, Modifiers};
        let k = |m, key| KeyboardShortcut::new(m, key);
        if ctx.input_mut(|i| i.consume_shortcut(&k(Modifiers::COMMAND, Key::K)) || i.consume_shortcut(&k(Modifiers::COMMAND, Key::P))) {
            self.search = Some((String::new(), 0));
        }
        if ctx.input_mut(|i| i.consume_shortcut(&k(Modifiers::COMMAND, Key::N))) {
            self.new_page("", now);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&k(Modifiers::COMMAND, Key::S))) {
            self.ws.flush(now, true);
            self.set_status("Saved", now);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&k(Modifiers::COMMAND, Key::Backslash))) {
            self.sidebar_open = !self.sidebar_open;
        }
    }

    fn screenshot_tick(&mut self, ctx: &egui::Context) {
        let Some(shot) = &mut self.shot else { return };
        ctx.request_repaint();
        shot.frames += 1;
        if shot.done.load(Ordering::SeqCst) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if shot.frames >= 120 && !shot.requested {
            shot.requested = true;
            let path = shot.path.clone();
            let done = shot.done.clone();
            let c2 = ctx.clone();
            ctx.request_screenshot(move |img: Arc<egui::ColorImage>| {
                let [w, h] = img.size;
                let mut bytes = Vec::with_capacity(w * h * 4);
                for p in &img.pixels {
                    bytes.extend_from_slice(&[p.r(), p.g(), p.b(), 255]);
                }
                if let Some(dir) = path.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                match image::RgbaImage::from_raw(w as u32, h as u32, bytes) {
                    Some(buf) => {
                        if let Err(e) = buf.save(&path) {
                            eprintln!("could not save screenshot: {e}");
                        }
                    }
                    None => eprintln!("bad screenshot buffer"),
                }
                done.store(true, Ordering::SeqCst);
                c2.request_repaint();
            });
        }
        if shot.frames > 2000 {
            eprintln!("screenshot timed out");
            std::process::exit(1);
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.ws.flush(0.0, true);
        if let Some(dir) = self.shot.as_ref().and_then(|_| self.ws.root.parent().map(|p| p.to_path_buf())) {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

impl eframe::App for App {
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        // Test hook: FOLIO_SHOT_HOVER=x,y puts a pretend mouse pointer there in screenshot runs.
        if self.shot.is_some() {
            if let Ok(v) = std::env::var("FOLIO_SHOT_HOVER") {
                let mut it = v.split(',').filter_map(|n| n.trim().parse::<f32>().ok());
                if let (Some(x), Some(y)) = (it.next(), it.next()) {
                    raw.events.push(egui::Event::PointerMoved(egui::pos2(x, y)));
                }
            }
            // FOLIO_SHOT_CLICK=x,y clicks there once, early in the run.
            if let Ok(v) = std::env::var("FOLIO_SHOT_CLICK") {
                let mut it = v.split(',').filter_map(|n| n.trim().parse::<f32>().ok());
                if let (Some(x), Some(y)) = (it.next(), it.next()) {
                    let pos = egui::pos2(x, y);
                    let f = self.shot.as_ref().map(|s| s.frames).unwrap_or(0);
                    let button = |pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
                    match f {
                        20 => raw.events.push(egui::Event::PointerMoved(pos)),
                        21 => raw.events.push(button(true)),
                        22 => raw.events.push(button(false)),
                        _ => {}
                    }
                }
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        let c = theme::colors(self.settings.dark);
        crate::platform::clear_keyboard_zones();

        let screen = ctx.content_rect();
        self.narrow = screen.width() < NARROW;
        if ctx.input(|i| i.any_touches()) {
            self.touch = true;
        }
        if self.first_frame {
            self.first_frame = false;
            if self.narrow && std::env::var("FOLIO_SHOT_SIDEBAR").is_err() {
                self.sidebar_open = false;
            }
        }
        self.handle_picked(now);
        let page_before = (self.current.clone(), self.show_trash);

        if self.shot.is_none() {
            self.shortcuts(&ctx, now);
        }

        // Pick up edits that arrive through Drive / OneDrive / another device.
        if now - self.last_poll > 1.5 {
            self.last_poll = now;
            if self.ws.poll_external() {
                if let Some(cur) = &self.current {
                    if self.ws.get(cur).is_none() {
                        self.current = None;
                    }
                }
            }
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(1600));

        if self.sidebar_open && self.narrow {
            // Phone: the sidebar slides over the page; tap outside to close it.
            let w = (screen.width() * 0.86).min(320.0);
            let mut close = false;
            egui::Area::new(egui::Id::new("sidebar_dim")).order(egui::Order::Middle).fixed_pos(screen.min).show(&ctx, |ui| {
                let r = ui.allocate_rect(screen, egui::Sense::click());
                ui.painter().rect_filled(screen, 0.0, egui::Color32::from_black_alpha(70));
                if r.clicked() {
                    close = true;
                }
            });
            egui::Area::new(egui::Id::new("sidebar_drawer")).order(egui::Order::Middle).fixed_pos(screen.min).interactable(true).show(&ctx, |ui| {
                let rect = egui::Rect::from_min_size(screen.min, egui::vec2(w, screen.height()));
                ui.painter().rect_filled(rect, 0.0, c.sidebar);
                let _ = ui.allocate_rect(rect, egui::Sense::hover());
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(egui::vec2(8.0, 10.0))).layout(egui::Layout::top_down(egui::Align::Min)));
                crate::editor::sidebar(self, &mut child, now);
            });
            if close {
                self.sidebar_open = false;
            }
        } else if self.sidebar_open {
            egui::Panel::left("sidebar")
                .resizable(true)
                .default_size(self.settings.sidebar_width)
                .size_range(200.0..=420.0)
                .show_separator_line(false)
                .frame(egui::Frame::new().fill(c.sidebar).inner_margin(egui::Margin::symmetric(6, 8)))
                .show(ui, |ui| crate::editor::sidebar(self, ui, now));
        }

        egui::CentralPanel::no_frame()
            .frame(egui::Frame::new().fill(c.bg))
            .show(ui, |ui| {
                if self.show_trash {
                    crate::editor::trash_view(self, ui, now);
                } else if let Some(id) = self.current.clone() {
                    crate::editor::page_view(self, ui, &id, now);
                } else {
                    crate::editor::empty_view(self, ui, now);
                }
            });

        if self.search.is_some() {
            crate::editor::search_popup(self, &ctx);
        }
        // On a phone, picking a page in the sidebar closes it.
        if self.narrow && self.sidebar_open && (self.current.clone(), self.show_trash) != page_before {
            self.sidebar_open = false;
        }

        if self.ws.flush(now, false) && self.shot.is_none() {
            // saved quietly
        }
        if self.ws.has_unsaved() {
            ctx.request_repaint_after(std::time::Duration::from_millis(300));
        }
        if ctx.input(|i| i.viewport().close_requested()) {
            self.ws.flush(now, true);
            if self.shot.is_none() {
                self.settings.save();
            }
        }
        self.screenshot_tick(&ctx);
    }
}
