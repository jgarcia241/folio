//! Sidebar, page view, block editor, search and trash.

use crate::App;
use crate::model::{Block, Kind};
use crate::theme::{self, Colors};
use eframe::egui::{
    self, Align, Color32, CornerRadius, FontFamily, FontId, Id, Key, Layout, Pos2, Rect, RichText, Sense, Stroke, Ui,
    pos2, text::LayoutJob, text::TextFormat, vec2,
};
use egui::text::{CCursor, CCursorRange};

#[derive(Default)]
pub struct EditorState {
    pub focus: Option<(u64, usize)>,
    pub focus_title: bool,
    pub slash_sel: usize,
    pub slash_dismissed: Option<u64>,
    pub icon_picker: bool,
    pub pending_open: Option<String>,
    pub focus_db_row: Option<String>,
    /// The one floating menu that is open (it stays open until a click outside).
    pub menu: Option<Menu>,
    pub last_closed: Option<(MenuKind, u64)>,
    /// Block being dragged by its handle.
    pub drag: Option<u64>,
    pub paste_sel: usize,
    pub cover_tab: u8,
    pub db_search: String,
    /// A long press just opened a menu: ignore the tap that ends it.
    pub ignore_click: bool,
    /// Touch screens: the block last tapped keeps its handle visible.
    pub touch_sel: Option<u64>,
}

#[derive(Clone, PartialEq, Debug)]
pub enum MenuKind {
    Page(String),
    Block(u64),
    Paste { block: u64, url: String, start: usize },
    Cover,
    DbFilter,
    DbSort,
}

#[derive(Clone, Debug)]
pub struct Menu {
    pub kind: MenuKind,
    pub pos: Pos2,
    pub frame: u64,
}

/// Open a floating menu, or close it when its button is clicked again.
pub fn open_menu(app: &mut App, ctx: &egui::Context, kind: MenuKind, pos: Pos2) {
    let f = ctx.cumulative_frame_nr();
    if let Some((k, fr)) = &app.ed.last_closed {
        if *k == kind && f <= fr + 2 {
            app.ed.last_closed = None;
            return;
        }
    }
    if app.ed.menu.as_ref().map(|m| m.kind == kind).unwrap_or(false) {
        app.ed.menu = None;
        return;
    }
    app.ed.menu = Some(Menu { kind, pos, frame: f });
}

pub fn close_menu(app: &mut App, ctx: &egui::Context) {
    if let Some(m) = app.ed.menu.take() {
        app.ed.last_closed = Some((m.kind, ctx.cumulative_frame_nr()));
    }
}

/// Draw a floating menu. Returns (contents result, should close).
pub fn float_menu<R>(ctx: &egui::Context, id: Id, m: &Menu, width: f32, c: &Colors, add: impl FnOnce(&mut Ui) -> R) -> (R, bool) {
    // Never wider than the screen (phones).
    let width = width.min(ctx.content_rect().width() - 28.0).max(160.0);
    let area = egui::Area::new(id).order(egui::Order::Foreground).fixed_pos(m.pos).constrain(true).show(ctx, |ui| {
        egui::Frame::popup(ui.style())
            .fill(c.popup)
            .stroke(Stroke::new(1.0, c.line))
            .corner_radius(8)
            .inner_margin(egui::Margin::same(6))
            .show(ui, |ui| {
                ui.set_width(width);
                ui.spacing_mut().item_spacing.y = 2.0;
                add(ui)
            })
            .inner
    });
    let rect = area.response.rect;
    let fnr = ctx.cumulative_frame_nr();
    let other_popup = egui::Popup::is_any_open(ctx);
    let outside = fnr > m.frame + 1
        && !other_popup
        && ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().map(|p| !rect.contains(p)).unwrap_or(false));
    let esc = ctx.input(|i| i.key_pressed(Key::Escape));
    (area.inner, outside || esc)
}

/// A full-width menu row (icon + label). Returns true when clicked.
pub fn menu_item(ui: &mut Ui, icon: &str, label: &str, c: &Colors) -> bool {
    menu_item_colored(ui, icon, label, c.text, c)
}

pub fn menu_item_colored(ui: &mut Ui, icon: &str, label: &str, color: Color32, c: &Colors) -> bool {
    let t = theme::touch_ui();
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), if t { 44.0 } else { 28.0 }), Sense::click());
    if resp.hovered() || resp.is_pointer_button_down_on() {
        ui.painter().rect_filled(r, 5.0, c.hover);
    }
    let (fs, x) = if t { (17.0, 36.0) } else { (14.0, 30.0) };
    ui.painter().text(pos2(r.left() + x / 2.0 - 1.0, r.center().y), egui::Align2::CENTER_CENTER, icon, FontId::proportional(fs), color);
    ui.painter().text(pos2(r.left() + x, r.center().y), egui::Align2::LEFT_CENTER, label, FontId::proportional(fs), color);
    resp.clicked()
}

enum Act {
    Split { i: usize, at: usize },
    MergeUp(usize),
    Delete(usize),
    InsertAfter(usize, Kind),
    MoveUp(usize),
    MoveDown(usize),
    Duplicate(usize),
    TurnInto(usize, Kind),
    FocusPrev(usize),
    FocusNext(usize),
    Indent(usize, i8),
    OpenPage(String),
    NewSubpage(usize, bool),
    Color(usize, u8),
    MoveTo { from: usize, to: usize },
    PasteAs { block: u64, url: String, start: usize, choice: u8 },
    PickImage(usize),
}

const EMOJIS: &[&str] = &[
    "📄", "📝", "📚", "📌", "📎", "📅", "🗓", "📊", "📈", "📋", "📁", "🗂", "💡", "⭐", "🔥", "✅", "❤", "🎯", "🚀", "🏠",
    "💼", "🎨", "🎵", "🍀", "🌱", "🌍", "☀", "🌙", "⚡", "🔑", "🔒", "🔔", "🧠", "💬", "✏", "🛠", "⚙", "🧪", "🎓", "🏆",
    "🎁", "🍎", "☕", "🍕", "🐱", "🐶", "🌸", "🌊", "⛰", "✈", "🚲", "📷", "🎮", "💰", "🧾", "📦", "🏁", "👋", "😀", "🤔",
];

pub const COVERS: &[(Color32, Color32)] = &[
    (Color32::from_rgb(255, 255, 255), Color32::from_rgb(255, 255, 255)),
    (Color32::from_rgb(250, 222, 201), Color32::from_rgb(214, 228, 248)),
    (Color32::from_rgb(208, 232, 214), Color32::from_rgb(196, 220, 236)),
    (Color32::from_rgb(236, 218, 245), Color32::from_rgb(250, 214, 220)),
    (Color32::from_rgb(253, 236, 200), Color32::from_rgb(244, 201, 178)),
    (Color32::from_rgb(52, 73, 94), Color32::from_rgb(100, 140, 170)),
];

// ------------------------------------------------------------------ helpers

fn take_key(ui: &Ui, key: Key, want_shift: bool, want_cmd: bool) -> bool {
    ui.input_mut(|i| {
        let mut hit = false;
        i.events.retain(|e| {
            if hit {
                return true;
            }
            if let egui::Event::Key { key: k, pressed: true, modifiers, .. } = e {
                if *k == key && modifiers.shift == want_shift && modifiers.command == want_cmd && !modifiers.alt {
                    hit = true;
                    return false;
                }
            }
            true
        });
        hit
    })
}

fn key_pressed(ui: &Ui, key: Key) -> bool {
    ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Key { key: k, pressed: true, modifiers, .. } if *k == key && !modifiers.shift && !modifiers.command)))
}

fn cursor_of(ctx: &egui::Context, id: Id) -> Option<CCursorRange> {
    egui::TextEdit::load_state(ctx, id).and_then(|s| s.cursor.char_range())
}

fn set_cursor(ctx: &egui::Context, id: Id, pos: usize) {
    let mut st = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
    st.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(pos))));
    egui::TextEdit::store_state(ctx, id, st);
    ctx.memory_mut(|m| m.request_focus(id));
}

fn byte_of(s: &str, ch: usize) -> usize {
    s.char_indices().nth(ch).map(|(b, _)| b).unwrap_or(s.len())
}

fn ago(secs: u64) -> String {
    let d = crate::store::now_secs().saturating_sub(secs);
    match d {
        0..=59 => "just now".into(),
        60..=3599 => format!("{}m ago", d / 60),
        3600..=86399 => format!("{}h ago", d / 3600),
        _ => {
            let (_, m, day) = crate::db::civil(secs as i64 / 86400);
            format!("{} {}", crate::db::MONTHS[(m as usize).clamp(1, 12) - 1], day)
        }
    }
}

/// Inline Markdown: **bold**, *italic*, `code`, ~~strike~~, [links](url),
/// [@mentions](url) and bare https:// addresses. While the block is being
/// edited the markers are shown dimmed; otherwise they are hidden.
#[allow(clippy::too_many_arguments)]
fn inline_job(text: &str, font: FontId, bold_base: bool, color: Color32, c: &Colors, strike_all: bool, wrap: f32, show_marks: bool) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap;
    let lh = Some(font.size * 1.5);
    let mut bold = false;
    let mut italic = false;
    let mut code = false;
    let mut strike = false;
    let mut seg = String::new();
    let fmt = |bold: bool, italic: bool, code: bool, strike: bool| {
        let fam = if code {
            FontFamily::Monospace
        } else if bold || bold_base {
            theme::bold()
        } else {
            FontFamily::Proportional
        };
        let size = if code { font.size * 0.88 } else { font.size };
        TextFormat {
            font_id: FontId::new(size, fam),
            color: if code { c.inline_code_fg } else { color },
            italics: italic,
            background: if code { c.inline_code } else { Color32::TRANSPARENT },
            strikethrough: if strike || strike_all { Stroke::new(1.0, color) } else { Stroke::NONE },
            line_height: lh,
            ..Default::default()
        }
    };
    let marker_fmt = TextFormat {
        font_id: FontId::new(if show_marks { font.size * 0.9 } else { 0.1 }, FontFamily::Proportional),
        color: if show_marks { c.text3 } else { Color32::TRANSPARENT },
        line_height: lh,
        ..Default::default()
    };
    let flush = |job: &mut LayoutJob, seg: &mut String, f: TextFormat| {
        if !seg.is_empty() {
            job.append(seg, 0.0, f);
            seg.clear();
        }
    };
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < text.len() {
        let rest = &text[i..];
        // [label](url)
        if !code && rest.starts_with('[') {
            if let Some((label, url, total)) = parse_link(rest) {
                flush(&mut job, &mut seg, fmt(bold, italic, code, strike));
                job.append("[", 0.0, marker_fmt.clone());
                let mention = label.starts_with('@');
                if mention {
                    job.append("@", 0.0, marker_fmt.clone());
                }
                let shown = if mention { &label[1..] } else { label };
                let mut lf = fmt(bold, italic, false, strike);
                lf.color = if mention { c.text } else { c.text2 };
                lf.underline = Stroke::new(1.0, if mention { Color32::TRANSPARENT } else { c.text3 });
                if mention {
                    lf.background = c.inline_code;
                    lf.font_id = FontId::new(font.size * 0.94, theme::bold());
                }
                job.append(shown, 0.0, lf);
                job.append(&format!("]({url})"), 0.0, marker_fmt.clone());
                i += total;
                continue;
            }
        }
        // bare address
        if !code && (rest.starts_with("https://") || rest.starts_with("http://")) && (i == 0 || bytes[i - 1].is_ascii_whitespace() || bytes[i - 1] == b'(') {
            let len = rest.find(char::is_whitespace).unwrap_or(rest.len());
            flush(&mut job, &mut seg, fmt(bold, italic, code, strike));
            let mut lf = fmt(bold, italic, false, strike);
            lf.color = c.text2;
            lf.underline = Stroke::new(1.0, c.text3);
            job.append(&rest[..len], 0.0, lf);
            i += len;
            continue;
        }
        let tok: Option<&str> = if code {
            if rest.starts_with('`') { Some("`") } else { None }
        } else if rest.starts_with("**") {
            Some("**")
        } else if rest.starts_with("~~") {
            Some("~~")
        } else if rest.starts_with('`') {
            Some("`")
        } else if rest.starts_with('*') && !(i > 0 && bytes[i - 1] == b' ' && rest.get(1..2) == Some(" ")) {
            Some("*")
        } else {
            None
        };
        if let Some(t) = tok {
            let state_on = match t {
                "**" => bold,
                "~~" => strike,
                "`" => code,
                _ => italic,
            };
            // Only open a marker when it is closed later.
            let closes = state_on || text[i + t.len()..].contains(t);
            if closes {
                flush(&mut job, &mut seg, fmt(bold, italic, code, strike));
                job.append(t, 0.0, marker_fmt.clone());
                match t {
                    "**" => bold = !bold,
                    "~~" => strike = !strike,
                    "`" => code = !code,
                    _ => italic = !italic,
                }
                i += t.len();
                continue;
            }
        }
        let ch = rest.chars().next().unwrap();
        seg.push(ch);
        i += ch.len_utf8();
    }
    if !seg.is_empty() || job.sections.is_empty() {
        job.append(&seg, 0.0, fmt(bold, italic, code, strike));
    }
    job
}

/// `[label](url)` at the start of `s` → (label, url, byte length).
fn parse_link(s: &str) -> Option<(&str, &str, usize)> {
    let close = s.find("](")?;
    let label = &s[1..close];
    if label.contains('[') || label.contains('\n') {
        return None;
    }
    let after = &s[close + 2..];
    let end = after.find(')')?;
    let url = &after[..end];
    if url.contains(char::is_whitespace) || url.is_empty() {
        return None;
    }
    Some((label, url, close + 2 + end + 1))
}

/// Links in a block as (first char, last char exclusive, url).
fn link_spans(text: &str) -> Vec<(usize, usize, String)> {
    let mut out = Vec::new();
    let mut i = 0;
    let bytes = text.as_bytes();
    while i < text.len() {
        let rest = &text[i..];
        if rest.starts_with('[') {
            if let Some((_, url, total)) = parse_link(rest) {
                let a = text[..i].chars().count();
                out.push((a, a + text[i..i + total].chars().count(), url.to_string()));
                i += total;
                continue;
            }
        }
        if (rest.starts_with("https://") || rest.starts_with("http://")) && (i == 0 || bytes[i - 1].is_ascii_whitespace()) {
            let len = rest.find(char::is_whitespace).unwrap_or(rest.len());
            let a = text[..i].chars().count();
            out.push((a, a + rest[..len].chars().count(), rest[..len].to_string()));
            i += len;
            continue;
        }
        i += rest.chars().next().map(|c| c.len_utf8()).unwrap_or(1);
    }
    out
}

/// Clicking a link opens it in the browser (⌘-click while editing).
fn link_click(ui: &Ui, r: &egui::Response, te_id: Id, text: &str, was_focused: bool) {
    let spans = link_spans(text);
    if spans.is_empty() {
        return;
    }
    let cmd = ui.input(|i| i.modifiers.command);
    if r.hovered() && (cmd || !was_focused) {
        // Only show the hand when the pointer is over a link; we know the cursor only after a click.
        ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
    }
    if r.clicked() && (cmd || !was_focused) {
        if let Some(cur) = cursor_of(ui.ctx(), te_id) {
            let at = usize::from(cur.primary.index);
            if let Some((_, _, url)) = spans.iter().find(|(a, b, _)| at > *a && at < *b) {
                open_url(url);
            }
        }
    }
    if r.hovered() && !spans.is_empty() {
        r.clone().on_hover_text_at_pointer(if was_focused { "⌘-click a link to open it" } else { "Click a link to open it" });
    }
}

pub fn open_url(url: &str) {
    if !(url.starts_with("http://") || url.starts_with("https://") || url.starts_with("mailto:")) {
        return;
    }
    crate::platform::open_url(url);
}

pub fn is_url(s: &str) -> bool {
    (s.starts_with("https://") || s.starts_with("http://")) && s.len() > 10 && !s.contains(char::is_whitespace)
}

pub fn host_of(url: &str) -> String {
    let rest = url.split("://").nth(1).unwrap_or(url);
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    host.trim_start_matches("www.").to_string()
}

fn is_image_path(p: &std::path::Path) -> bool {
    matches!(p.extension().map(|e| e.to_string_lossy().to_lowercase()).as_deref(), Some("png" | "jpg" | "jpeg" | "gif" | "webp"))
}

// ------------------------------------------------------------------ pictures

/// Load (once) a picture from the workspace as a texture.
pub fn image_tex(app: &mut App, ctx: &egui::Context, rel: &str) -> Option<egui::TextureHandle> {
    if let Some(t) = app.textures.get(rel) {
        return t.clone();
    }
    let path = app.ws.resolve(rel);
    let tex = crate::vfs::read(&path).ok().and_then(|bytes| image::load_from_memory(&bytes).ok()).map(|img| {
        let img = if img.width() > 2000 { img.thumbnail(2000, 2000) } else { img };
        let rgba = img.to_rgba8();
        let ci = egui::ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], rgba.as_raw());
        ctx.load_texture(rel, ci, egui::TextureOptions::LINEAR)
    });
    app.textures.insert(rel.to_string(), tex.clone());
    tex
}

fn pattern_tex(app: &mut App, ctx: &egui::Context, pat: u8) -> egui::TextureHandle {
    let key = format!("pattern:{pat}");
    if let Some(Some(t)) = app.textures.get(&key) {
        return t.clone();
    }
    let img = crate::covers::render_pattern(pat, crate::covers::W, crate::covers::H);
    let t = ctx.load_texture(key.clone(), img, egui::TextureOptions::LINEAR);
    app.textures.insert(key, Some(t.clone()));
    t
}

fn has_cover(p: &crate::model::Page) -> bool {
    p.cover > 0 || !p.cover_image.is_empty()
}

fn paint_cover(app: &mut App, ui: &Ui, r: Rect, cover: u8, image: &str) {
    use crate::covers::{Cover, get, paint_fill, paint_grad};
    let ctx = ui.ctx().clone();
    let painter = ui.painter_at(r);
    if !image.is_empty() {
        if let Some(t) = image_tex(app, &ctx, image) {
            paint_fill(&painter, r, &t);
            return;
        }
    }
    match get(cover) {
        Cover::None => {
            painter.rect_filled(r, 0.0, theme::colors(app.settings.dark).callout);
        }
        Cover::Grad(a, b) => paint_grad(&painter, r, a, b),
        Cover::Solid(col) => {
            painter.rect_filled(r, 0.0, col);
        }
        Cover::Pattern(pat) => {
            let t = pattern_tex(app, &ctx, pat);
            paint_fill(&painter, r, &t);
        }
    }
}

fn cover_picker(app: &mut App, ctx: &egui::Context, page: &mut crate::model::Page, c: &Colors) {
    let Some(m) = app.ed.menu.clone() else { return };
    if m.kind != MenuKind::Cover {
        return;
    }
    let mut done = false;
    let (_, outside) = float_menu(ctx, Id::new("cover_picker"), &m, 460.0, c, |ui| {
        ui.horizontal(|ui| {
            for (k, label) in ["Gallery", "Colors & gradients", "Upload"].iter().enumerate() {
                let sel = app.ed.cover_tab == k as u8;
                let b = egui::Button::new(RichText::new(*label).size(13.0).color(if sel { c.text } else { c.text2 }).family(if sel { theme::bold() } else { FontFamily::Proportional }))
                    .fill(if sel { c.hover } else { Color32::TRANSPARENT });
                if ui.add(b).clicked() {
                    app.ed.cover_tab = k as u8;
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if small_button(ui, "Remove", c).clicked() {
                    page.cover = 0;
                    page.cover_image.clear();
                    done = true;
                }
            });
        });
        ui.separator();
        match app.ed.cover_tab {
            0 | 1 => {
                let list = if app.ed.cover_tab == 0 { crate::covers::gallery_indices() } else { crate::covers::color_indices() };
                let avail = ui.available_width();
                let per_row = if app.ed.cover_tab == 0 { if avail < 400.0 { 2 } else { 3 } } else if avail < 400.0 { 3 } else { 4 };
                let tw = ((avail - 6.0 * (per_row as f32 - 1.0)) / per_row as f32).floor();
                let th = if app.ed.cover_tab == 0 { tw * 0.44 } else { tw * 0.5 };
                egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                    egui::Grid::new(("cover_grid", app.ed.cover_tab)).spacing(vec2(6.0, 6.0)).show(ui, |ui| {
                        for (k, idx) in list.iter().enumerate() {
                            ui.vertical(|ui| {
                                let (r, resp) = ui.allocate_exact_size(vec2(tw, th), Sense::click());
                                paint_cover(app, ui, r, *idx, "");
                                let sel = page.cover == *idx && page.cover_image.is_empty();
                                let stroke = if sel { Stroke::new(2.0, c.accent) } else if resp.hovered() { Stroke::new(2.0, c.text3) } else { Stroke::new(1.0, c.line) };
                                ui.painter().rect_stroke(r, 4.0, stroke, egui::StrokeKind::Inside);
                                if let crate::covers::Cover::Pattern(p) = crate::covers::get(*idx) {
                                    ui.label(RichText::new(crate::covers::PATTERN_NAMES[p as usize]).size(11.0).color(c.text2));
                                }
                                if resp.clicked() {
                                    page.cover = *idx;
                                    page.cover_image.clear();
                                }
                            });
                            if k % per_row == per_row - 1 {
                                ui.end_row();
                            }
                        }
                    });
                });
            }
            _ => {
                ui.add_space(10.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new(if crate::platform::IS_WEB { "Use any picture from your photos or files." } else { "Use any picture from your computer." }).color(c.text2));
                    ui.label(RichText::new("It is copied into the notes folder, so it syncs too.").size(12.0).color(c.text3));
                    ui.add_space(8.0);
                    let b = egui::Button::new(RichText::new("Choose a picture…").color(Color32::WHITE)).fill(c.accent).corner_radius(5);
                    if ui.add(b).clicked() {
                        crate::platform::pick(crate::platform::PickKind::Cover { page: page.id.clone() });
                        done = true;
                    }
                    ui.add_space(10.0);
                });
            }
        }
    });
    if done || outside {
        close_menu(app, ctx);
    }
}

fn image_ui(app: &mut App, ui: &mut Ui, b: &mut Block, i: usize, w: f32, c: &Colors, acts: &mut Vec<Act>) -> Rect {
    if b.extra.is_empty() {
        let (r, resp) = ui.allocate_exact_size(vec2(w, 46.0), Sense::click());
        ui.painter().rect_filled(r, 4.0, if resp.hovered() { c.hover } else { c.callout });
        ui.painter().text(pos2(r.left() + 14.0, r.center().y), egui::Align2::LEFT_CENTER, "🖼   Add an image  (or drop a picture on the page)", FontId::proportional(14.0), c.text2);
        if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            acts.push(Act::PickImage(i));
        }
        return r;
    }
    let ctx = ui.ctx().clone();
    let Some(tex) = image_tex(app, &ctx, &b.extra.clone()) else {
        let (r, resp) = ui.allocate_exact_size(vec2(w, 46.0), Sense::click());
        ui.painter().rect_filled(r, 4.0, c.callout);
        ui.painter().text(pos2(r.left() + 14.0, r.center().y), egui::Align2::LEFT_CENTER, format!("Picture not found: {}  (click to choose another)", b.extra), FontId::proportional(13.0), c.text2);
        if resp.clicked() {
            acts.push(Act::PickImage(i));
        }
        return r;
    };
    let [tw, th] = tex.size();
    let full = (tw as f32).min(w);
    let dw = if b.size > 0 { (b.size as f32).min(w).max(80.0) } else { full };
    let dh = dw * th as f32 / tw.max(1) as f32;
    let top = ui.cursor().top();
    let out = ui.vertical(|ui| {
        let (r, resp) = ui.allocate_exact_size(vec2(dw, dh), Sense::click());
        ui.painter().image(tex.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        let hovered = ui.rect_contains_pointer(r);
        if resp.double_clicked() {
            crate::editor::reveal(&app.ws.resolve(&b.extra));
        }
        // Resize handle on the right edge.
        let handle = Rect::from_center_size(pos2(r.right() - 6.0, r.center().y), vec2(8.0, (dh * 0.3).clamp(24.0, 60.0)));
        let hr = ui.interact(handle, Id::new(("img_resize", b.id)), Sense::drag());
        if hovered || hr.dragged() {
            ui.painter().rect_filled(handle, 4.0, Color32::from_black_alpha(110));
            ui.painter().rect_stroke(handle, 4.0, Stroke::new(1.0, Color32::WHITE), egui::StrokeKind::Inside);
        }
        if hr.hovered() || hr.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }
        if hr.dragged() {
            let nw = (dw + hr.drag_delta().x).clamp(80.0, w);
            b.size = if nw >= w - 2.0 { 0 } else { nw as u16 };
        }
        let cap_id = Id::new(("caption", b.id));
        let cap_focus = ui.memory(|m| m.has_focus(cap_id));
        if hovered || cap_focus || !b.text.is_empty() {
            ui.add(
                egui::TextEdit::singleline(&mut b.text)
                    .id(cap_id)
                    .frame(egui::Frame::NONE)
                    .hint_text(RichText::new("Add a caption…").color(c.text3).size(13.0))
                    .font(FontId::proportional(13.0))
                    .text_color(c.text2)
                    .desired_width(dw),
            );
        } else {
            ui.add_space(4.0);
        }
    });
    let mut r = out.response.rect;
    r.min.y = top;
    r
}

fn bookmark_ui(ui: &mut Ui, b: &mut Block, w: f32, c: &Colors) -> Rect {
    let host = host_of(&b.extra);
    let dark = c.bg != Color32::WHITE;
    let fr = egui::Frame::new().stroke(Stroke::new(1.0, c.line)).corner_radius(5).show(ui, |ui| {
        ui.set_width(w - 2.0);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 2.0);
            let thumb_w = if w > 460.0 { 180.0 } else { 0.0 };
            ui.vertical(|ui| {
                ui.set_width(w - 2.0 - thumb_w);
                ui.set_max_width(w - 2.0 - thumb_w);
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.add_space(14.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut b.text)
                            .id(Id::new(("bm_title", b.id)))
                            .frame(egui::Frame::NONE)
                            .hint_text(RichText::new(if host.is_empty() { "Title".to_string() } else { host.clone() }).color(c.text).family(theme::bold()).size(14.0))
                            .font(FontId::new(14.0, theme::bold()))
                            .text_color(c.text)
                            .desired_width(f32::INFINITY),
                    );
                });
                ui.horizontal(|ui| {
                    ui.add_space(14.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut b.desc)
                            .id(Id::new(("bm_desc", b.id)))
                            .frame(egui::Frame::NONE)
                            .hint_text(RichText::new("Add a note about this link…").color(c.text3).size(12.5))
                            .font(FontId::proportional(12.5))
                            .text_color(c.text2)
                            .desired_width(f32::INFINITY),
                    );
                });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add_space(14.0);
                    let (fr, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                    ui.painter().rect_filled(fr, 3.0, theme::block_bg(theme::tag_color(&host), dark).gamma_multiply(1.0));
                    ui.painter().rect_stroke(fr, 3.0, Stroke::new(1.0, c.line), egui::StrokeKind::Inside);
                    let initial = host.chars().next().unwrap_or('?').to_uppercase().to_string();
                    ui.painter().text(fr.center(), egui::Align2::CENTER_CENTER, initial, FontId::new(10.0, theme::bold()), c.text);
                    ui.add_space(6.0);
                    let short: String = b.extra.chars().take(70).collect();
                    let lr = ui.add(egui::Label::new(RichText::new(short).size(12.0).color(c.text2)).sense(Sense::click()).truncate());
                    if lr.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text("Open in browser").clicked() {
                        open_url(&b.extra);
                    }
                });
                ui.add_space(10.0);
            });
            if thumb_w > 0.0 {
                let h = ui.min_rect().height().max(84.0);
                let (r, resp) = ui.allocate_exact_size(vec2(thumb_w, h), Sense::click());
                let a = theme::block_bg(theme::tag_color(&host), dark);
                let bcol = theme::block_bg(theme::tag_color(&format!("{host}!")), dark);
                let painter = ui.painter_at(r);
                crate::covers::paint_grad(&painter, r, a, bcol);
                painter.text(r.center(), egui::Align2::CENTER_CENTER, host.split('.').next().unwrap_or("").to_string(), FontId::new(20.0, theme::bold()), c.text2);
                if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                    open_url(&b.extra);
                }
            }
        });
    });
    fr.response.rect
}

fn hover_rect(ui: &Ui, rect: Rect) -> bool {
    ui.rect_contains_pointer(rect)
}

fn small_button(ui: &mut Ui, text: &str, c: &Colors) -> egui::Response {
    ui.add(egui::Button::new(RichText::new(text).color(c.text2).size(13.0)).frame(false).corner_radius(4))
}

/// A top-bar icon button: finger-sized (44 points) on touch screens.
fn bar_button(ui: &mut Ui, icon: &str, color: Color32) -> egui::Response {
    if theme::touch_ui() {
        ui.add(egui::Button::new(RichText::new(icon).color(color).size(22.0)).frame(false).min_size(vec2(44.0, 40.0)))
    } else {
        ui.add(egui::Button::new(RichText::new(icon).color(color).size(13.0)).frame(false).corner_radius(4))
    }
}

// ------------------------------------------------------------------ sidebar

fn row(ui: &mut Ui, selected: bool, c: &Colors, height: f32) -> (Rect, egui::Response) {
    let height = if theme::touch_ui() { height.max(44.0) } else { height };
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    if selected {
        ui.painter().rect_filled(rect, 5.0, c.selected);
    } else if resp.hovered() || resp.is_pointer_button_down_on() {
        ui.painter().rect_filled(rect, 5.0, c.hover);
    }
    (rect, resp)
}

fn row_label(ui: &Ui, rect: Rect, x: f32, icon: &str, text: &str, color: Color32, bold: bool) {
    let p = ui.painter();
    let y = rect.center().y;
    let t = theme::touch_ui();
    let (fs, gap) = if t { (17.0, 30.0) } else { (14.0, 24.0) };
    let mut x = rect.left() + x;
    if !icon.is_empty() {
        p.text(pos2(x + 9.0, y), egui::Align2::CENTER_CENTER, icon, FontId::proportional(fs), color);
        x += gap;
    }
    let fam = if bold { theme::bold() } else { FontFamily::Proportional };
    let galley = p.layout_no_wrap(text.to_string(), FontId::new(fs, fam), color);
    let clip = Rect::from_min_max(pos2(x, rect.top()), pos2(rect.right() - 4.0, rect.bottom()));
    p.with_clip_rect(clip).galley(pos2(x, y - galley.size().y / 2.0), galley, color);
}

pub fn sidebar(app: &mut App, ui: &mut Ui, now: f64) {
    let c = theme::colors(app.settings.dark);
    if !app.narrow {
        app.settings.sidebar_width = (ui.available_width() + 12.0).clamp(200.0, 420.0);
    }

    // Web: the linked folder needs a tap before the browser lets the app use it again.
    if crate::platform::IS_WEB {
        let st = crate::platform::folder_state();
        if st.mode == 3 {
            let text = if st.provider.is_empty() { format!("🔗  Reconnect folder \"{}\"", st.name) } else { format!("🔗  Sign in to {} again", st.provider) };
            let b = egui::Button::new(RichText::new(text).color(Color32::WHITE).size(14.0)).fill(c.accent).corner_radius(6);
            if ui.add_sized([ui.available_width(), if theme::touch_ui() { 44.0 } else { 30.0 }], b).on_hover_text("Changes are kept on this device until the folder is reconnected").clicked() {
                crate::platform::reconnect_notes_folder();
            }
            ui.add_space(8.0);
        }
    }
    // Search box
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
    crate::platform::keyboard_zone(rect);
    {
        let p = ui.painter();
        p.rect(rect, 7.0, if resp.hovered() { c.hover } else { c.bg }, Stroke::new(1.0, c.line), egui::StrokeKind::Inside);
        p.text(pos2(rect.left() + 10.0, rect.center().y), egui::Align2::LEFT_CENTER, "Search", FontId::proportional(14.0), c.text3);
        let kb = Rect::from_center_size(pos2(rect.right() - 20.0, rect.center().y), vec2(28.0, 18.0));
        p.rect_filled(kb, 4.0, c.hover);
        p.text(kb.center(), egui::Align2::CENTER_CENTER, "⌘K", FontId::proportional(10.5), c.text2);
    }
    if resp.clicked() {
        app.search = Some((String::new(), 0));
    }
    ui.add_space(10.0);
    let (rect, resp) = row(ui, false, &c, 30.0);
    row_label(ui, rect, 4.0, "📝", "New page", c.text, false);
    if resp.clicked() {
        app.new_page("", now);
    }
    let (rect, resp) = row(ui, false, &c, 30.0);
    row_label(ui, rect, 4.0, "🗃", "New database", c.text, false);
    if resp.clicked() {
        let id = app.new_page("", now);
        if let Some(e) = app.ws.get_mut(&id) {
            e.page.db = Some(crate::model::Db::new());
            e.page.icon = "🗃".into();
            e.page.blocks.clear();
        }
    }
    let (rect, resp) = row(ui, app.show_trash, &c, 30.0);
    row_label(ui, rect, 4.0, "🗑", "Trash", c.text, false);
    if resp.clicked() {
        app.show_trash = true;
    }
    ui.add_space(14.0);

    let bottom_h = 44.0;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .max_height(ui.available_height() - bottom_h)
        .show(ui, |ui| {
            let favs: Vec<(String, String, String)> = app
                .ws
                .entries
                .iter()
                .filter(|e| e.page.favorite && !e.page.trashed)
                .map(|e| (e.page.id.clone(), e.page.icon.clone(), e.page.display_title().to_string()))
                .collect();
            if !favs.is_empty() {
                section_header(ui, "Favorites", &c);
                for (id, icon, title) in favs {
                    let sel = app.current.as_deref() == Some(&id) && !app.show_trash;
                    let (rect, resp) = row(ui, sel, &c, 28.0);
                    row_label(ui, rect, 22.0, if icon.is_empty() { "📄" } else { &icon }, &title, c.text, false);
                    if resp.clicked() {
                        app.open_page(&id);
                    }
                }
                ui.add_space(10.0);
            }
            let (hrect, hresp) = section_header(ui, "Private", &c);
            if hover_rect(ui, hrect) || hresp.hovered() {
                let b = Rect::from_center_size(pos2(hrect.right() - 12.0, hrect.center().y), vec2(20.0, 20.0));
                if ui.put(b, egui::Button::new(RichText::new("+").color(c.text2)).frame(false)).on_hover_text("Add a page").clicked() {
                    app.new_page("", now);
                }
            }
            tree(app, ui, "", 0, now, &c);
        });

    page_menu_float(app, &ui.ctx().clone(), now, &c);

    // Bottom: workspace switcher
    ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
        let st = crate::platform::folder_state();
        let name = if crate::platform::IS_WEB && !st.provider.is_empty() {
            format!("{} · {}", st.provider, if st.status.is_empty() { "…".to_string() } else { st.status.clone() })
        } else if crate::platform::IS_WEB && st.mode != 0 && !st.name.is_empty() {
            st.name.clone()
        } else {
            app.ws.name()
        };
        let (rect, resp) = row(ui, false, &c, 32.0);
        {
            let p = ui.painter();
            let sq = Rect::from_center_size(pos2(rect.left() + 15.0, rect.center().y), vec2(20.0, 20.0));
            p.rect_filled(sq, 4.0, c.selected);
            let initial = name.chars().next().unwrap_or('F').to_uppercase().to_string();
            p.text(sq.center(), egui::Align2::CENTER_CENTER, initial, FontId::new(12.0, theme::bold()), c.text2);
        }
        row_label(ui, rect, 30.0, "", &format!("{name}  ⌄"), c.text, false);
        egui::Popup::menu(&resp).width(280.0).align(egui::RectAlign::TOP_START).show(|ui| {
            if crate::platform::IS_WEB {
                let st = crate::platform::folder_state();
                let cloud = !st.provider.is_empty();
                match st.mode {
                    1 if cloud => {
                        ui.label(RichText::new(format!("☁ Notes are saved in {} ({}).", st.provider, st.name)).size(12.0).color(c.text2));
                        ui.label(RichText::new(format!("{} — changes upload in the background; edits made offline upload when you're back online.", st.status)).size(11.0).color(c.text3));
                    }
                    3 if cloud => {
                        ui.label(RichText::new(format!("☁ Sign in to {} again to keep syncing.", st.provider)).size(12.0).color(c.text2));
                        ui.label(RichText::new("Your changes are kept on this device meanwhile.").size(11.0).color(c.text3));
                    }
                    1 => {
                        ui.label(RichText::new(format!("📂 Notes are saved in the folder \"{}\".", st.name)).size(12.0).color(c.text2));
                        ui.label(RichText::new("Every change is written there right away, and changes made in the folder show up here.").size(11.0).color(c.text3));
                    }
                    2 => {
                        ui.label(RichText::new(format!("📂 Notes were read from the folder \"{}\".", st.name)).size(12.0).color(c.text2));
                        ui.label(RichText::new("Safari on iPhone and iPad can read a folder from Files but can't save into it. Your changes are kept on this device; use Export to save them back to Files.").size(11.0).color(c.text3));
                    }
                    3 => {
                        ui.label(RichText::new(format!("📂 Folder \"{}\" needs to be reconnected.", st.name)).size(12.0).color(c.text2));
                    }
                    _ => {
                        ui.label(RichText::new("Your notes are saved on this device and work offline.").size(11.0).color(c.text3));
                    }
                }
                ui.separator();
                if st.mode == 3 && ui.button(if cloud { format!("🔗  Sign in to {} again", st.provider) } else { format!("🔗  Reconnect \"{}\"", st.name) }).clicked() {
                    crate::platform::reconnect_notes_folder();
                }
                if cloud && st.mode == 1 && ui.button("⟳  Sync now").clicked() {
                    crate::platform::sync_now();
                }
                // Cloud services (need an app key in web/config.js, see the README).
                ui.label(RichText::new("Cloud services").size(11.0).color(c.text3));
                let ready = crate::platform::cloud_ready();
                for (i, cl) in crate::store::Cloud::ALL.iter().enumerate() {
                    let connected = cloud && st.provider == cl.name();
                    let label = if connected { format!("✔  {} connected", cl.name()) } else { format!("☁  Connect {}", cl.name()) };
                    let hint = if ready[i] {
                        format!("Sign in once; every page is then saved in your {} (folder {}).", cl.name(), cl.notes_subfolder())
                    } else {
                        format!("{} is not set up yet: add its app key to web/config.js (see the README, \"Cloud services\").", cl.name())
                    };
                    if ui.add_enabled(ready[i] && !connected, egui::Button::new(label)).on_hover_text(&hint).on_disabled_hover_text(&hint).clicked() {
                        crate::platform::connect_cloud(cl.key());
                    }
                }
                ui.separator();
                let label = if st.can_write {
                    if st.mode == 1 { "📂  Use a different folder…" } else { "📂  Use a folder for my notes…" }
                } else {
                    "📂  Open a folder from Files…"
                };
                let hint = if st.can_write {
                    "Pick a folder (for example inside Google Drive or OneDrive): every page is saved there as a .md file"
                } else {
                    "Loads all pages and pictures from a folder in Files. Safari can't save back into it: use Export."
                };
                if ui.button(label).on_hover_text(hint).clicked() {
                    crate::platform::choose_notes_folder();
                }
                if st.mode != 0 && ui.button(if cloud { format!("✕  Disconnect {}", st.provider) } else { "✕  Stop using this folder".to_string() }).on_hover_text("Keep the notes on this device only").clicked() {
                    crate::platform::forget_notes_folder();
                }
            } else {
                ui.label(RichText::new("Your notes are saved in").size(11.0).color(c.text3));
                ui.label(RichText::new(app.ws.root.display().to_string()).size(11.0).color(c.text2));
            }
            ui.separator();
            if ui.button("⬇  Import notes or pictures…").on_hover_text(".md pages, pictures, or a .zip of a notes folder").clicked() {
                crate::platform::pick(crate::platform::PickKind::Import);
            }
            if ui.button("⬆  Export everything as .zip…").on_hover_text("All pages, folders and pictures, ready to save anywhere").clicked() {
                let bytes = app.ws.export_zip();
                let name = format!("{}.zip", app.ws.name());
                match crate::platform::save_file(&name, &bytes) {
                    Ok(true) => app.set_status("Exported", ui.input(|i| i.time)),
                    Ok(false) => {}
                    Err(e) => app.set_status(format!("Export failed: {e}"), ui.input(|i| i.time)),
                }
            }
            ui.separator();
            let recent: Vec<_> = app.settings.recent.iter().filter(|r| **r != app.ws.root).cloned().collect();
            if !recent.is_empty() && !crate::platform::IS_WEB {
                ui.label(RichText::new("Recent folders").size(11.0).color(c.text3));
                for r in recent {
                    let label = r.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                    if ui.button(format!("📁  {label}")).on_hover_text(r.display().to_string()).clicked() {
                        app.switch_workspace(r);
                    }
                }
                ui.separator();
            }
            if !crate::platform::IS_WEB {
                if ui.button("📂  Open or create a folder…").on_hover_text("Pick a folder inside Google Drive, OneDrive or Dropbox to share it").clicked() {
                    if let Some(dir) = crate::platform::pick_folder() {
                        app.switch_workspace(dir);
                    }
                }
                if ui.button("🔎  Show folder").clicked() {
                    reveal(&app.ws.root);
                }
                ui.separator();
                ui.label(RichText::new("Keep notes in a cloud service").size(11.0).color(c.text3));
                for cloud in crate::store::Cloud::ALL {
                    let root = crate::store::cloud_sync_root(cloud);
                    let label = format!("☁  Use {} folder", cloud.name());
                    let hint = match &root {
                        Some(r) => format!("Saves notes in {}/{} — the {} app syncs them to your other devices and the web app", r.display(), cloud.notes_subfolder(), cloud.name()),
                        None => format!("Install the {} desktop app first, then choose this again", cloud.name()),
                    };
                    if ui.add_enabled(root.is_some(), egui::Button::new(label)).on_hover_text(&hint).on_disabled_hover_text(&hint).clicked() {
                        if let Some(dir) = crate::store::cloud_notes_folder(cloud) {
                            // An empty cloud folder gets the notes you already have.
                            let empty = crate::vfs::list_md(&dir).is_empty();
                            if empty && dir != app.ws.root {
                                app.ws.flush(0.0, true);
                                crate::store::copy_notes(&app.ws.root.clone(), &dir);
                            }
                            app.switch_workspace(dir);
                            app.set_status(format!("Notes are now in {}", cloud.name()), ui.input(|i| i.time));
                        }
                    }
                }
                ui.separator();
            }
            let mut dark = app.settings.dark;
            if ui.checkbox(&mut dark, "Dark mode").changed() {
                app.settings.dark = dark;
                theme::apply(ui.ctx(), dark);
                app.settings.save();
            }
            ui.checkbox(&mut app.full_width, "Full width pages");
            if !crate::platform::IS_WEB && ui.checkbox(&mut app.settings.confirm_close, "Ask before closing").changed() {
                app.settings.save();
            }
        });
        if let Some(err) = &app.ws.last_error {
            ui.label(RichText::new(err).color(Color32::from_rgb(220, 70, 70)).size(11.0));
        }
        ui.add_space(2.0);
        let y = ui.cursor().bottom();
        ui.painter().hline(ui.max_rect().x_range(), y, Stroke::new(1.0, c.line));
    });
}

fn section_header(ui: &mut Ui, text: &str, c: &Colors) -> (Rect, egui::Response) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::hover());
    ui.painter().text(pos2(rect.left() + 8.0, rect.center().y), egui::Align2::LEFT_CENTER, text, FontId::new(12.0, theme::bold()), c.text3);
    (rect, resp)
}

fn tree(app: &mut App, ui: &mut Ui, parent: &str, depth: usize, now: f64, c: &Colors) {
    let kids: Vec<(String, String, String, bool, bool)> = app
        .ws
        .children(parent)
        .iter()
        .map(|p| (p.id.clone(), p.icon.clone(), p.display_title().to_string(), p.favorite, p.db.is_some()))
        .collect();
    if kids.is_empty() && depth > 0 {
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::hover());
        ui.painter().text(
            pos2(rect.left() + 12.0 + depth as f32 * 14.0 + 16.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            "No pages inside",
            FontId::proportional(13.0),
            c.text3,
        );
        return;
    }
    let n = kids.len();
    let t = theme::touch_ui();
    for (k, (id, icon, title, fav, is_db)) in kids.into_iter().enumerate() {
        let sel = app.current.as_deref() == Some(&id) && !app.show_trash;
        let (rect, resp) = row(ui, sel, c, 28.0);
        let indent = depth as f32 * if t { 18.0 } else { 14.0 };
        let open = app.expanded.contains(&id);
        let hovered = hover_rect(ui, rect);
        let has_kids = !is_db && !app.ws.children(&id).is_empty();
        let shown_icon = if icon.is_empty() { "📄".to_string() } else { icon.clone() };
        let mut menu_clicked = false;
        let arrow_clicked;
        if t {
            // Touch: the arrow has its own always-visible column, big enough for a finger.
            let arrow_rect = Rect::from_min_size(pos2(rect.left() + indent, rect.top()), vec2(30.0, rect.height()));
            row_label(ui, rect, 28.0 + indent, &shown_icon, &title, c.text, sel);
            let arrow = ui.interact(arrow_rect, Id::new(("arrow", &id)), Sense::click());
            if has_kids {
                ui.painter().text(arrow_rect.center(), egui::Align2::CENTER_CENTER, if open { "⏷" } else { "⏵" }, FontId::proportional(14.0), c.text2);
            }
            arrow_clicked = arrow.clicked() && has_kids;
            if arrow_clicked {
                if open {
                    app.expanded.remove(&id);
                } else {
                    app.expanded.insert(id.clone());
                }
            }
            // The open page shows its "···" and "+" buttons; any row: long-press for the menu.
            if sel {
                let dots = Rect::from_min_size(pos2(rect.right() - 84.0, rect.top()), vec2(42.0, rect.height()));
                let plus = Rect::from_min_size(pos2(rect.right() - 42.0, rect.top()), vec2(42.0, rect.height()));
                ui.painter().rect_filled(Rect::from_min_max(dots.min, rect.max), 5.0, c.selected);
                let dr = ui.interact(dots, Id::new(("dots", &id)), Sense::click());
                ui.painter().text(dots.center(), egui::Align2::CENTER_CENTER, "···", FontId::proportional(20.0), c.text2);
                let pr = ui.interact(plus, Id::new(("plus_page", &id)), Sense::click());
                ui.painter().text(plus.center(), egui::Align2::CENTER_CENTER, "+", FontId::proportional(24.0), c.text2);
                if dr.clicked() {
                    open_menu(app, ui.ctx(), MenuKind::Page(id.clone()), pos2(rect.right() - 240.0, rect.bottom() + 4.0));
                    menu_clicked = true;
                }
                if pr.clicked() {
                    app.new_page(&id, now);
                    menu_clicked = true;
                }
            }
            if resp.long_touched() {
                open_menu(app, ui.ctx(), MenuKind::Page(id.clone()), pos2(rect.left() + 40.0, rect.bottom() + 4.0));
                app.ed.ignore_click = true;
                menu_clicked = true;
            }
        } else {
            // Mouse: the arrow replaces the icon on hover; "···" and "+" appear on hover.
            let arrow_rect = Rect::from_center_size(pos2(rect.left() + 31.0 + indent, rect.center().y), vec2(20.0, 20.0));
            row_label(ui, rect, 22.0 + indent, &shown_icon, &title, c.text, false);
            let arrow = ui.interact(arrow_rect, Id::new(("arrow", &id)), Sense::click());
            if hovered && !is_db {
                ui.painter().rect_filled(arrow_rect.expand(1.0), 3.0, if arrow.hovered() || sel { c.selected } else { c.hover });
                ui.painter().text(arrow_rect.center(), egui::Align2::CENTER_CENTER, if open { "⏷" } else { "⏵" }, FontId::proportional(11.0), c.text2);
            }
            arrow_clicked = arrow.clicked();
            if arrow_clicked && is_db {
                app.open_page(&id);
            } else if arrow_clicked {
                if open {
                    app.expanded.remove(&id);
                } else {
                    app.expanded.insert(id.clone());
                }
            }
            if hovered {
                let plus = Rect::from_center_size(pos2(rect.right() - 12.0, rect.center().y), vec2(20.0, 20.0));
                let dots = Rect::from_center_size(pos2(rect.right() - 34.0, rect.center().y), vec2(20.0, 20.0));
                ui.painter().rect_filled(Rect::from_min_max(pos2(dots.left() - 2.0, rect.top()), rect.right_bottom()), 5.0, if sel { c.selected } else { c.hover });
                let pr = ui.interact(plus, Id::new(("plus_page", &id)), Sense::click());
                ui.painter().text(plus.center(), egui::Align2::CENTER_CENTER, "+", FontId::proportional(15.0), c.text2);
                if pr.on_hover_text("Add a page inside").clicked() {
                    app.new_page(&id, now);
                    menu_clicked = true;
                }
                let dr = ui.interact(dots, Id::new(("dots", &id)), Sense::click());
                ui.painter().text(dots.center(), egui::Align2::CENTER_CENTER, "···", FontId::proportional(14.0), c.text2);
                if dr.on_hover_text("Delete, duplicate, and more").clicked() {
                    open_menu(app, ui.ctx(), MenuKind::Page(id.clone()), dots.left_bottom() + vec2(0.0, 4.0));
                    menu_clicked = true;
                }
            } else if app.ws.get(&id).map(|e| e.page.folder).unwrap_or(false) {
                ui.painter().text(pos2(rect.right() - 12.0, rect.center().y), egui::Align2::CENTER_CENTER, "📁", FontId::proportional(11.0), c.text3);
            }
        }
        // Right-click (egui also reports a long press as one; that is handled above).
        if resp.secondary_clicked() && !resp.long_touched() {
            let pos = ui.input(|i| i.pointer.interact_pos()).unwrap_or(rect.center());
            open_menu(app, ui.ctx(), MenuKind::Page(id.clone()), pos);
        }
        let _ = (fav, k, n);
        if resp.clicked() && app.ed.ignore_click {
            app.ed.ignore_click = false;
        } else if resp.clicked() && !arrow_clicked && !menu_clicked {
            app.open_page(&id);
        }
        if open && !is_db {
            tree(app, ui, &id, depth + 1, now, c);
        }
    }
}

/// The sidebar's "···" / right-click menu for a page. Drawn every frame while open.
pub fn page_menu_float(app: &mut App, ctx: &egui::Context, now: f64, c: &Colors) {
    let Some(m) = app.ed.menu.clone() else { return };
    let MenuKind::Page(id) = &m.kind else { return };
    let id = id.clone();
    let Some(e) = app.ws.get(&id) else {
        app.ed.menu = None;
        return;
    };
    let fav = e.page.favorite;
    let folder = e.page.folder;
    let parent = e.page.parent.clone();
    let sibs: Vec<String> = app.ws.children(&parent).iter().map(|p| p.id.clone()).collect();
    let k = sibs.iter().position(|x| *x == id).unwrap_or(0);
    let n = sibs.len();
    let (close, outside) = float_menu(ctx, Id::new("page_menu"), &m, 230.0, c, |ui| {
        let mut close = false;
        if menu_item(ui, if fav { "☆" } else { "⭐" }, if fav { "Remove from Favorites" } else { "Add to Favorites" }, c) {
            if let Some(e) = app.ws.get_mut(&id) {
                e.page.favorite = !fav;
            }
            app.ws.touch(&id, now - 10.0);
            close = true;
        }
        if menu_item(ui, "⧉", "Duplicate", c) {
            duplicate_page(app, &id, now);
            close = true;
        }
        if menu_item(ui, "➕", "Add a page inside", c) {
            app.new_page(&id, now);
            close = true;
        }
        if k > 0 && menu_item(ui, "⬆", "Move up", c) {
            reorder(app, &id, -1, now);
        }
        if k + 1 < n && menu_item(ui, "⬇", "Move down", c) {
            reorder(app, &id, 1, now);
        }
        if menu_item(ui, if folder { "☑" } else { "☐" }, "Keep sub-pages in a folder", c) {
            app.ws.set_folder(&id, !folder, now);
        }
        if !crate::platform::IS_WEB && menu_item(ui, "🔎", "Show file", c) {
            if let Some(e) = app.ws.get(&id) {
                reveal(&e.path);
            }
            close = true;
        }
        ui.separator();
        if menu_item_colored(ui, "🗑", "Move to Trash", Color32::from_rgb(220, 70, 70), c) {
            app.ws.set_trashed(&id, true, now);
            if app.current.as_deref() == Some(id.as_str()) || app.current.as_ref().map(|cur| app.ws.path_to(cur).contains(&id)).unwrap_or(false) {
                app.current = app.ws.children("").first().map(|p| p.id.clone());
            }
            close = true;
        }
        close
    });
    if close || outside {
        close_menu(app, ctx);
    }
}

fn duplicate_page(app: &mut App, id: &str, now: f64) {
    let Some(src) = app.ws.get(id).map(|e| e.page.clone()) else { return };
    let new_id = app.ws.create("", &src.parent, now);
    if let Some(e) = app.ws.get_mut(&new_id) {
        let order = e.page.order;
        e.page = src.clone();
        e.page.id = new_id.clone();
        e.page.title = format!("{} (copy)", src.display_title());
        e.page.order = order;
        for b in &mut e.page.blocks {
            b.id = crate::model::next_block_id();
        }
    }
    app.ws.touch(&new_id, now - 10.0);
    app.open_page(&new_id);
}

fn reorder(app: &mut App, id: &str, dir: i32, now: f64) {
    let Some(parent) = app.ws.get(id).map(|e| e.page.parent.clone()) else { return };
    let mut ids: Vec<String> = app.ws.children(&parent).iter().map(|p| p.id.clone()).collect();
    let Some(pos) = ids.iter().position(|x| x == id) else { return };
    let to = pos as i32 + dir;
    if to < 0 || to as usize >= ids.len() {
        return;
    }
    ids.swap(pos, to as usize);
    for (o, pid) in ids.iter().enumerate() {
        let changed = app.ws.get(pid).map(|e| e.page.order != o as i64).unwrap_or(false);
        if changed {
            app.ws.get_mut(pid).unwrap().page.order = o as i64;
            app.ws.touch(pid, now - 10.0);
        }
    }
}

pub fn reveal(path: &std::path::Path) {
    crate::platform::reveal(path);
}

// ------------------------------------------------------------------ views

pub fn empty_view(app: &mut App, ui: &mut Ui, now: f64) {
    let c = theme::colors(app.settings.dark);
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() * 0.35);
        ui.label(RichText::new("No page open").size(20.0).color(c.text2));
        ui.add_space(8.0);
        if ui.button(RichText::new("➕  New page").size(15.0)).clicked() {
            app.new_page("", now);
        }
    });
}

pub fn trash_view(app: &mut App, ui: &mut Ui, now: f64) {
    let c = theme::colors(app.settings.dark);
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        let w = ui.available_width().min(720.0);
        let pad = (ui.available_width() - w) / 2.0;
        ui.add_space(60.0);
        ui.horizontal(|ui| {
            ui.add_space(pad);
            ui.vertical(|ui| {
                ui.set_width(w);
                ui.label(RichText::new("🗑  Trash").size(32.0).family(theme::bold()).color(c.text));
                ui.add_space(6.0);
                ui.label(RichText::new("Pages in the trash stay in your folder until you delete them for good.").color(c.text2));
                ui.add_space(16.0);
                let items: Vec<(String, String, u64)> = app
                    .ws
                    .entries
                    .iter()
                    .filter(|e| e.page.trashed && app.ws.get(&e.page.parent).map(|p| !p.page.trashed).unwrap_or(true))
                    .map(|e| (e.page.id.clone(), format!("{} {}", if e.page.icon.is_empty() { "📄" } else { &e.page.icon }, e.page.display_title()), e.page.updated))
                    .collect();
                if items.is_empty() {
                    ui.label(RichText::new("Nothing in the trash.").color(c.text3));
                }
                for (id, label, upd) in items {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(label).size(15.0).color(c.text));
                        ui.label(RichText::new(ago(upd)).size(12.0).color(c.text3));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button(RichText::new("Delete forever").color(Color32::from_rgb(220, 70, 70))).clicked() {
                                app.ws.delete_forever(&id);
                            }
                            if ui.button("↩ Restore").clicked() {
                                app.ws.set_trashed(&id, false, now);
                                app.ws_parent_trashed(&id);
                                app.open_page(&id);
                            }
                        });
                    });
                    ui.separator();
                }
            });
        });
    });
}

impl App {
    fn ws_parent_trashed(&mut self, id: &str) -> bool {
        let parent = self.ws.get(id).map(|e| e.page.parent.clone()).unwrap_or_default();
        let t = self.ws.get(&parent).map(|p| p.page.trashed).unwrap_or(false);
        if t {
            if let Some(e) = self.ws.get_mut(id) {
                e.page.parent.clear();
            }
        }
        t
    }
}

pub fn search_popup(app: &mut App, ctx: &egui::Context) {
    let c = theme::colors(app.settings.dark);
    let screen = ctx.content_rect();
    let mut close = false;
    let mut open: Option<String> = None;
    // dim background
    egui::Area::new(Id::new("search_dim"))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let r = ui.allocate_rect(screen, Sense::click());
            ui.painter().rect_filled(screen, 0.0, Color32::from_black_alpha(60));
            if r.clicked() {
                close = true;
            }
        });
    let w = 620.0f32.min(screen.width() - 40.0);
    egui::Area::new(Id::new("search_box"))
        .order(egui::Order::Tooltip)
        .fixed_pos(pos2(screen.center().x - w / 2.0, screen.top() + 90.0))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(c.popup)
                .corner_radius(CornerRadius::same(10))
                .stroke(Stroke::new(1.0, c.line))
                .shadow(egui::epaint::Shadow { offset: [0, 8], blur: 24, spread: 0, color: Color32::from_black_alpha(40) })
                .inner_margin(egui::Margin::same(10))
                .show(ui, |ui| {
                    ui.set_width(w - 20.0);
                    let (q, sel) = app.search.as_mut().unwrap();
                    let id = Id::new("search_q");
                    let up = take_key(ui, Key::ArrowUp, false, false);
                    let down = take_key(ui, Key::ArrowDown, false, false);
                    let enter = take_key(ui, Key::Enter, false, false);
                    if take_key(ui, Key::Escape, false, false) {
                        close = true;
                    }
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("🔍").size(16.0).color(c.text3));
                        let qr = ui.add(
                            egui::TextEdit::singleline(q)
                                .id(id)
                                .frame(egui::Frame::NONE)
                                .hint_text("Search pages…")
                                .font(FontId::proportional(17.0))
                                .text_color(c.text)
                                .desired_width(f32::INFINITY),
                        );
                        crate::platform::keyboard_zone(qr.rect);
                    });
                    ui.memory_mut(|m| m.request_focus(id));
                    ui.add_space(6.0);
                    ui.separator();
                    let ql = q.to_lowercase();
                    let mut hits: Vec<(i32, String, String, String, String)> = Vec::new();
                    for e in &app.ws.entries {
                        if e.page.trashed {
                            continue;
                        }
                        let title = e.page.display_title().to_string();
                        let icon = if e.page.icon.is_empty() { "📄".to_string() } else { e.page.icon.clone() };
                        if ql.is_empty() {
                            hits.push((-(e.page.updated as i32 / 2), e.page.id.clone(), icon, title, String::new()));
                            continue;
                        }
                        if title.to_lowercase().contains(&ql) {
                            hits.push((0, e.page.id.clone(), icon, title, String::new()));
                        } else {
                            let body = e.page.plain_text();
                            let bl = body.to_lowercase();
                            if let Some(pos) = bl.find(&ql) {
                                let start = bl[..pos].char_indices().rev().nth(30).map(|(b, _)| b).unwrap_or(0);
                                let snippet: String = body.get(start..).unwrap_or("").chars().take(80).collect();
                                hits.push((1, e.page.id.clone(), icon, title, snippet.replace('\n', " ")));
                            }
                        }
                    }
                    hits.sort_by(|a, b| a.0.cmp(&b.0).then(a.3.cmp(&b.3)));
                    hits.truncate(12);
                    if hits.is_empty() {
                        ui.add_space(6.0);
                        ui.label(RichText::new("No results").color(c.text3));
                    }
                    if !hits.is_empty() {
                        if down {
                            *sel = (*sel + 1).min(hits.len() - 1);
                        }
                        if up {
                            *sel = sel.saturating_sub(1);
                        }
                        *sel = (*sel).min(hits.len() - 1);
                    }
                    ui.label(RichText::new(if ql.is_empty() { "Recent" } else { "Best matches" }).size(11.0).color(c.text3));
                    for (k, (_, pid, icon, title, snip)) in hits.iter().enumerate() {
                        let h = if snip.is_empty() { 32.0 } else { 46.0 };
                        let (rect, resp) = row(ui, k == *sel, &c, h);
                        row_label(ui, Rect::from_min_size(rect.min, vec2(rect.width(), 32.0)), 4.0, icon, title, c.text, false);
                        if !snip.is_empty() {
                            ui.painter().text(pos2(rect.left() + 32.0, rect.top() + 34.0), egui::Align2::LEFT_CENTER, snip, FontId::proportional(12.0), c.text2);
                        }
                        if resp.hovered() {
                            *sel = k;
                        }
                        if resp.clicked() || (enter && k == *sel) {
                            open = Some(pid.clone());
                        }
                    }
                });
        });
    if let Some(id) = open {
        app.open_page(&id);
        close = true;
    }
    if close {
        app.search = None;
    }
}

// ------------------------------------------------------------------ page

pub fn page_view(app: &mut App, ui: &mut Ui, pid: &str, now: f64) {
    let c = theme::colors(app.settings.dark);
    if app.ws.get(pid).is_none() {
        app.current = None;
        return;
    }

    // Top bar: breadcrumb + status
    let touch = theme::touch_ui();
    let bar = ui.allocate_exact_size(vec2(ui.available_width(), if touch { 52.0 } else { 44.0 }), Sense::hover()).0;
    {
        let mut bui = ui.new_child(egui::UiBuilder::new().max_rect(bar.shrink2(vec2(if touch { 4.0 } else { 12.0 }, if touch { 4.0 } else { 8.0 }))).layout(Layout::left_to_right(Align::Center)));
        if !app.sidebar_open {
            if bar_button(&mut bui, "☰", c.text2).on_hover_text("Show sidebar (⌘\\)").clicked() {
                app.sidebar_open = true;
            }
        } else if !app.narrow && bar_button(&mut bui, "«", c.text2).on_hover_text("Hide sidebar (⌘\\)").clicked() {
            app.sidebar_open = false;
        }
        // Phone: a back button to the page you came from.
        if app.narrow && !app.history.is_empty() && bar_button(&mut bui, "‹", c.text2).on_hover_text("Back").clicked() {
            app.go_back();
        }
        let path = app.ws.path_to(pid);
        let crumbs: Vec<(String, String)> = path
            .iter()
            .filter_map(|id| app.ws.get(id).map(|e| (id.clone(), format!("{} {}", if e.page.icon.is_empty() { "📄" } else { &e.page.icon }, e.page.display_title()))))
            .collect();
        let crumbs: Vec<(String, String)> = if app.narrow { crumbs.into_iter().last().into_iter().collect() } else { crumbs };
        let n = crumbs.len();
        for (k, (id, label)) in crumbs.into_iter().enumerate() {
            let short: String = label.chars().take(if app.narrow { 18 } else { 28 }).collect();
            if bui.add(egui::Button::new(RichText::new(short).color(c.text).size(if touch { 17.0 } else { 14.0 })).frame(false)).clicked() {
                app.open_page(&id);
            }
            if k + 1 < n {
                bui.label(RichText::new("/").color(c.text3));
            }
        }
        bui.with_layout(Layout::right_to_left(Align::Center), |bui| {
            let (fav, upd, dirty) = app.ws.get(pid).map(|e| (e.page.favorite, e.page.updated, e.dirty)).unwrap_or_default();
            let dr = bar_button(bui, "···", c.text2);
            egui::Popup::menu(&dr).width(220.0).show(|ui| {
                ui.checkbox(&mut app.full_width, "Full width");
                if ui.button("⧉  Duplicate").clicked() {
                    duplicate_page(app, pid, now);
                    ui.close();
                }
                let is_folder = app.ws.get(pid).map(|e| e.page.folder).unwrap_or(false);
                let mut f = is_folder;
                if ui.checkbox(&mut f, "Keep sub-pages in a folder").on_hover_text("Sub-pages are saved inside a folder named like this page").changed() {
                    app.ws.set_folder(pid, f, now);
                }
                let is_db = app.ws.get(pid).map(|e| e.page.db.is_some()).unwrap_or(false);
                if !is_db && ui.button("🗃  Turn into database").on_hover_text("Sub-pages become rows of a table").clicked() {
                    if let Some(e) = app.ws.get_mut(pid) {
                        e.page.db = Some(crate::model::Db::new());
                    }
                    app.ws.touch(pid, now - 10.0);
                    ui.close();
                }
                if !crate::platform::IS_WEB && ui.button("🔎  Show file").clicked() {
                    if let Some(e) = app.ws.get(pid) {
                        reveal(&e.path);
                    }
                    ui.close();
                }
                if !crate::platform::IS_WEB && ui.button("📋  Copy file location").clicked() {
                    if let Some(e) = app.ws.get(pid) {
                        ui.ctx().copy_text(e.path.display().to_string());
                    }
                    ui.close();
                }
                ui.separator();
                if ui.button("🗑  Move to Trash").clicked() {
                    app.ws.set_trashed(pid, true, now);
                    app.current = app.ws.children("").first().map(|p| p.id.clone());
                    ui.close();
                }
                ui.separator();
                if let Some(e) = app.ws.get(pid) {
                    let words = e.page.plain_text().split_whitespace().count();
                    ui.label(RichText::new(format!("{words} words")).size(11.0).color(c.text3));
                    ui.label(RichText::new(format!("Edited {}", ago(e.page.updated))).size(11.0).color(c.text3));
                }
            });
            let star = if fav { bar_button(bui, "★", Color32::from_rgb(242, 180, 40)) } else { bar_button(bui, "☆", c.text2) };
            if star.on_hover_text(if fav { "Remove from Favorites" } else { "Add to Favorites" }).clicked() {
                if let Some(e) = app.ws.get_mut(pid) {
                    e.page.favorite = !fav;
                }
                app.ws.touch(pid, now - 10.0);
            }
            let status = if let Some((s, t)) = &app.status {
                if now - t < 2.0 { s.clone() } else { String::new() }
            } else {
                String::new()
            };
            let status = if !status.is_empty() {
                status
            } else if dirty {
                "Saving…".into()
            } else {
                format!("Edited {}", ago(upd))
            };
            if !app.narrow || status != format!("Edited {}", ago(upd)) {
                bui.label(RichText::new(status).size(13.0).color(c.text3));
            }
        });
    }

    // Take the page out so the editor can borrow the rest of the app freely.
    let mut page = app.ws.get(pid).unwrap().page.clone();
    let before_sig = signature(&page);

    let scroll_source = if app.ed.drag.is_some() { egui::containers::scroll_area::ScrollSource { drag: egui::containers::scroll_area::DragScroll::Never, ..egui::containers::scroll_area::ScrollSource::ALL } } else { egui::containers::scroll_area::ScrollSource::ALL };
    egui::ScrollArea::vertical().id_salt(("page", pid)).auto_shrink([false, false]).scroll_source(scroll_source).show(ui, |ui| {
        let avail = ui.available_width();
        // Cover
        if has_cover(&page) {
            let (r, resp) = ui.allocate_exact_size(vec2(avail, if app.narrow { 130.0 } else { 200.0 }), Sense::hover());
            paint_cover(app, ui, r, page.cover, &page.cover_image);
            let picker_open = matches!(app.ed.menu.as_ref().map(|m| &m.kind), Some(MenuKind::Cover));
            if resp.hovered() || ui.rect_contains_pointer(r) || picker_open {
                let br = Rect::from_min_size(pos2(r.right() - 250.0, r.bottom() - 36.0), vec2(240.0, 26.0));
                let mut cu = ui.new_child(egui::UiBuilder::new().max_rect(br).layout(Layout::right_to_left(Align::Center)));
                if cu.add(egui::Button::new(RichText::new("Remove").size(12.0).color(c.text2)).fill(c.popup)).clicked() {
                    page.cover = 0;
                    page.cover_image.clear();
                }
                let cb = cu.add(egui::Button::new(RichText::new("Change cover").size(12.0).color(c.text2)).fill(c.popup));
                if cb.clicked() {
                    open_menu(app, ui.ctx(), MenuKind::Cover, pos2(r.right() - 470.0, r.bottom() + 4.0));
                }
            }
        }
        cover_picker(app, ui.ctx(), &mut page, &c);

        let is_db = page.db.is_some();
        // Phone: narrow margins with room on the left for the block handle.
        let (width, gutter) = if app.narrow {
            ((avail - 36.0 - 14.0).max(200.0), 36.0)
        } else {
            let width = if app.full_width || is_db { (avail - 192.0).max(300.0) } else { 708.0f32.min(avail - 120.0).max(300.0) };
            (width, ((avail - width) / 2.0).max(56.0))
        };
        ui.horizontal_top(|ui| {
            ui.add_space(gutter);
            ui.vertical(|ui| {
                ui.set_width(width);
                ui.set_max_width(width);
                page_header(app, ui, &mut page, &c);
                ui.add_space(6.0);
                if is_db {
                    crate::db::db_view(app, ui, &mut page, &c, now);
                    return;
                }
                if app.ws.get(&page.parent).map(|e| e.page.db.is_some()).unwrap_or(false) {
                    crate::db::row_props(app, ui, &mut page, &c, now);
                }
                blocks_ui(app, ui, &mut page, &c, now);
                // Click below the last block adds / focuses a trailing text block.
                let (r, resp) = ui.allocate_exact_size(vec2(width, (ui.available_height()).max(160.0)), Sense::click());
                let _ = r;
                if resp.clicked() {
                    let last_empty = page.blocks.last().map(|b| b.kind == Kind::Text && b.text.is_empty()).unwrap_or(false);
                    if !last_empty {
                        page.blocks.push(Block::new(Kind::Text, ""));
                    }
                    let id = page.blocks.last().unwrap().id;
                    app.ed.focus = Some((id, 0));
                }
            });
        });
    });

    // Pictures dragged in from Finder / Explorer become image blocks.
    let dropped: Vec<std::path::PathBuf> = ui.ctx().input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty()).collect());
    if !dropped.is_empty() && page.db.is_none() && !crate::platform::IS_WEB {
        for path in dropped {
            if is_image_path(&path) {
                let name = path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                let bytes = crate::vfs::read(&path).unwrap_or_default();
                match app.ws.import_asset(&name, &bytes) {
                    Ok(rel) => {
                        let mut b = Block::new(Kind::Image, "");
                        b.extra = rel;
                        let at = page.blocks.iter().rposition(|b| !(b.kind == Kind::Text && b.text.is_empty())).map(|i| i + 1).unwrap_or(0);
                        page.blocks.insert(at, b);
                    }
                    Err(e) => app.set_status(format!("Could not add picture: {e}"), now),
                }
            }
        }
    }
    if ui.ctx().input(|i| !i.raw.hovered_files.is_empty()) {
        let r = ui.max_rect();
        let p = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Foreground, Id::new("drop_overlay")));
        p.rect_filled(r, 0.0, Color32::from_rgba_unmultiplied(35, 131, 226, 30));
        p.text(r.center(), egui::Align2::CENTER_CENTER, "Drop pictures to add them to this page", FontId::proportional(18.0), c.accent);
    }

    if signature(&page) != before_sig {
        page.updated = crate::store::now_secs();
        if let Some(e) = app.ws.get_mut(pid) {
            e.page = page;
        }
        app.ws.touch(pid, now);
    } else if let Some(e) = app.ws.get_mut(pid) {
        // Opening or closing a toggle only changes the view, not the file:
        // keep it in memory without saving.
        for (dst, src) in e.page.blocks.iter_mut().zip(page.blocks.iter()) {
            if dst.id == src.id {
                dst.open = src.open;
            }
        }
    }
    if let Some(sub) = app.ed.pending_open.take() {
        app.new_page_opened(&sub);
    }
}

fn signature(p: &crate::model::Page) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    p.title.hash(&mut h);
    p.icon.hash(&mut h);
    p.cover.hash(&mut h);
    p.db.hash(&mut h);
    p.props.hash(&mut h);
    p.favorite.hash(&mut h);
    p.cover_image.hash(&mut h);
    p.folder.hash(&mut h);
    for b in &p.blocks {
        b.id.hash(&mut h);
        (b.kind as u8).hash(&mut h);
        b.text.hash(&mut h);
        b.checked.hash(&mut h);
        b.indent.hash(&mut h);
        b.extra.hash(&mut h);
        b.table.hash(&mut h);
        b.color.hash(&mut h);
        b.desc.hash(&mut h);
        b.size.hash(&mut h);
    }
    h.finish()
}

fn page_header(app: &mut App, ui: &mut Ui, page: &mut crate::model::Page, c: &Colors) {
    let top = ui.cursor().min;
    let hovering = ui.rect_contains_pointer(Rect::from_min_size(top, vec2(ui.available_width(), 150.0)));
    let covered = has_cover(page);
    if !covered {
        ui.add_space(if app.narrow { 20.0 } else { 50.0 });
    } else if !page.icon.is_empty() {
        ui.add_space(-40.0);
    } else {
        ui.add_space(20.0);
    }
    // Icon
    if !page.icon.is_empty() {
        let r = ui.add(egui::Button::new(RichText::new(&page.icon).size(if app.narrow { 48.0 } else { 62.0 })).frame(false));
        if r.clicked() {
            app.ed.icon_picker = !app.ed.icon_picker;
        }
        icon_picker(app, ui, &r, page, c);
    }
    // "Add icon" / "Add cover" row
    let need_row = page.icon.is_empty() || !covered;
    ui.horizontal(|ui| {
        ui.set_height(if need_row { 26.0 } else { 4.0 });
        if hovering && need_row {
            if page.icon.is_empty() {
                let r = small_button(ui, "☺ Add icon", c);
                if r.clicked() {
                    page.icon = "📄".into();
                    app.ed.icon_picker = true;
                }
            }
            if !covered {
                let r = small_button(ui, "🖼 Add cover", c);
                if r.clicked() {
                    page.cover = 1;
                    open_menu(app, ui.ctx(), MenuKind::Cover, r.rect.left_bottom() + vec2(0.0, 210.0));
                }
            }
        }
    });
    // Title
    let tid = Id::new(("title", &page.id));
    if app.ed.focus_title {
        app.ed.focus_title = false;
        ui.memory_mut(|m| m.request_focus(tid));
    }
    let has_focus = ui.memory(|m| m.has_focus(tid));
    if has_focus && take_key(ui, Key::Enter, false, false) {
        // Enter in the title moves into the page body.
        if page.blocks.is_empty() {
            page.blocks.push(Block::new(Kind::Text, ""));
        }
        app.ed.focus = Some((page.blocks[0].id, 0));
    }
    if has_focus && key_pressed(ui, Key::ArrowDown) {
        if let Some(b) = page.blocks.first() {
            app.ed.focus = Some((b.id, 0));
        }
    }
    let title_size = if app.narrow { 30.0 } else { 40.0 };
    let te = egui::TextEdit::multiline(&mut page.title)
        .id(tid)
        .frame(egui::Frame::NONE)
        .hint_text(RichText::new("Untitled").color(c.text3).size(title_size).family(theme::bold()))
        .font(FontId::new(title_size, theme::bold()))
        .text_color(c.text)
        .desired_rows(1)
        .desired_width(f32::INFINITY);
    let tr = ui.add(te);
    crate::platform::keyboard_zone(tr.rect);
    if page.title.contains('\n') {
        page.title = page.title.replace('\n', " ");
    }
}

fn icon_picker(app: &mut App, ui: &mut Ui, anchor: &egui::Response, page: &mut crate::model::Page, c: &Colors) {
    if !app.ed.icon_picker {
        return;
    }
    let mut close = false;
    let area = egui::Area::new(Id::new("icon_picker"))
        .order(egui::Order::Foreground)
        .fixed_pos(anchor.rect.left_bottom() + vec2(0.0, 4.0))
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).fill(c.popup).show(ui, |ui| {
                ui.set_width(330.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Pick an icon").size(12.0).color(c.text2));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if small_button(ui, "Remove", c).clicked() {
                            page.icon.clear();
                            close = true;
                        }
                    });
                });
                ui.separator();
                egui::Grid::new("emoji_grid").spacing(vec2(2.0, 2.0)).show(ui, |ui| {
                    for (k, e) in EMOJIS.iter().enumerate() {
                        if ui.add(egui::Button::new(RichText::new(*e).size(22.0)).frame(false).min_size(vec2(30.0, 30.0))).clicked() {
                            page.icon = e.to_string();
                            close = true;
                        }
                        if k % 10 == 9 {
                            ui.end_row();
                        }
                    }
                });
            });
        });
    let clicked_out = ui.input(|i| i.pointer.any_click()) && !area.response.hovered() && !anchor.hovered() && !area.response.contains_pointer();
    if close || clicked_out || ui.input(|i| i.key_pressed(Key::Escape)) {
        app.ed.icon_picker = false;
    }
}

// ------------------------------------------------------------------ blocks

fn slash_items(filter: &str) -> Vec<Kind> {
    let f = filter.to_lowercase();
    let mut all: Vec<Kind> = Kind::MENU.to_vec();
    all.push(Kind::PageLink);
    all.push(Kind::DbLink);
    all.into_iter()
        .filter(|k| f.is_empty() || k.label().to_lowercase().contains(&f) || k.keywords().contains(&f))
        .collect()
}

fn blocks_ui(app: &mut App, ui: &mut Ui, page: &mut crate::model::Page, c: &Colors, now: f64) {
    let ctx = ui.ctx().clone();
    let mut acts: Vec<Act> = Vec::new();
    let mut number = 0usize;
    let mut prev_kind = Kind::Text;
    let only_block = page.blocks.len() == 1;
    let mut slash_rect: Option<(u64, Rect, String)> = None;

    // Keys for the "Paste as" menu go to it before any text field sees them.
    let paste_menu = app.ed.menu.clone().filter(|m| matches!(&m.kind, MenuKind::Paste { block, .. } if page.blocks.iter().any(|b| b.id == *block)));
    if let Some(m) = &paste_menu {
        if let MenuKind::Paste { block, url, start } = &m.kind {
            if take_key(ui, Key::ArrowDown, false, false) {
                app.ed.paste_sel = (app.ed.paste_sel + 1) % 3;
            }
            if take_key(ui, Key::ArrowUp, false, false) {
                app.ed.paste_sel = (app.ed.paste_sel + 2) % 3;
            }
            if take_key(ui, Key::Enter, false, false) {
                acts.push(Act::PasteAs { block: *block, url: url.clone(), start: *start, choice: app.ed.paste_sel as u8 });
                app.ed.menu = None;
            } else if take_key(ui, Key::Escape, false, false) || ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Text(_)))) {
                app.ed.menu = None;
            }
        }
    }

    let mut spans: Vec<(f32, f32)> = Vec::with_capacity(page.blocks.len());
    let left = ui.cursor().left();
    let right = left + ui.available_width();
    for i in 0..page.blocks.len() {
        let b = &page.blocks[i];
        if b.kind == Kind::Numbered {
            number = if prev_kind == Kind::Numbered { number + 1 } else { 1 };
        }
        prev_kind = b.kind;
        let top = ui.cursor().top();
        if let Some(sr) = block_row(app, ui, &mut page.blocks, i, number, only_block, c, &mut acts) {
            slash_rect = Some(sr);
        }
        spans.push((top, ui.cursor().top()));
    }

    // Slash menu
    if let Some((bid, rect, filter)) = slash_rect {
        let items = slash_items(&filter);
        if let Some(i) = page.blocks.iter().position(|b| b.id == bid) {
            slash_menu(app, ui, rect, &items, i, c, &mut acts);
        }
    }

    // Dragging a block by its handle: show where it will land, drop on release.
    if let Some(did) = app.ed.drag {
        if let Some(from) = page.blocks.iter().position(|b| b.id == did) {
            let pointer = ctx.input(|i| i.pointer.latest_pos());
            if let Some(p) = pointer {
                let n = spans.len();
                let mut gap = n;
                for (k, (t, b)) in spans.iter().enumerate() {
                    if p.y < (t + b) / 2.0 {
                        gap = k;
                        break;
                    }
                }
                let y = if gap < n { spans[gap].0 } else { spans.last().map(|s| s.1).unwrap_or(p.y) };
                let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, Id::new("drag_line")));
                let group_end = list_group_end(&page.blocks, from);
                let no_move = gap >= from && gap <= group_end + 1;
                if !no_move {
                    painter.line_segment([pos2(left, y), pos2(right, y)], Stroke::new(3.0, c.accent.gamma_multiply(0.8)));
                }
                // Ghost of the block under the pointer.
                let label: String = {
                    let b = &page.blocks[from];
                    let t = if b.text.is_empty() { b.kind.label().to_string() } else { b.text.replace('\n', " ") };
                    t.chars().take(48).collect()
                };
                let g = painter.layout_no_wrap(label, FontId::proportional(14.0), c.text);
                let gr = Rect::from_min_size(p + vec2(14.0, -12.0), g.size() + vec2(20.0, 10.0));
                painter.rect_filled(gr, 5.0, c.popup.gamma_multiply(0.92));
                painter.rect_stroke(gr, 5.0, Stroke::new(1.0, c.line), egui::StrokeKind::Inside);
                painter.galley(gr.min + vec2(10.0, 5.0), g, c.text);
                ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
                // Scroll when close to the top or bottom edge.
                let clip = ui.clip_rect();
                if p.y < clip.top() + 50.0 {
                    ui.scroll_with_delta(vec2(0.0, 14.0));
                } else if p.y > clip.bottom() - 50.0 {
                    ui.scroll_with_delta(vec2(0.0, -14.0));
                }
                ctx.request_repaint();
                let released = !ctx.input(|i| i.pointer.primary_down());
                if released {
                    if !no_move {
                        acts.push(Act::MoveTo { from, to: gap });
                    }
                    app.ed.drag = None;
                } else if ctx.input(|i| i.key_pressed(Key::Escape)) {
                    app.ed.drag = None;
                }
            } else {
                app.ed.drag = None;
            }
        }
    }

    block_menu_float(app, &ctx, page, c, &mut acts);
    paste_menu_float(app, &ctx, page, c, &mut acts);

    // Apply actions
    for act in acts {
        apply(app, &ctx, page, act, now);
    }
    if page.blocks.is_empty() {
        page.blocks.push(Block::new(Kind::Text, ""));
    }
}

/// Index of the last block that moves together with block `i` (its nested list items).
fn list_group_end(blocks: &[Block], i: usize) -> usize {
    let mut end = i;
    if blocks[i].kind.is_list() {
        let ind = blocks[i].indent;
        while end + 1 < blocks.len() && blocks[end + 1].kind.is_list() && blocks[end + 1].indent > ind {
            end += 1;
        }
    }
    end
}

fn block_menu_float(app: &mut App, ctx: &egui::Context, page: &crate::model::Page, c: &Colors, acts: &mut Vec<Act>) {
    let Some(m) = app.ed.menu.clone() else { return };
    let MenuKind::Block(bid) = m.kind else { return };
    let Some(i) = page.blocks.iter().position(|b| b.id == bid) else { return };
    let b = &page.blocks[i];
    let dark = app.settings.dark;
    let (close, outside) = float_menu(ctx, Id::new("block_menu"), &m, 240.0, c, |ui| {
        let mut close = false;
        ui.label(RichText::new("Turn into").size(11.0).color(c.text3));
        egui::Grid::new("turn_into").num_columns(2).spacing(vec2(2.0, 2.0)).show(ui, |ui| {
            for (k, kind) in Kind::MENU.iter().enumerate() {
                let sel = b.kind == *kind;
                let r = ui.add_sized(
                    [112.0, 24.0],
                    egui::Button::new(RichText::new(format!("{}  {}", kind.icon(), kind.label())).size(12.5).color(c.text)).fill(if sel { c.selected } else { Color32::TRANSPARENT }),
                );
                if r.clicked() {
                    acts.push(Act::TurnInto(i, *kind));
                    close = true;
                }
                if k % 2 == 1 {
                    ui.end_row();
                }
            }
        });
        ui.separator();
        ui.label(RichText::new("Background").size(11.0).color(c.text3));
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
            for (ci, name) in theme::COLOR_NAMES.iter().enumerate() {
                let (r, resp) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::click());
                let bg = if ci == 0 { c.bg } else { theme::block_bg(ci as u8, dark) };
                ui.painter().rect(r, 4.0, bg, Stroke::new(if b.color as usize == ci { 2.0 } else { 1.0 }, if b.color as usize == ci { c.accent } else { c.line }), egui::StrokeKind::Inside);
                if resp.on_hover_text(*name).clicked() {
                    acts.push(Act::Color(i, ci as u8));
                    close = true;
                }
            }
        });
        ui.separator();
        if menu_item(ui, "⬆", "Move up", c) {
            acts.push(Act::MoveUp(i));
        }
        if menu_item(ui, "⬇", "Move down", c) {
            acts.push(Act::MoveDown(i));
        }
        if menu_item(ui, "⧉", "Duplicate", c) {
            acts.push(Act::Duplicate(i));
            close = true;
        }
        if menu_item_colored(ui, "🗑", "Delete", Color32::from_rgb(220, 70, 70), c) {
            acts.push(Act::Delete(i));
            close = true;
        }
        ui.label(RichText::new("Tip: drag ⋮⋮ to move a block").size(11.0).color(c.text3));
        close
    });
    if close || outside {
        close_menu(app, ctx);
    }
}

fn paste_menu_float(app: &mut App, ctx: &egui::Context, page: &crate::model::Page, c: &Colors, acts: &mut Vec<Act>) {
    let Some(m) = app.ed.menu.clone() else { return };
    let MenuKind::Paste { block, url, start } = &m.kind else { return };
    if !page.blocks.iter().any(|b| b.id == *block) {
        return;
    }
    let (choice, outside) = float_menu(ctx, Id::new("paste_menu"), &m, 230.0, c, |ui| {
        let mut choice = None;
        ui.label(RichText::new("Paste as").size(12.0).color(c.text2));
        ui.add_space(2.0);
        let items = [("@", "Mention", "Short link with the site name"), ("🔗", "URL", "Keep the plain address"), ("🔖", "Preview", "A card with the site and address")];
        for (k, (icon, label, hint)) in items.iter().enumerate() {
            let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::click());
            if k == app.ed.paste_sel || resp.hovered() {
                ui.painter().rect_filled(r, 5.0, c.hover);
            }
            if resp.hovered() && ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO) {
                app.ed.paste_sel = k;
            }
            ui.painter().text(pos2(r.left() + 16.0, r.center().y), egui::Align2::CENTER_CENTER, *icon, FontId::proportional(15.0), c.text2);
            ui.painter().text(pos2(r.left() + 34.0, r.top() + 13.0), egui::Align2::LEFT_CENTER, *label, FontId::proportional(15.0), c.text);
            ui.painter().text(pos2(r.left() + 34.0, r.top() + 29.0), egui::Align2::LEFT_CENTER, *hint, FontId::proportional(11.0), c.text3);
            if resp.clicked() {
                choice = Some(k as u8);
            }
        }
        choice
    });
    if let Some(ch) = choice {
        acts.push(Act::PasteAs { block: *block, url: url.clone(), start: *start, choice: ch });
        app.ed.menu = None;
    } else if outside {
        app.ed.menu = None;
    }
}

fn slash_menu(app: &mut App, ui: &mut Ui, rect: Rect, items: &[Kind], i: usize, c: &Colors, acts: &mut Vec<Act>) {
    let screen = ui.ctx().content_rect();
    let h = (items.len().max(1) as f32 * 46.0 + 36.0).min(360.0);
    let mut pos = rect.left_bottom() + vec2(0.0, 4.0);
    if pos.y + h > screen.bottom() {
        pos.y = rect.top() - h - 4.0;
    }
    egui::Area::new(Id::new("slash_menu"))
        .order(egui::Order::Foreground)
        .fixed_pos(pos)
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).fill(c.popup).inner_margin(egui::Margin::same(6)).show(ui, |ui| {
                ui.set_width(300.0);
                ui.label(RichText::new(if items.is_empty() { "No results" } else { "Basic blocks" }).size(11.0).color(c.text3));
                egui::ScrollArea::vertical().max_height(h - 30.0).show(ui, |ui| {
                    for (k, kind) in items.iter().enumerate() {
                        let sel = k == app.ed.slash_sel;
                        let (r, resp) = row(ui, sel, c, 44.0);
                        let p = ui.painter();
                        let ic = Rect::from_min_size(r.min + vec2(4.0, 4.0), vec2(36.0, 36.0));
                        p.rect(ic, 5.0, c.bg, Stroke::new(1.0, c.line), egui::StrokeKind::Inside);
                        p.text(ic.center(), egui::Align2::CENTER_CENTER, kind.icon(), FontId::proportional(15.0), c.text);
                        p.text(pos2(r.left() + 50.0, r.top() + 14.0), egui::Align2::LEFT_CENTER, kind.label(), FontId::proportional(14.0), c.text);
                        p.text(pos2(r.left() + 50.0, r.top() + 31.0), egui::Align2::LEFT_CENTER, kind.hint(), FontId::proportional(11.5), c.text2);
                        if sel {
                            resp.scroll_to_me(None);
                        }
                        if resp.hovered() && ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO) {
                            app.ed.slash_sel = k;
                        }
                        if resp.clicked() {
                            acts.push(slash_pick(*kind, i));
                        }
                    }
                });
            });
        });
}

fn slash_pick(kind: Kind, i: usize) -> Act {
    match kind {
        Kind::PageLink => Act::NewSubpage(i, false),
        Kind::DbLink => Act::NewSubpage(i, true),
        _ => Act::TurnInto(i, kind),
    }
}

#[allow(clippy::too_many_arguments)]
fn block_row(
    app: &mut App,
    ui: &mut Ui,
    blocks: &mut [Block],
    i: usize,
    number: usize,
    only_block: bool,
    c: &Colors,
    acts: &mut Vec<Act>,
) -> Option<(u64, Rect, String)> {
    let ctx = ui.ctx().clone();
    let bid = blocks[i].id;
    let te_id = Id::new(("blk", bid));
    let mut slash_out = None;

    // Space above some blocks
    match blocks[i].kind {
        Kind::H1 => ui.add_space(18.0),
        Kind::H2 => ui.add_space(12.0),
        Kind::H3 => ui.add_space(8.0),
        _ => ui.add_space(1.0),
    }

    let row_top = ui.cursor().min;
    let full_w = ui.available_width();
    let bg_idx = ui.painter().add(egui::Shape::Noop);

    // Apply pending focus requests
    if let Some((fid, pos)) = app.ed.focus {
        if fid == bid {
            app.ed.focus = None;
            if blocks[i].kind.is_text_like() {
                set_cursor(&ctx, te_id, pos.min(blocks[i].text.chars().count()));
            }
        }
    }

    let focused = ctx.memory(|m| m.has_focus(te_id));
    let b = &mut blocks[i];

    // Keyboard handling before the text field sees the keys.
    let mut up_jump = None;
    let mut pasted: Option<(String, usize)> = None;
    if focused && b.kind.is_text_like() {
        let cur = cursor_of(&ctx, te_id);
        let (cpos, has_sel) = cur.map(|r| (usize::from(r.primary.index), r.primary.index != r.secondary.index)).unwrap_or((0, false));
        if b.kind != Kind::Code {
            let url = ui.input(|i| {
                i.events.iter().find_map(|e| match e {
                    egui::Event::Paste(t) if is_url(t.trim()) => Some(t.trim().to_string()),
                    _ => None,
                })
            });
            if let Some(u) = url {
                let start = cur.map(|r| usize::from(r.primary.index).min(usize::from(r.secondary.index))).unwrap_or(0);
                pasted = Some((u, start));
            }
        }
        let chars = b.text.chars().count();
        let slash_open = b.kind != Kind::Code
            && b.text.starts_with('/')
            && !b.text.contains(' ')
            && b.text.len() < 24
            && app.ed.slash_dismissed != Some(bid);

        if slash_open {
            let items = slash_items(&b.text[1..]);
            if take_key(ui, Key::ArrowDown, false, false) && !items.is_empty() {
                app.ed.slash_sel = (app.ed.slash_sel + 1) % items.len();
            }
            if take_key(ui, Key::ArrowUp, false, false) && !items.is_empty() {
                app.ed.slash_sel = (app.ed.slash_sel + items.len() - 1) % items.len();
            }
            if take_key(ui, Key::Escape, false, false) {
                app.ed.slash_dismissed = Some(bid);
            }
            if take_key(ui, Key::Enter, false, false) {
                if let Some(k) = items.get(app.ed.slash_sel.min(items.len().saturating_sub(1))) {
                    acts.push(slash_pick(*k, i));
                } else {
                    app.ed.slash_dismissed = Some(bid);
                }
            }
            if app.ed.slash_sel >= items.len().max(1) {
                app.ed.slash_sel = 0;
            }
        } else {
            if b.kind == Kind::Code {
                if take_key(ui, Key::Enter, false, true) || take_key(ui, Key::Enter, true, false) {
                    acts.push(Act::InsertAfter(i, Kind::Text));
                }
                if take_key(ui, Key::Tab, false, false) {
                    let byte = byte_of(&b.text, cpos);
                    b.text.insert_str(byte, "    ");
                    set_cursor(&ctx, te_id, cpos + 4);
                }
            } else if take_key(ui, Key::Enter, false, false) {
                if b.kind.is_list() && b.text.is_empty() {
                    if b.indent > 0 {
                        acts.push(Act::Indent(i, -1));
                    } else {
                        acts.push(Act::TurnInto(i, Kind::Text));
                    }
                } else {
                    acts.push(Act::Split { i, at: cpos });
                }
            }
            if take_key(ui, Key::Tab, false, false) {
                acts.push(Act::Indent(i, 1));
            }
            if take_key(ui, Key::Tab, true, false) {
                acts.push(Act::Indent(i, -1));
            }
            if cpos == 0 && !has_sel && take_key(ui, Key::Backspace, false, false) {
                if b.kind != Kind::Text && b.kind != Kind::Code {
                    if b.indent > 0 {
                        acts.push(Act::Indent(i, -1));
                    } else {
                        acts.push(Act::TurnInto(i, Kind::Text));
                    }
                } else if b.kind == Kind::Code && !b.text.is_empty() {
                    // nothing
                } else {
                    acts.push(Act::MergeUp(i));
                }
            }
            if !has_sel {
                let before_nl = !b.text.chars().take(cpos).any(|c| c == '\n');
                let after_nl = !b.text.chars().skip(cpos).any(|c| c == '\n');
                if key_pressed(ui, Key::ArrowUp) && before_nl {
                    up_jump = Some((true, cpos));
                }
                if key_pressed(ui, Key::ArrowDown) && after_nl {
                    up_jump = Some((false, cpos));
                }
                if key_pressed(ui, Key::ArrowLeft) && cpos == 0 && i > 0 {
                    acts.push(Act::FocusPrev(i));
                }
                if key_pressed(ui, Key::ArrowRight) && cpos == chars && i + 1 < usize::MAX {
                    // handled by FocusNext only when at end
                    acts.push(Act::FocusNext(i));
                }
            }
        }
        if slash_open {
            slash_out = Some((bid, Rect::NOTHING, b.text[1..].to_string()));
        }
    }

    // Layout: [gutter 44][prefix][content]
    let indent_px = b.indent as f32 * 26.0;
    let mut content_rect = Rect::NOTHING;
    let mut touch_flags: Option<(bool, bool)> = None;
    let resp_rect = ui
        .horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.add_space(indent_px);
            let (font, boldb, color) = match b.kind {
                Kind::H1 => (FontId::new(30.0, theme::bold()), true, c.text),
                Kind::H2 => (FontId::new(24.0, theme::bold()), true, c.text),
                Kind::H3 => (FontId::new(20.0, theme::bold()), true, c.text),
                Kind::Todo if b.checked => (FontId::proportional(16.0), false, c.text3),
                _ => (FontId::proportional(16.0), false, c.text),
            };
            let line_h = font.size * 1.5;

            // Prefix
            match b.kind {
                Kind::Bullet => {
                    let (r, _) = ui.allocate_exact_size(vec2(26.0, line_h), Sense::hover());
                    let glyph = ["•", "◦", "▪"][(b.indent % 3) as usize];
                    ui.painter().text(r.center() - vec2(2.0, 0.0), egui::Align2::CENTER_CENTER, glyph, FontId::proportional(20.0), c.text);
                }
                Kind::Numbered => {
                    let (r, _) = ui.allocate_exact_size(vec2(26.0, line_h), Sense::hover());
                    let label = if b.indent % 3 == 1 {
                        format!("{}.", (b'a' + ((number - 1) % 26) as u8) as char)
                    } else {
                        format!("{number}.")
                    };
                    ui.painter().text(pos2(r.left() + 2.0, r.center().y), egui::Align2::LEFT_CENTER, label, FontId::proportional(16.0), c.text);
                }
                Kind::Todo => {
                    let (r, resp) = ui.allocate_exact_size(vec2(26.0, line_h), Sense::click());
                    let bx = Rect::from_center_size(pos2(r.left() + 9.0, r.center().y), vec2(16.0, 16.0));
                    if b.checked {
                        ui.painter().rect_filled(bx, 3.0, c.accent);
                        ui.painter().text(bx.center(), egui::Align2::CENTER_CENTER, "✔", FontId::proportional(12.0), Color32::WHITE);
                    } else {
                        ui.painter().rect_stroke(bx, 3.0, Stroke::new(1.5, c.text), egui::StrokeKind::Inside);
                    }
                    if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        b.checked = !b.checked;
                    }
                }
                Kind::Toggle => {
                    let (r, resp) = ui.allocate_exact_size(vec2(26.0, line_h), Sense::click());
                    if resp.hovered() {
                        ui.painter().rect_filled(Rect::from_center_size(r.center() - vec2(3.0, 0.0), vec2(20.0, 20.0)), 3.0, c.hover);
                    }
                    ui.painter().text(r.center() - vec2(3.0, 0.0), egui::Align2::CENTER_CENTER, if b.open { "⏷" } else { "⏵" }, FontId::proportional(13.0), c.text);
                    if resp.clicked() {
                        b.open = !b.open;
                    }
                }
                Kind::Quote => {
                    ui.add_space(14.0);
                }
                _ => {}
            }

            let w = ui.available_width();
            match b.kind {
                Kind::Divider => {
                    let (r, _) = ui.allocate_exact_size(vec2(w, 22.0), Sense::click());
                    ui.painter().hline(r.x_range(), r.center().y, Stroke::new(1.0, c.line));
                    content_rect = r;
                }
                Kind::PageLink => {
                    let target = b.extra.clone();
                    let (title, icon) = app
                        .ws
                        .get(&target)
                        .map(|e| (e.page.display_title().to_string(), if e.page.icon.is_empty() { "📄".to_string() } else { e.page.icon.clone() }))
                        .unwrap_or_else(|| (format!("{} (missing)", b.text), "📄".into()));
                    if b.text != title && app.ws.get(&target).is_some() {
                        b.text = title.clone();
                    }
                    let (r, resp) = ui.allocate_exact_size(vec2(w, 30.0), Sense::click());
                    if resp.hovered() {
                        ui.painter().rect_filled(r, 4.0, c.hover);
                    }
                    ui.painter().text(pos2(r.left() + 12.0, r.center().y), egui::Align2::CENTER_CENTER, icon, FontId::proportional(16.0), c.text);
                    let g = ui.painter().layout_no_wrap(title, FontId::new(16.0, FontFamily::Proportional), c.text);
                    let tp = pos2(r.left() + 26.0, r.center().y - g.size().y / 2.0);
                    let tw = g.size().x;
                    ui.painter().galley(tp, g, c.text);
                    ui.painter().hline(tp.x..=tp.x + tw, r.center().y + 10.0, Stroke::new(1.0, c.text3));
                    if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() && app.ws.get(&target).is_some() {
                        acts.push(Act::OpenPage(target));
                    }
                    content_rect = r;
                }
                Kind::Table => {
                    content_rect = table_ui(ui, b, c, w);
                }
                Kind::Image => {
                    content_rect = image_ui(app, ui, b, i, w, c, acts);
                }
                Kind::Bookmark => {
                    content_rect = bookmark_ui(ui, b, w, c);
                }
                Kind::Code => {
                    let fr = egui::Frame::new().fill(c.code).corner_radius(4).inner_margin(egui::Margin { left: 16, right: 16, top: 30, bottom: 16 }).show(ui, |ui| {
                        ui.set_width(w - 32.0);
                        let mut lay = |ui: &Ui, s: &dyn egui::TextBuffer, wrap: f32| {
                            let mut job = LayoutJob::simple(s.as_str().to_string(), FontId::monospace(14.0), c.text, wrap);
                            job.wrap.max_width = wrap;
                            ui.ctx().fonts_mut(|f| f.layout_job(job))
                        };
                        let cr = ui.add(
                            egui::TextEdit::multiline(&mut b.text)
                                .id(te_id)
                                .frame(egui::Frame::NONE)
                                .code_editor()
                                .layouter(&mut lay)
                                .desired_rows(1)
                                .desired_width(f32::INFINITY)
                                .hint_text(RichText::new("Write some code…").color(c.text3)),
                        );
                        crate::platform::keyboard_zone(cr.rect);
                    });
                    let r = fr.response.rect;
                    let lr = Rect::from_min_size(r.min + vec2(12.0, 6.0), vec2(160.0, 20.0));
                    ui.put(
                        lr,
                        egui::TextEdit::singleline(&mut b.extra)
                            .frame(egui::Frame::NONE)
                            .hint_text(RichText::new("plain text").color(c.text3).size(12.0))
                            .font(FontId::proportional(12.0))
                            .text_color(c.text2)
                            .id(Id::new(("lang", bid))),
                    );
                    content_rect = r;
                }
                _ => {
                    // Text-like block
                    let placeholder = match b.kind {
                        Kind::H1 => "Heading 1",
                        Kind::H2 => "Heading 2",
                        Kind::H3 => "Heading 3",
                        Kind::Bullet | Kind::Numbered => "List",
                        Kind::Todo => "To-do",
                        Kind::Toggle => "Toggle",
                        Kind::Quote => "Empty quote",
                        Kind::Callout => "Type something…",
                        _ => {
                            if focused || only_block {
                                "Write something, or type '/' for commands…"
                            } else {
                                ""
                            }
                        }
                    };
                    let strike = b.kind == Kind::Todo && b.checked;
                    let cc = *c;
                    let font2 = font.clone();
                    let marks = focused;
                    let mut lay = move |ui: &Ui, s: &dyn egui::TextBuffer, wrap: f32| {
                        let job = inline_job(s.as_str(), font2.clone(), boldb, color, &cc, strike, wrap, marks);
                        ui.ctx().fonts_mut(|f| f.layout_job(job))
                    };
                    // On touch screens a block that isn't being edited is drawn as plain
                    // text: a finger drag then scrolls the page, and a tap starts editing
                    // right where it lands.
                    let touch_static = app.touch && !focused;
                    let ctx2 = ctx.clone();
                    let ignore_click = app.ed.ignore_click;
                    let mut tapped = false;
                    let mut long_pressed = false;
                    let mut edit = |ui: &mut Ui, text: &mut String, width: f32| -> egui::Response {
                        let r = if touch_static {
                            let galley = lay(ui, &*text as &dyn egui::TextBuffer, width - 4.0);
                            let h = galley.size().y.max(font.size * 1.5);
                            let (rect, resp) = ui.allocate_exact_size(vec2(width, h + 4.0), Sense::click());
                            let gpos = rect.min + vec2(2.0, 2.0);
                            if text.is_empty() && !placeholder.is_empty() {
                                ui.painter().text(gpos, egui::Align2::LEFT_TOP, placeholder, font.clone(), c.text3);
                            } else {
                                ui.painter().galley(gpos, galley.clone(), color);
                            }
                            if resp.long_touched() {
                                // Press and hold: pick the block up to move it.
                                long_pressed = true;
                            } else if resp.clicked() && !ignore_click {
                                tapped = true;
                                let pos = resp.interact_pointer_pos().unwrap_or(gpos);
                                let at = usize::from(galley.cursor_from_pos(pos - gpos).index);
                                if let Some((_, _, url)) = link_spans(text).into_iter().find(|(a, b, _)| at > *a && at < *b) {
                                    open_url(&url);
                                } else {
                                    set_cursor(&ctx2, te_id, at);
                                }
                            }
                            resp
                        } else {
                            ui.add(
                                egui::TextEdit::multiline(text)
                                    .id(te_id)
                                    .frame(egui::Frame::NONE)
                                    .layouter(&mut lay)
                                    .desired_rows(1)
                                    .desired_width(width)
                                    .margin(egui::Margin::symmetric(2, 2))
                                    .hint_text(RichText::new(placeholder).color(c.text3).size(font.size).family(font.family.clone())),
                            )
                        };
                        crate::platform::keyboard_zone(r.rect);
                        r
                    };
                    if b.kind == Kind::Callout {
                        let fill = if b.color > 0 { theme::block_bg(b.color, app.settings.dark) } else { c.callout };
                        let fr = egui::Frame::new().fill(fill).corner_radius(4).inner_margin(egui::Margin { left: 12, right: 16, top: 12, bottom: 12 }).show(ui, |ui| {
                            ui.set_width(w - 28.0);
                            ui.horizontal_top(|ui| {
                                let ir = ui.add(egui::Button::new(RichText::new(&b.extra).size(20.0)).frame(false));
                                if ir.clicked() {
                                    let cur = EMOJIS.iter().position(|e| *e == b.extra).unwrap_or(0);
                                    b.extra = EMOJIS[(cur + 1) % EMOJIS.len()].to_string();
                                }
                                ui.add_space(6.0);
                                let ww = ui.available_width();
                                let r = edit(ui, &mut b.text, ww);
                                if !touch_static {
                                    link_click(ui, &r, te_id, &b.text, focused);
                                }
                            });
                        });
                        content_rect = fr.response.rect;
                    } else {
                        let r = edit(ui, &mut b.text, w);
                        if !touch_static {
                            link_click(ui, &r, te_id, &b.text, focused);
                        }
                        content_rect = r.rect;
                        if b.kind == Kind::Quote {
                            let bar = Rect::from_min_max(pos2(r.rect.left() - 14.0, r.rect.top() + 2.0), pos2(r.rect.left() - 11.0, r.rect.bottom() - 2.0));
                            ui.painter().rect_filled(bar, 0.0, c.text);
                        }
                    }
                    if touch_static {
                        touch_flags = Some((tapped, long_pressed));
                    }
                }
            }
        })
        .response
        .rect;

    if let Some(flags) = touch_flags {
        let (tapped, long_pressed) = flags;
        if long_pressed {
            app.ed.drag = Some(bid);
            app.ed.touch_sel = Some(bid);
            app.ed.ignore_click = true;
            ctx.memory_mut(|m| m.surrender_focus(te_id));
        } else if tapped {
            app.ed.touch_sel = Some(bid);
        } else if app.ed.ignore_click && ctx.input(|i| i.pointer.any_released()) {
            app.ed.ignore_click = false;
        }
    }
    if focused && app.touch {
        app.ed.touch_sel = Some(bid);
    }

    if let Some((url, start)) = pasted {
        let end = start + url.chars().count();
        let got: String = b.text.chars().skip(start).take(end - start).collect();
        if got == url {
            app.ed.paste_sel = 0;
            app.ed.menu = Some(Menu { kind: MenuKind::Paste { block: bid, url, start }, pos: content_rect.left_bottom() + vec2(0.0, 6.0), frame: ctx.cumulative_frame_nr() });
        }
    }

    // Toggle body
    if b.kind == Kind::Toggle && b.open {
        ui.horizontal_top(|ui| {
            ui.add_space(indent_px + 26.0);
            let cc = *c;
            let mut lay = move |ui: &Ui, s: &dyn egui::TextBuffer, wrap: f32| {
                let job = inline_job(s.as_str(), FontId::proportional(16.0), false, cc.text, &cc, false, wrap, true);
                ui.ctx().fonts_mut(|f| f.layout_job(job))
            };
            let tb = ui.add(
                egui::TextEdit::multiline(&mut b.extra)
                    .id(Id::new(("toggle_body", bid)))
                    .frame(egui::Frame::NONE)
                    .layouter(&mut lay)
                    .desired_rows(1)
                    .desired_width(ui.available_width())
                    .hint_text(RichText::new("Empty toggle. Click to add content.").color(c.text3).size(16.0)),
            );
            crate::platform::keyboard_zone(tb.rect);
        });
    }

    // Markdown shortcuts typed at the start of a text block.
    if b.kind == Kind::Text {
        let shortcuts: [(&str, Kind); 11] = [
            ("# ", Kind::H1),
            ("## ", Kind::H2),
            ("### ", Kind::H3),
            ("- ", Kind::Bullet),
            ("* ", Kind::Bullet),
            ("1. ", Kind::Numbered),
            ("[] ", Kind::Todo),
            ("[ ] ", Kind::Todo),
            ("> ", Kind::Quote),
            ("```", Kind::Code),
            ("---", Kind::Divider),
        ];
        for (p, k) in shortcuts {
            if b.text.starts_with(p) && focused {
                b.text = b.text[p.len()..].to_string();
                b.kind = k;
                b.fix_kind_data();
                if k == Kind::Divider {
                    b.text.clear();
                    acts.push(Act::InsertAfter(i, Kind::Text));
                } else {
                    app.ed.focus = Some((bid, 0));
                }
                break;
            }
        }
    }
    if !b.text.starts_with('/') && app.ed.slash_dismissed == Some(bid) {
        app.ed.slash_dismissed = None;
    }

    if let Some((up, _cpos)) = up_jump {
        if up {
            acts.push(Act::FocusPrev(i));
        } else {
            acts.push(Act::FocusNext(i));
        }
    }

    if b.color > 0 && b.kind != Kind::Callout {
        let bottom = resp_rect.bottom().max(content_rect.bottom());
        let left = if b.kind == Kind::Quote { content_rect.left() - 14.0 } else { row_top.x + indent_px };
        let r = Rect::from_min_max(pos2(left, row_top.y), pos2(row_top.x + full_w, bottom));
        ui.painter().set(bg_idx, egui::Shape::rect_filled(r.expand2(vec2(0.0, 1.0)), 3.0, theme::block_bg(b.color, app.settings.dark)));
    }

    // Hover gutter with "+" and "⋮⋮"
    let row_rect = Rect::from_min_max(pos2(row_top.x - 52.0, row_top.y), pos2(row_top.x + full_w, resp_rect.bottom().max(content_rect.bottom())));
    let menu_here = matches!(app.ed.menu.as_ref().map(|m| &m.kind), Some(MenuKind::Block(x)) if *x == bid);
    let show_gutter = if app.touch {
        // No hover on touch screens: show the handle on the block being edited.
        focused || menu_here || app.ed.drag == Some(bid) || app.ed.touch_sel == Some(bid)
    } else {
        ui.rect_contains_pointer(row_rect) || menu_here
    };
    if show_gutter {
        let y = row_top.y + match b.kind {
            Kind::H1 => 22.0,
            Kind::H2 => 18.0,
            Kind::H3 => 16.0,
            Kind::Code | Kind::Callout => 22.0,
            _ => 13.0,
        };
        let x = row_top.x + indent_px;
        let plus = Rect::from_center_size(pos2(x - 40.0, y), vec2(20.0, 22.0));
        let grip = if app.touch { Rect::from_center_size(pos2(x - 19.0, y), vec2(36.0, 40.0)) } else { Rect::from_center_size(pos2(x - 18.0, y), vec2(18.0, 22.0)) };
        // Hover buttons are painted in the margin without taking layout space,
        // so the block itself never moves.
        if !app.narrow {
            let pr = ui.interact(plus, Id::new(("plus", bid)), Sense::click());
            if pr.hovered() {
                ui.painter().rect_filled(plus, 4.0, c.hover);
            }
            ui.painter().text(plus.center(), egui::Align2::CENTER_CENTER, "+", FontId::proportional(17.0), c.text3);
            let pr = pr.on_hover_text("Click to add a block below");
            if pr.clicked() {
                acts.push(Act::InsertAfter(i, Kind::Text));
            }
        }
        let gr = ui.interact(grip, Id::new(("grip", bid)), Sense::click_and_drag());
        if gr.hovered() || app.ed.drag == Some(bid) {
            ui.painter().rect_filled(grip, 4.0, c.hover);
        }
        ui.painter().text(grip.center(), egui::Align2::CENTER_CENTER, "⋮⋮", FontId::proportional(if app.touch { 22.0 } else { 13.0 }), if app.touch { c.text2 } else { c.text3 });
        let gr = gr.on_hover_text("Drag to move\nClick to open menu").on_hover_cursor(egui::CursorIcon::Grab);
        if gr.drag_started() {
            app.ed.drag = Some(bid);
            app.ed.menu = None;
        } else if gr.clicked() {
            open_menu(app, ui.ctx(), MenuKind::Block(bid), grip.left_bottom() + vec2(0.0, 4.0));
        }
    }

    slash_out.map(|(id, _, f)| (id, content_rect, f))
}

fn table_ui(ui: &mut Ui, b: &mut Block, c: &Colors, w: f32) -> Rect {
    let bid = b.id;
    let cols = b.table.iter().map(|r| r.len()).max().unwrap_or(1).max(1);
    for r in &mut b.table {
        r.resize(cols, String::new());
    }
    let col_w = ((w - 30.0) / cols as f32).clamp(90.0, 260.0);
    let mut del_row: Option<usize> = None;
    let mut del_col: Option<usize> = None;
    let top = ui.cursor().min;
    let resp = ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        let nrows = b.table.len();
        for ri in 0..nrows {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                for ci in 0..cols {
                    let (cell, _) = ui.allocate_exact_size(vec2(col_w, 34.0), Sense::hover());
                    if ri == 0 {
                        ui.painter().rect_filled(cell, 0.0, c.callout);
                    }
                    ui.painter().rect_stroke(cell, 0.0, Stroke::new(1.0, c.line), egui::StrokeKind::Inside);
                    let fam = if ri == 0 { theme::bold() } else { FontFamily::Proportional };
                    let r = ui.put(
                        cell.shrink2(vec2(8.0, 7.0)),
                        egui::TextEdit::singleline(&mut b.table[ri][ci])
                            .id(Id::new(("cell", bid, ri, ci)))
                            .frame(egui::Frame::NONE)
                            .font(FontId::new(14.0, fam))
                            .text_color(c.text),
                    );
                    crate::platform::keyboard_zone(cell);
                    r.context_menu(|ui| {
                        if ui.button("Delete row").clicked() {
                            del_row = Some(ri);
                            ui.close();
                        }
                        if ui.button("Delete column").clicked() {
                            del_col = Some(ci);
                            ui.close();
                        }
                    });
                }
            });
        }
    });
    let rect = resp.response.rect;
    let hovered = ui.rect_contains_pointer(rect.expand2(vec2(30.0, 24.0)));
    // add column / row buttons
    let add_col = Rect::from_min_size(pos2(rect.right() + 2.0, top.y), vec2(20.0, rect.height()));
    let add_row = Rect::from_min_size(pos2(rect.left(), rect.bottom() + 2.0), vec2(rect.width(), 18.0));
    let (_, r2) = ui.allocate_exact_size(vec2(rect.width(), 20.0), Sense::hover());
    let _ = r2;
    if hovered {
        // Painted on top, without taking layout space, so nothing shifts on hover.
        let mut hover_btn = |r: Rect, salt: &str, tip: &str| {
            let resp = ui.interact(r, Id::new((salt, bid)), Sense::click());
            ui.painter().rect_filled(r, 3.0, if resp.hovered() { c.selected } else { c.callout });
            ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, "+", FontId::proportional(14.0), c.text3);
            resp.on_hover_text(tip).clicked()
        };
        if hover_btn(add_col, "tbl_add_col", "Add column") {
            for r in &mut b.table {
                r.push(String::new());
            }
        }
        if hover_btn(add_row, "tbl_add_row", "Add row") {
            b.table.push(vec![String::new(); cols]);
        }
    }
    if let Some(r) = del_row {
        if b.table.len() > 1 {
            b.table.remove(r);
        }
    }
    if let Some(ci) = del_col {
        if cols > 1 {
            for r in &mut b.table {
                r.remove(ci);
            }
        }
    }
    rect
}

fn apply(app: &mut App, ctx: &egui::Context, page: &mut crate::model::Page, act: Act, now: f64) {
    let n = page.blocks.len();
    match act {
        Act::Split { i, at } => {
            let b = &mut page.blocks[i];
            let byte = byte_of(&b.text, at);
            let tail = b.text[byte..].to_string();
            b.text.truncate(byte);
            let new_kind = match b.kind {
                Kind::Bullet | Kind::Numbered | Kind::Todo => b.kind,
                Kind::Toggle => Kind::Toggle,
                _ => Kind::Text,
            };
            let mut nb = Block::new(new_kind, tail);
            nb.indent = b.indent;
            // Pressing Enter at the very start keeps the block and pushes an empty one above.
            if at == 0 && !nb.text.is_empty() {
                let mut empty = Block::new(b.kind, "");
                empty.indent = b.indent;
                let moved = std::mem::replace(b, empty);
                let mut moved = moved;
                moved.text = nb.text.clone();
                let id = moved.id;
                page.blocks.insert(i + 1, moved);
                app.ed.focus = Some((id, 0));
            } else {
                let id = nb.id;
                page.blocks.insert(i + 1, nb);
                app.ed.focus = Some((id, 0));
            }
        }
        Act::MergeUp(i) => {
            if i == 0 {
                return;
            }
            let prev = &page.blocks[i - 1];
            if !prev.kind.is_text_like() || prev.kind == Kind::Code {
                if page.blocks[i].text.is_empty() {
                    // Remove the empty block and focus the previous one if possible.
                    page.blocks.remove(i);
                    if let Some(p) = page.blocks.get(i - 1) {
                        if p.kind == Kind::Code {
                            app.ed.focus = Some((p.id, p.text.chars().count()));
                        }
                    }
                } else {
                    // Delete the divider/table/link above.
                    page.blocks.remove(i - 1);
                }
                return;
            }
            let cur = page.blocks.remove(i);
            let prev = &mut page.blocks[i - 1];
            let pos = prev.text.chars().count();
            prev.text.push_str(&cur.text);
            app.ed.focus = Some((prev.id, pos));
        }
        Act::Delete(i) => {
            if i < n {
                page.blocks.remove(i);
            }
        }
        Act::InsertAfter(i, kind) => {
            let nb = Block::new(kind, "");
            let id = nb.id;
            page.blocks.insert((i + 1).min(n), nb);
            app.ed.focus = Some((id, 0));
        }
        Act::MoveUp(i) => {
            if i > 0 {
                page.blocks.swap(i, i - 1);
            }
        }
        Act::MoveDown(i) => {
            if i + 1 < n {
                page.blocks.swap(i, i + 1);
            }
        }
        Act::Duplicate(i) => {
            let mut nb = page.blocks[i].clone();
            nb.id = crate::model::next_block_id();
            page.blocks.insert(i + 1, nb);
        }
        Act::TurnInto(i, kind) => {
            let b = &mut page.blocks[i];
            if b.text.starts_with('/') && !b.text.contains(' ') {
                b.text.clear();
            }
            let had_text = b.text.clone();
            b.kind = kind;
            b.fix_kind_data();
            if kind == Kind::Divider {
                b.text.clear();
                let nb = Block::new(Kind::Text, had_text);
                let id = nb.id;
                page.blocks.insert(i + 1, nb);
                app.ed.focus = Some((id, 0));
            } else if kind == Kind::Image {
                b.text.clear();
                if i + 1 >= page.blocks.len() {
                    page.blocks.push(Block::new(Kind::Text, ""));
                }
                apply(app, ctx, page, Act::PickImage(i), now);
            } else if kind == Kind::Bookmark {
                if is_url(b.text.trim()) {
                    b.extra = b.text.trim().to_string();
                }
                b.text.clear();
                if i + 1 >= page.blocks.len() {
                    page.blocks.push(Block::new(Kind::Text, ""));
                }
            } else if kind == Kind::Table {
                b.text.clear();
                if i + 1 >= page.blocks.len() {
                    page.blocks.push(Block::new(Kind::Text, ""));
                }
            } else {
                app.ed.focus = Some((b.id, b.text.chars().count()));
            }
            app.ed.slash_sel = 0;
        }
        Act::FocusPrev(i) => {
            if let Some(p) = page.blocks[..i].iter().rev().find(|b| b.kind.is_text_like()) {
                app.ed.focus = Some((p.id, p.text.chars().count()));
            } else {
                app.ed.focus_title = true;
            }
        }
        Act::FocusNext(i) => {
            if let Some(p) = page.blocks.iter().skip(i + 1).find(|b| b.kind.is_text_like()) {
                app.ed.focus = Some((p.id, 0));
            }
        }
        Act::Indent(i, d) => {
            let b = &mut page.blocks[i];
            if b.kind.is_list() || d > 0 {
                if d > 0 && i > 0 && page.blocks[i - 1].kind.is_list() {
                    let max = page.blocks[i - 1].indent + 1;
                    let b = &mut page.blocks[i];
                    if b.kind.is_list() {
                        b.indent = (b.indent + 1).min(max).min(6);
                    }
                } else if d < 0 {
                    let b = &mut page.blocks[i];
                    b.indent = b.indent.saturating_sub(1);
                }
            }
            let b = &page.blocks[i];
            app.ed.focus = Some((b.id, cursor_of(ctx, Id::new(("blk", b.id))).map(|r| usize::from(r.primary.index)).unwrap_or(0)));
        }
        Act::OpenPage(id) => {
            app.open_page(&id);
        }
        Act::MoveTo { from, to } => move_blocks(&mut page.blocks, from, to),
        Act::PasteAs { block, url, start, choice } => {
            let Some(i) = page.blocks.iter().position(|b| b.id == block) else { return };
            let len = url.chars().count();
            let b = &mut page.blocks[i];
            let sb = byte_of(&b.text, start);
            let eb = byte_of(&b.text, start + len);
            if b.text.get(sb..eb) != Some(url.as_str()) {
                return;
            }
            match choice {
                0 => {
                    let m = format!("[@{}]({})", host_of(&url), url);
                    let mlen = m.chars().count();
                    b.text.replace_range(sb..eb, &m);
                    app.ed.focus = Some((b.id, start + mlen));
                }
                2 => {
                    let mut bm = Block::new(Kind::Bookmark, "");
                    bm.extra = url.clone();
                    b.text.replace_range(sb..eb, "");
                    if b.text.trim().is_empty() && b.kind == Kind::Text {
                        page.blocks[i] = bm;
                        if i + 1 >= page.blocks.len() {
                            page.blocks.push(Block::new(Kind::Text, ""));
                        }
                        let nid = page.blocks[i + 1].id;
                        app.ed.focus = Some((nid, 0));
                    } else {
                        page.blocks.insert(i + 1, bm);
                    }
                }
                _ => {}
            }
        }
        Act::PickImage(i) => {
            // The picture arrives through `platform::take_picked` (right away on desktop,
            // after the file picker closes on iPhone/iPad).
            if let Some(b) = page.blocks.get(i) {
                crate::platform::pick(crate::platform::PickKind::BlockImage { page: page.id.clone(), block: b.id });
            }
            let _ = now;
        }
        Act::Color(i, col) => {
            if let Some(b) = page.blocks.get_mut(i) {
                b.color = col;
            }
        }
        Act::NewSubpage(i, is_db) => {
            let parent = page.id.clone();
            let sub = app.ws.create("", &parent, now);
            if is_db {
                if let Some(e) = app.ws.get_mut(&sub) {
                    e.page.db = Some(crate::model::Db::new());
                    e.page.icon = "🗃".into();
                    e.page.blocks.clear();
                }
            }
            app.expanded.insert(parent);
            let b = &mut page.blocks[i];
            b.kind = Kind::PageLink;
            b.text = "Untitled".into();
            b.extra = sub.clone();
            app.ed.slash_sel = 0;
            // Save the parent first, then go to the new page.
            let pid = page.id.clone();
            if let Some(e) = app.ws.get_mut(&pid) {
                e.page = page.clone();
            }
            app.ws.touch(&pid, now - 10.0);
            app.ed.pending_open = Some(sub);
        }
    }
}


/// Move block `from` (with its nested list items) so it lands before position `to`
/// (positions counted before the move; `to == len` means the end).
pub fn move_blocks(blocks: &mut Vec<Block>, from: usize, to: usize) {
    if from >= blocks.len() {
        return;
    }
    let end = list_group_end(blocks, from);
    if to >= from && to <= end + 1 {
        return;
    }
    let group: Vec<Block> = blocks.drain(from..=end).collect();
    let len = group.len();
    let at = if to > end { to - len } else { to };
    let at = at.min(blocks.len());
    // Keep nesting sensible: a moved list item can't be deeper than the one above + 1.
    let max_indent = if at == 0 { 0 } else if blocks[at - 1].kind.is_list() { blocks[at - 1].indent + 1 } else { 0 };
    let shift = group[0].indent.saturating_sub(max_indent);
    for (k, mut b) in group.into_iter().enumerate() {
        b.indent = b.indent.saturating_sub(shift);
        blocks.insert(at + k, b);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(v: &[Block]) -> Vec<String> {
        v.iter().map(|b| format!("{}{}", " ".repeat(b.indent as usize), b.text)).collect()
    }

    #[test]
    fn drag_moves() {
        let mk = |t: &str, k: Kind, ind: u8| {
            let mut b = Block::new(k, t);
            b.indent = ind;
            b
        };
        let base = vec![
            mk("A", Kind::Text, 0),
            mk("B", Kind::Bullet, 0),
            mk("b1", Kind::Bullet, 1),
            mk("b2", Kind::Bullet, 1),
            mk("C", Kind::Text, 0),
            mk("D", Kind::Text, 0),
        ];
        // Move a list item with its children to the end.
        let mut v = base.clone();
        move_blocks(&mut v, 1, 6);
        assert_eq!(names(&v), ["A", "C", "D", "B", " b1", " b2"]);
        // Move the last block to the top.
        let mut v = base.clone();
        move_blocks(&mut v, 5, 0);
        assert_eq!(names(&v), ["D", "A", "B", " b1", " b2", "C"]);
        // Dropping inside its own group does nothing.
        let mut v = base.clone();
        move_blocks(&mut v, 1, 3);
        assert_eq!(names(&v), names(&base));
        // A nested item dragged under a plain paragraph loses its extra indent.
        let mut v = base.clone();
        move_blocks(&mut v, 3, 5);
        assert_eq!(names(&v), ["A", "B", " b1", "C", "b2", "D"]);
        // Moving down by one.
        let mut v = base.clone();
        move_blocks(&mut v, 4, 6);
        assert_eq!(names(&v), ["A", "B", " b1", " b2", "D", "C"]);
    }
}
