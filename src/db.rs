//! Databases: a page whose child pages are its rows. Each row is still its
//! own Markdown file, with column values in the file's header.

use crate::App;
use crate::model::{ColKind, Column, Db, Filter, Page, View};
use crate::editor::{MenuKind, close_menu, float_menu, open_menu};
use crate::theme::{self, Colors};
use eframe::egui::{self, Align, Color32, FontFamily, FontId, Id, Layout, Rect, RichText, Sense, Stroke, Ui, pos2, vec2};

pub fn today() -> String {
    let (y, m, d) = civil(crate::store::now_secs() as i64 / 86400);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Days since 1970-01-01 to (year, month, day).
pub fn civil(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

pub fn pretty_date(iso: &str) -> String {
    let parts: Vec<&str> = iso.split('-').collect();
    if parts.len() == 3 {
        if let (Ok(y), Ok(m), Ok(d)) = (parts[0].parse::<i32>(), parts[1].parse::<usize>(), parts[2].parse::<u32>()) {
            if (1..=12).contains(&m) {
                return format!("{} {}, {}", MONTHS[m - 1], d, y);
            }
        }
    }
    iso.to_string()
}

pub fn chip(ui: &mut Ui, text: &str, dark: bool, c: &Colors) -> egui::Response {
    let bg = theme::block_bg(theme::tag_color(text), dark);
    let g = ui.painter().layout_no_wrap(text.to_string(), FontId::proportional(13.0), c.text);
    let size = vec2(g.size().x + 12.0, 20.0);
    let (r, resp) = ui.allocate_exact_size(size, Sense::click());
    ui.painter().rect_filled(r, 3.0, bg);
    ui.painter().galley(pos2(r.left() + 6.0, r.center().y - g.size().y / 2.0), g, c.text);
    resp
}

fn checkbox(ui: &mut Ui, on: bool, c: &Colors) -> bool {
    let (r, resp) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
    let bx = Rect::from_center_size(r.center(), vec2(16.0, 16.0));
    if on {
        ui.painter().rect_filled(bx, 3.0, c.accent);
        ui.painter().text(bx.center(), egui::Align2::CENTER_CENTER, "✔", FontId::proportional(12.0), Color32::WHITE);
    } else {
        ui.painter().rect_stroke(bx, 3.0, Stroke::new(1.2, c.text3), egui::StrokeKind::Inside);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}

/// One editable value. Returns true when the value (or the column's options) changed.
pub fn cell_ui(ui: &mut Ui, id: Id, col: &mut Column, props: &mut Vec<(String, String)>, width: f32, dark: bool, c: &Colors) -> bool {
    let mut val = props.iter().find(|(k, _)| *k == col.name).map(|(_, v)| v.clone()).unwrap_or_default();
    let before = val.clone();
    let opts_before = col.options.len();
    match col.kind {
        ColKind::Checkbox => {
            if checkbox(ui, val == "true", c) {
                val = if val == "true" { String::new() } else { "true".into() };
            }
        }
        ColKind::Text | ColKind::Number | ColKind::Date => {
            let hint = match col.kind {
                ColKind::Date => "YYYY-MM-DD",
                ColKind::Number => "0",
                _ => "",
            };
            let focused = ui.memory(|m| m.has_focus(id));
            if col.kind == ColKind::Date && !focused && !val.is_empty() {
                let r = ui.add(egui::Label::new(RichText::new(pretty_date(&val)).size(14.0).color(c.text)).sense(Sense::click()));
                if r.clicked() {
                    ui.memory_mut(|m| m.request_focus(id));
                }
                ui.allocate_space(vec2((width - r.rect.width()).max(0.0), 1.0));
            } else {
                ui.horizontal(|ui| {
                    let tw = if col.kind == ColKind::Date && focused { width - 50.0 } else { width };
                    crate::platform::keyboard_zone(ui.max_rect());
                    ui.add(
                        egui::TextEdit::singleline(&mut val)
                            .id(id)
                            .frame(egui::Frame::NONE)
                            .hint_text(RichText::new(if ui.rect_contains_pointer(ui.max_rect()) || focused { hint } else { "" }).color(c.text3))
                            .font(FontId::proportional(14.0))
                            .text_color(c.text)
                            .desired_width(tw.max(30.0)),
                    );
                    if col.kind == ColKind::Date && (focused || val.is_empty()) && ui.rect_contains_pointer(ui.max_rect()) {
                        if ui.add(egui::Button::new(RichText::new("Today").size(11.0).color(c.text2)).frame(false)).clicked() {
                            val = today();
                        }
                    }
                });
            }
        }
        ColKind::Select | ColKind::Multi => {
            let multi = col.kind == ColKind::Multi;
            let mut chosen: Vec<String> = if multi {
                val.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
            } else if val.is_empty() {
                vec![]
            } else {
                vec![val.clone()]
            };
            let inner = ui.horizontal(|ui| {
                ui.set_min_width(width);
                ui.spacing_mut().item_spacing.x = 4.0;
                for ch in &chosen {
                    chip(ui, ch, dark, c);
                }
                ui.allocate_space(vec2(ui.available_width().max(4.0), 22.0));
            });
            let resp = ui.interact(inner.response.rect, id.with("pick"), Sense::click());
            egui::Popup::menu(&resp)
                .width(240.0)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| {
                    ui.label(RichText::new(if multi { "Select options" } else { "Select an option" }).size(11.0).color(c.text3));
                    let mut new_opt = ui.data_mut(|d| d.get_temp::<String>(id.with("new")).unwrap_or_default());
                    let r = ui.add(egui::TextEdit::singleline(&mut new_opt).hint_text("Search or create…").desired_width(220.0));
                    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if enter && !new_opt.trim().is_empty() {
                        let o = new_opt.trim().replace([',', '|'], " ");
                        if !col.options.contains(&o) {
                            col.options.push(o.clone());
                        }
                        if multi {
                            if !chosen.contains(&o) {
                                chosen.push(o);
                            }
                        } else {
                            chosen = vec![o];
                        }
                        new_opt.clear();
                    }
                    ui.separator();
                    let filter = new_opt.to_lowercase();
                    let mut all = col.options.clone();
                    for ch in &chosen {
                        if !all.contains(ch) {
                            all.push(ch.clone());
                        }
                    }
                    for o in all.iter().filter(|o| filter.is_empty() || o.to_lowercase().contains(&filter)) {
                        let on = chosen.contains(o);
                        ui.horizontal(|ui| {
                            let rr = chip(ui, o, dark, c);
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if on {
                                    ui.label(RichText::new("✔").color(c.accent));
                                }
                            });
                            if rr.clicked() {
                                if multi {
                                    if on {
                                        chosen.retain(|x| x != o);
                                    } else {
                                        chosen.push(o.clone());
                                    }
                                } else {
                                    chosen = if on { vec![] } else { vec![o.clone()] };
                                    ui.close();
                                }
                            }
                        });
                    }
                    if !chosen.is_empty() && ui.button(RichText::new("Clear").size(12.0).color(c.text2)).clicked() {
                        chosen.clear();
                    }
                    ui.data_mut(|d| d.insert_temp(id.with("new"), new_opt));
                });
            val = chosen.join(", ");
        }
    }
    let changed = val != before || col.options.len() != opts_before;
    if val != before {
        if let Some(p) = props.iter_mut().find(|(k, _)| *k == col.name) {
            p.1 = val;
        } else {
            props.push((col.name.clone(), val));
        }
    }
    changed
}

fn col_kind(db: &Db, col: &str) -> ColKind {
    db.columns.iter().find(|c| c.name == col).map(|c| c.kind).unwrap_or(ColKind::Text)
}

fn value_of<'a>(p: &'a Page, col: &str) -> &'a str {
    if col == "Name" { &p.title } else { p.prop(col) }
}

pub fn ops_for(kind: ColKind) -> &'static [&'static str] {
    match kind {
        ColKind::Text => &["contains", "does not contain", "is", "is not", "is empty", "is not empty"],
        ColKind::Number => &["=", ">", "<", "is empty", "is not empty"],
        ColKind::Checkbox => &["is checked", "is not checked"],
        ColKind::Date => &["is", "is before", "is after", "is empty", "is not empty"],
        ColKind::Select => &["is", "is not", "is empty", "is not empty"],
        ColKind::Multi => &["contains", "does not contain", "is empty", "is not empty"],
    }
}

fn needs_value(op: &str) -> bool {
    !matches!(op, "is empty" | "is not empty" | "is checked" | "is not checked")
}

pub fn matches(p: &Page, db: &Db, f: &Filter) -> bool {
    let kind = if f.col == "Name" { ColKind::Text } else { col_kind(db, &f.col) };
    let v = value_of(p, &f.col).trim();
    let want = if kind == ColKind::Date && f.value == "today" { today() } else { f.value.clone() };
    let vl = v.to_lowercase();
    let wl = want.to_lowercase();
    let list = || v.split(',').map(|x| x.trim().to_lowercase()).filter(|x| !x.is_empty()).collect::<Vec<_>>();
    match f.op.as_str() {
        "is empty" => v.is_empty(),
        "is not empty" => !v.is_empty(),
        "is checked" => v == "true",
        "is not checked" => v != "true",
        _ if wl.is_empty() => true,
        "contains" if kind == ColKind::Multi => list().contains(&wl),
        "does not contain" if kind == ColKind::Multi => !list().contains(&wl),
        "contains" => vl.contains(&wl),
        "does not contain" => !vl.contains(&wl),
        "is" | "=" if kind == ColKind::Number => v.parse::<f64>().ok() == want.parse::<f64>().ok(),
        "is" | "=" => vl == wl,
        "is not" => vl != wl,
        "is before" => !v.is_empty() && v < want.as_str(),
        "is after" => !v.is_empty() && v > want.as_str(),
        ">" => v.parse::<f64>().ok().zip(want.parse::<f64>().ok()).map(|(a, b)| a > b).unwrap_or(false),
        "<" => v.parse::<f64>().ok().zip(want.parse::<f64>().ok()).map(|(a, b)| a < b).unwrap_or(false),
        _ => true,
    }
}

fn cmp_values(db: &Db, col: &str, a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let kind = if col == "Name" { ColKind::Text } else { col_kind(db, col) };
    match kind {
        ColKind::Number => a.trim().parse::<f64>().unwrap_or(0.0).partial_cmp(&b.trim().parse::<f64>().unwrap_or(0.0)).unwrap_or(Ordering::Equal),
        ColKind::Checkbox => (a == "true").cmp(&(b == "true")),
        ColKind::Select => {
            let opts = db.columns.iter().find(|c| c.name == col).map(|c| c.options.clone()).unwrap_or_default();
            let ia = opts.iter().position(|o| o == a).unwrap_or(usize::MAX);
            let ib = opts.iter().position(|o| o == b).unwrap_or(usize::MAX);
            ia.cmp(&ib)
        }
        _ => a.to_lowercase().cmp(&b.to_lowercase()),
    }
}

/// Rows after the database's filters, sorts and the quick search.
fn rows_of(app: &App, db_id: &str, db: &Db) -> Vec<String> {
    let q = app.ed.db_search.trim().to_lowercase();
    let mut rows: Vec<&Page> = app
        .ws
        .children(db_id)
        .into_iter()
        .filter(|p| db.filters.iter().all(|f| matches(p, db, f)))
        .filter(|p| q.is_empty() || p.title.to_lowercase().contains(&q) || p.props.iter().any(|(_, v)| v.to_lowercase().contains(&q)))
        .collect();
    if !db.sorts.is_empty() {
        rows.sort_by(|a, b| {
            for (col, asc) in &db.sorts {
                let (va, vb) = (value_of(a, col), value_of(b, col));
                // Empty values always go last.
                let o = match (va.trim().is_empty(), vb.trim().is_empty()) {
                    (true, false) => std::cmp::Ordering::Greater,
                    (false, true) => std::cmp::Ordering::Less,
                    (true, true) => std::cmp::Ordering::Equal,
                    _ => {
                        let o = cmp_values(db, col, va, vb);
                        if *asc { o } else { o.reverse() }
                    }
                };
                if o != std::cmp::Ordering::Equal {
                    return o;
                }
            }
            std::cmp::Ordering::Equal
        });
    }
    rows.iter().map(|p| p.id.clone()).collect()
}

fn col_names(db: &Db) -> Vec<String> {
    let mut v = vec!["Name".to_string()];
    v.extend(db.columns.iter().map(|c| c.name.clone()));
    v
}

fn filter_label(f: &Filter) -> String {
    if needs_value(&f.op) {
        format!("{} {} {}", f.col, f.op, if f.value.is_empty() { "…" } else { &f.value })
    } else {
        format!("{} {}", f.col, f.op)
    }
}

fn filter_menu(app: &mut App, ctx: &egui::Context, db: &mut Db, c: &Colors) {
    let Some(m) = app.ed.menu.clone() else { return };
    if m.kind != MenuKind::DbFilter {
        return;
    }
    let dark = app.settings.dark;
    let (_, outside) = float_menu(ctx, Id::new("db_filter_menu"), &m, 470.0, c, |ui| {
        ui.label(RichText::new("Show only rows where…").size(12.0).color(c.text2));
        ui.add_space(4.0);
        let names = col_names(db);
        let mut remove = None;
        for (k, f) in db.filters.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.label(RichText::new(if k == 0 { "Where" } else { "And" }).size(12.0).color(c.text3));
                egui::ComboBox::from_id_salt(("fcol", k)).selected_text(f.col.clone()).width(110.0).show_ui(ui, |ui| {
                    for n in &names {
                        ui.selectable_value(&mut f.col, n.clone(), n);
                    }
                });
                let kind = if f.col == "Name" { ColKind::Text } else { db.columns.iter().find(|c| c.name == f.col).map(|c| c.kind).unwrap_or(ColKind::Text) };
                let ops = ops_for(kind);
                if !ops.contains(&f.op.as_str()) {
                    f.op = ops[0].to_string();
                }
                egui::ComboBox::from_id_salt(("fop", k)).selected_text(f.op.clone()).width(110.0).show_ui(ui, |ui| {
                    for o in ops {
                        ui.selectable_value(&mut f.op, o.to_string(), *o);
                    }
                });
                if needs_value(&f.op) {
                    let opts = db.columns.iter().find(|c| c.name == f.col).map(|c| c.options.clone()).unwrap_or_default();
                    if matches!(kind, ColKind::Select | ColKind::Multi) && !opts.is_empty() {
                        egui::ComboBox::from_id_salt(("fval", k)).selected_text(if f.value.is_empty() { "Pick…".to_string() } else { f.value.clone() }).width(110.0).show_ui(ui, |ui| {
                            for o in &opts {
                                ui.selectable_value(&mut f.value, o.clone(), o);
                            }
                        });
                    } else {
                        ui.add(egui::TextEdit::singleline(&mut f.value).desired_width(if kind == ColKind::Date { 90.0 } else { 120.0 }).hint_text(if kind == ColKind::Date { "YYYY-MM-DD" } else { "Value" }));
                        if kind == ColKind::Date && ui.small_button("Today").on_hover_text("Always compares with the current day").clicked() {
                            f.value = "today".into();
                        }
                    }
                }
                if ui.add(egui::Button::new(RichText::new("×").color(c.text3)).frame(false)).on_hover_text("Remove filter").clicked() {
                    remove = Some(k);
                }
            });
        }
        if let Some(k) = remove {
            db.filters.remove(k);
        }
        if db.filters.is_empty() {
            ui.label(RichText::new("No filters yet.").size(12.0).color(c.text3));
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui.add(egui::Button::new(RichText::new("+ Add filter").color(c.text2)).frame(false)).clicked() {
                let col = db.columns.iter().find(|c| c.kind == ColKind::Select).or(db.columns.first()).map(|c| c.name.clone()).unwrap_or_else(|| "Name".into());
                let kind = col_kind(db, &col);
                db.filters.push(Filter { col, op: ops_for(kind)[0].to_string(), value: String::new() });
            }
            if !db.filters.is_empty() && ui.add(egui::Button::new(RichText::new("Clear all").color(c.text3)).frame(false)).clicked() {
                db.filters.clear();
            }
        });
        let _ = dark;
    });
    if outside {
        close_menu(app, ctx);
    }
}

fn sort_menu(app: &mut App, ctx: &egui::Context, db: &mut Db, c: &Colors) {
    let Some(m) = app.ed.menu.clone() else { return };
    if m.kind != MenuKind::DbSort {
        return;
    }
    let (_, outside) = float_menu(ctx, Id::new("db_sort_menu"), &m, 330.0, c, |ui| {
        ui.label(RichText::new("Sort rows by…").size(12.0).color(c.text2));
        ui.add_space(4.0);
        let names = col_names(db);
        let mut remove = None;
        let n = db.sorts.len();
        for k in 0..n {
            ui.horizontal(|ui| {
                ui.label(RichText::new(if k == 0 { "Sort by" } else { "then" }).size(12.0).color(c.text3));
                let (col, asc) = &mut db.sorts[k];
                egui::ComboBox::from_id_salt(("scol", k)).selected_text(col.clone()).width(120.0).show_ui(ui, |ui| {
                    for nm in &names {
                        ui.selectable_value(col, nm.clone(), nm);
                    }
                });
                egui::ComboBox::from_id_salt(("sdir", k)).selected_text(if *asc { "Ascending" } else { "Descending" }).width(100.0).show_ui(ui, |ui| {
                    ui.selectable_value(asc, true, "Ascending");
                    ui.selectable_value(asc, false, "Descending");
                });
                if ui.add(egui::Button::new(RichText::new("×").color(c.text3)).frame(false)).clicked() {
                    remove = Some(k);
                }
            });
        }
        if let Some(k) = remove {
            db.sorts.remove(k);
        }
        if db.sorts.is_empty() {
            ui.label(RichText::new("Rows keep their own order.").size(12.0).color(c.text3));
        }
        if ui.add(egui::Button::new(RichText::new("+ Add sort").color(c.text2)).frame(false)).clicked() {
            let used: Vec<String> = db.sorts.iter().map(|s| s.0.clone()).collect();
            let col = names.iter().find(|n| !used.contains(n)).cloned().unwrap_or_else(|| "Name".into());
            db.sorts.push((col, true));
        }
    });
    if outside {
        close_menu(app, ctx);
    }
}

fn new_row(app: &mut App, db_id: &str, now: f64, prop: Option<(String, String)>) -> String {
    let id = app.ws.create("", db_id, now);
    if let Some(e) = app.ws.get_mut(&id) {
        e.page.blocks = vec![crate::model::Block::new(crate::model::Kind::Text, "")];
        if let Some((k, v)) = prop {
            e.page.set_prop(&k, v);
        }
    }
    id
}

/// Full-page database view, drawn under the page title.
pub fn db_view(app: &mut App, ui: &mut Ui, page: &mut Page, c: &Colors, now: f64) {
    let dark = app.settings.dark;
    let Some(db) = page.db.as_mut() else { return };
    let db_id = page.id.clone();

    // View tabs + New
    // Two-line toolbar on phones and in narrow page areas (iPad portrait).
    let narrow = app.narrow || ui.available_width() < 760.0;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for (v, label) in [(View::Table, "▦  Table"), (View::Board, "▥  Board")] {
            let sel = db.view == v;
            let b = egui::Button::new(RichText::new(label).size(14.0).color(if sel { c.text } else { c.text2 }).family(if sel { theme::bold() } else { FontFamily::Proportional }))
                .fill(if sel { c.hover } else { Color32::TRANSPARENT })
                .corner_radius(14);
            if ui.add(b).clicked() {
                db.view = v;
            }
        }
        if !narrow {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let nb = egui::Button::new(RichText::new("New").color(Color32::WHITE).family(theme::bold()).size(14.0)).fill(c.accent).corner_radius(5);
                if ui.add(nb).clicked() {
                    let id = new_row(app, &db_id, now, None);
                    app.ed.pending_open = Some(id);
                }
                ui.add_space(6.0);
                let sort_on = !db.sorts.is_empty();
                let sr = ui.add(egui::Button::new(RichText::new(if sort_on { format!("⇅ Sort ({})", db.sorts.len()) } else { "⇅ Sort".into() }).size(13.0).color(if sort_on { c.accent } else { c.text2 })).frame(false));
                if sr.clicked() {
                    open_menu(app, ui.ctx(), MenuKind::DbSort, sr.rect.left_bottom() + vec2(-200.0, 4.0));
                }
                let filt_on = !db.filters.is_empty();
                let fr = ui.add(egui::Button::new(RichText::new(if filt_on { format!("⚲ Filter ({})", db.filters.len()) } else { "⚲ Filter".into() }).size(13.0).color(if filt_on { c.accent } else { c.text2 })).frame(false));
                if fr.clicked() {
                    open_menu(app, ui.ctx(), MenuKind::DbFilter, fr.rect.left_bottom() + vec2(-300.0, 4.0));
                }
                let sr = ui.add(
                    egui::TextEdit::singleline(&mut app.ed.db_search)
                        .hint_text(RichText::new("🔍 Search").color(c.text3))
                        .desired_width(if narrow { 90.0 } else { 130.0 })
                        .frame(egui::Frame::NONE),
                );
                crate::platform::keyboard_zone(sr.rect);
                let total = app.ws.children(&db_id).len();
                let n = rows_of(app, &db_id, db).len();
                if !narrow || n != total {
                ui.label(RichText::new(if n == total { format!("{n} rows") } else { format!("{n} of {total}") }).size(12.0).color(c.text3));
            }
            });
        }
    });
    if narrow {
        // Phone: search, filter, sort and New get their own line.
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let nb = egui::Button::new(RichText::new("New").color(Color32::WHITE).family(theme::bold()).size(14.0)).fill(c.accent).corner_radius(5);
                if ui.add(nb).clicked() {
                    let id = new_row(app, &db_id, now, None);
                    app.ed.pending_open = Some(id);
                }
                ui.add_space(6.0);
                let sort_on = !db.sorts.is_empty();
                let sr = ui.add(egui::Button::new(RichText::new(if sort_on { format!("⇅ Sort ({})", db.sorts.len()) } else { "⇅ Sort".into() }).size(13.0).color(if sort_on { c.accent } else { c.text2 })).frame(false));
                if sr.clicked() {
                    open_menu(app, ui.ctx(), MenuKind::DbSort, sr.rect.left_bottom() + vec2(-200.0, 4.0));
                }
                let filt_on = !db.filters.is_empty();
                let fr = ui.add(egui::Button::new(RichText::new(if filt_on { format!("⚲ Filter ({})", db.filters.len()) } else { "⚲ Filter".into() }).size(13.0).color(if filt_on { c.accent } else { c.text2 })).frame(false));
                if fr.clicked() {
                    open_menu(app, ui.ctx(), MenuKind::DbFilter, fr.rect.left_bottom() + vec2(-300.0, 4.0));
                }
                let sr = ui.add(
                    egui::TextEdit::singleline(&mut app.ed.db_search)
                        .hint_text(RichText::new("🔍 Search").color(c.text3))
                        .desired_width(if narrow { 90.0 } else { 130.0 })
                        .frame(egui::Frame::NONE),
                );
                crate::platform::keyboard_zone(sr.rect);
                let total = app.ws.children(&db_id).len();
                let n = rows_of(app, &db_id, db).len();
                if !narrow || n != total {
                ui.label(RichText::new(if n == total { format!("{n} rows") } else { format!("{n} of {total}") }).size(12.0).color(c.text3));
            }
            });
        });
    }
    // Active filters and sorts as chips.
    if !db.filters.is_empty() || !db.sorts.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
            let mut remove_f = None;
            let mut remove_s = None;
            for (k, f) in db.filters.iter().enumerate() {
                let r = ui.add(egui::Button::new(RichText::new(format!("{}  ×", filter_label(f))).size(12.0).color(c.accent)).fill(c.accent.gamma_multiply(0.12)).corner_radius(10));
                if r.clicked() {
                    remove_f = Some(k);
                }
            }
            for (k, (col, asc)) in db.sorts.iter().enumerate() {
                let r = ui.add(egui::Button::new(RichText::new(format!("{} {}  ×", if *asc { "↑" } else { "↓" }, col)).size(12.0).color(c.text2)).fill(c.hover).corner_radius(10));
                if r.clicked() {
                    remove_s = Some(k);
                }
            }
            if let Some(k) = remove_f {
                db.filters.remove(k);
            }
            if let Some(k) = remove_s {
                db.sorts.remove(k);
            }
        });
    }
    ui.add_space(4.0);
    ui.painter().hline(ui.max_rect().x_range(), ui.cursor().top(), Stroke::new(1.0, c.line));
    ui.add_space(6.0);
    let ctx = ui.ctx().clone();
    filter_menu(app, &ctx, db, c);
    sort_menu(app, &ctx, db, c);

    match db.view {
        View::Table => table(app, ui, &db_id, db, c, dark, now),
        View::Board => board(app, ui, &db_id, db, c, dark, now),
    }
}

fn col_header_menu(ui: &mut Ui, db: &mut Db, ci: usize, renames: &mut Vec<(String, String)>, c: &Colors) -> bool {
    let mut delete = false;
    let old = db.columns[ci].name.clone();
    ui.label(RichText::new("Property name").size(11.0).color(c.text3));
    let mut name = old.clone();
    ui.add(egui::TextEdit::singleline(&mut name).desired_width(200.0));
    let name = name.replace(['|', ':', '\n'], " ");
    if name != old && !name.trim().is_empty() && !db.columns.iter().any(|x| x.name == name) {
        db.columns[ci].name = name.clone();
        if db.group == old {
            db.group = name.clone();
        }
        renames.push((old, name));
    }
    ui.separator();
    ui.label(RichText::new("Type").size(11.0).color(c.text3));
    for k in ColKind::ALL {
        if ui.selectable_label(db.columns[ci].kind == k, format!("{}  {}", k.icon(), k.label())).clicked() {
            db.columns[ci].kind = k;
        }
    }
    ui.separator();
    if ui.button("↑  Sort ascending").clicked() {
        db.sorts = vec![(db.columns[ci].name.clone(), true)];
        ui.close();
    }
    if ui.button("↓  Sort descending").clicked() {
        db.sorts = vec![(db.columns[ci].name.clone(), false)];
        ui.close();
    }
    if ui.button("⚲  Filter by this").clicked() {
        let col = db.columns[ci].name.clone();
        let op = ops_for(db.columns[ci].kind)[0].to_string();
        db.filters.push(Filter { col, op, value: String::new() });
        ui.close();
    }
    if db.columns[ci].kind == ColKind::Select && ui.button("▥  Group board by this").clicked() {
        db.group = db.columns[ci].name.clone();
        db.view = View::Board;
        ui.close();
    }
    if ci > 0 && ui.button("⬅  Move left").clicked() {
        db.columns.swap(ci, ci - 1);
        ui.close();
    }
    if ci + 1 < db.columns.len() && ui.button("➡  Move right").clicked() {
        db.columns.swap(ci, ci + 1);
        ui.close();
    }
    if ui.button(RichText::new("🗑  Delete property").color(Color32::from_rgb(220, 70, 70))).clicked() {
        delete = true;
        ui.close();
    }
    delete
}

fn col_width(k: ColKind) -> f32 {
    match k {
        ColKind::Checkbox => 90.0,
        ColKind::Date => 150.0,
        ColKind::Number => 100.0,
        _ => 180.0,
    }
}

fn table(app: &mut App, ui: &mut Ui, db_id: &str, db: &mut Db, c: &Colors, dark: bool, now: f64) {
    let rows = rows_of(app, db_id, db);
    let others: f32 = db.columns.iter().map(|c| col_width(c.kind)).sum::<f32>() + 40.0;
    let name_w = (ui.available_width() - others).clamp(if app.narrow { 170.0 } else { 220.0 }, 420.0);
    let row_h = 36.0;
    let mut renames: Vec<(String, String)> = Vec::new();
    let mut delete_col: Option<usize> = None;
    let mut delete_row: Option<String> = None;
    let mut open_row: Option<String> = None;

    egui::ScrollArea::horizontal().id_salt(("dbh", db_id)).show(ui, |ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        let total_w: f32 = name_w + db.columns.iter().map(|c| col_width(c.kind)).sum::<f32>() + 40.0;
        // Header
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            let (r, _) = ui.allocate_exact_size(vec2(name_w, 34.0), Sense::hover());
            ui.painter().text(pos2(r.left() + 8.0, r.center().y), egui::Align2::LEFT_CENTER, "Aa  Name", FontId::proportional(14.0), c.text2);
            ui.painter().vline(r.right(), r.y_range(), Stroke::new(1.0, c.line));
            for ci in 0..db.columns.len() {
                let w = col_width(db.columns[ci].kind);
                let (r, resp) = ui.allocate_exact_size(vec2(w, 34.0), Sense::click());
                if resp.hovered() {
                    ui.painter().rect_filled(r, 0.0, c.hover);
                }
                let label = format!("{}  {}", db.columns[ci].kind.icon(), db.columns[ci].name);
                ui.painter().with_clip_rect(r.shrink(2.0)).text(pos2(r.left() + 8.0, r.center().y), egui::Align2::LEFT_CENTER, label, FontId::proportional(14.0), c.text2);
                ui.painter().vline(r.right(), r.y_range(), Stroke::new(1.0, c.line));
                egui::Popup::menu(&resp).width(220.0).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                    if col_header_menu(ui, db, ci, &mut renames, c) {
                        delete_col = Some(ci);
                    }
                });
            }
            let (r, resp) = ui.allocate_exact_size(vec2(40.0, 34.0), Sense::click());
            if resp.hovered() {
                ui.painter().rect_filled(r, 0.0, c.hover);
            }
            ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, "+", FontId::proportional(16.0), c.text3);
            if resp.on_hover_text("Add a property").clicked() {
                let mut n = 1;
                let name = loop {
                    let nm = if n == 1 { "Property".to_string() } else { format!("Property {n}") };
                    if !db.columns.iter().any(|x| x.name == nm) {
                        break nm;
                    }
                    n += 1;
                };
                db.columns.push(Column { name, kind: ColKind::Text, options: vec![] });
            }
        });
        let y = ui.cursor().top();
        ui.painter().hline(ui.cursor().left()..=ui.cursor().left() + total_w, y, Stroke::new(1.0, c.line));

        // Rows
        for rid in &rows {
            let Some(e) = app.ws.get(rid) else { continue };
            let mut title = e.page.title.clone();
            let mut props = e.page.props.clone();
            let icon = if e.page.icon.is_empty() { "📄".to_string() } else { e.page.icon.clone() };
            let mut changed = false;
            let row_resp = ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                ui.set_min_height(row_h);
                // Name cell
                let (r, _) = ui.allocate_exact_size(vec2(name_w, row_h), Sense::hover());
                ui.painter().vline(r.right(), r.y_range(), Stroke::new(1.0, c.line));
                let mut cu = ui.new_child(egui::UiBuilder::new().max_rect(r.shrink2(vec2(8.0, 0.0))).layout(Layout::left_to_right(Align::Center)));
                cu.label(RichText::new(&icon).size(14.0));
                cu.add_space(6.0);
                let hovered = cu.rect_contains_pointer(r);
                let tw = if hovered { r.width() - 90.0 } else { r.width() - 40.0 };
                let te = cu.add(
                    egui::TextEdit::singleline(&mut title)
                        .id(Id::new(("dbname", rid)))
                        .frame(egui::Frame::NONE)
                        .hint_text(RichText::new("Untitled").color(c.text3))
                        .font(FontId::new(14.0, theme::bold()))
                        .text_color(c.text)
                        .desired_width(tw),
                );
                changed |= te.changed();
                crate::platform::keyboard_zone(te.rect);
                if hovered {
                    let ob = cu.add(egui::Button::new(RichText::new("↗ Open").size(11.0).color(c.text2)).fill(c.bg).stroke(Stroke::new(1.0, c.line)).corner_radius(4));
                    if ob.clicked() {
                        open_row = Some(rid.clone());
                    }
                }
                for ci in 0..db.columns.len() {
                    let w = col_width(db.columns[ci].kind);
                    let (r, _) = ui.allocate_exact_size(vec2(w, row_h), Sense::hover());
                    ui.painter().vline(r.right(), r.y_range(), Stroke::new(1.0, c.line));
                    let mut cu = ui.new_child(egui::UiBuilder::new().max_rect(r.shrink2(vec2(8.0, 0.0))).layout(Layout::left_to_right(Align::Center)));
                    let id = Id::new(("dbcell", rid, ci));
                    changed |= cell_ui(&mut cu, id, &mut db.columns[ci], &mut props, w - 16.0, dark, c);
                }
                ui.allocate_exact_size(vec2(40.0, row_h), Sense::hover());
            });
            let rr = row_resp.response.rect;
            ui.painter().hline(rr.left()..=rr.left() + total_w, rr.bottom(), Stroke::new(1.0, c.line));
            row_resp.response.interact(Sense::click()).context_menu(|ui| {
                if ui.button("↗  Open").clicked() {
                    open_row = Some(rid.clone());
                    ui.close();
                }
                if ui.button(RichText::new("🗑  Delete").color(Color32::from_rgb(220, 70, 70))).clicked() {
                    delete_row = Some(rid.clone());
                    ui.close();
                }
            });
            if changed {
                if let Some(e) = app.ws.get_mut(rid) {
                    e.page.title = title.replace('\n', " ");
                    e.page.props = props;
                }
                app.ws.touch(rid, now);
            }
        }
        // + New
        let (r, resp) = ui.allocate_exact_size(vec2(total_w, 34.0), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(r, 0.0, c.hover);
        }
        ui.painter().text(pos2(r.left() + 8.0, r.center().y), egui::Align2::LEFT_CENTER, "+  New", FontId::proportional(14.0), c.text3);
        if resp.clicked() {
            let id = new_row(app, db_id, now, None);
            app.ed.focus_db_row = Some(id);
        }
    });

    apply_db_edits(app, db_id, db, renames, delete_col, now);
    if let Some(r) = delete_row {
        app.ws.set_trashed(&r, true, now);
    }
    if let Some(r) = open_row {
        app.ed.pending_open = Some(r);
    }
    if let Some(r) = app.ed.focus_db_row.take() {
        ui.memory_mut(|m| m.request_focus(Id::new(("dbname", &r))));
    }
}

fn apply_db_edits(app: &mut App, db_id: &str, db: &mut Db, renames: Vec<(String, String)>, delete_col: Option<usize>, now: f64) {
    let rows: Vec<String> = app.ws.children(db_id).iter().map(|p| p.id.clone()).collect();
    for (old, new) in &renames {
        for f in &mut db.filters {
            if f.col == *old {
                f.col = new.clone();
            }
        }
        for s in &mut db.sorts {
            if s.0 == *old {
                s.0 = new.clone();
            }
        }
    }
    if let Some(ci) = delete_col {
        if let Some(name) = db.columns.get(ci).map(|c| c.name.clone()) {
            db.filters.retain(|f| f.col != name);
            db.sorts.retain(|s| s.0 != name);
        }
    }
    for (old, new) in renames {
        for rid in &rows {
            if let Some(e) = app.ws.get_mut(rid) {
                for p in &mut e.page.props {
                    if p.0 == old {
                        p.0 = new.clone();
                    }
                }
            }
            app.ws.touch(rid, now - 10.0);
        }
    }
    if let Some(ci) = delete_col {
        if ci < db.columns.len() {
            let name = db.columns.remove(ci).name;
            for rid in &rows {
                if let Some(e) = app.ws.get_mut(rid) {
                    e.page.props.retain(|p| p.0 != name);
                }
                app.ws.touch(rid, now - 10.0);
            }
        }
    }
}

fn board(app: &mut App, ui: &mut Ui, db_id: &str, db: &mut Db, c: &Colors, dark: bool, now: f64) {
    let Some(gcol) = db.group_col().cloned() else {
        ui.add_space(20.0);
        ui.label(RichText::new("A board groups rows by a Select property.").color(c.text2));
        if ui.button("➕  Add a Status property").clicked() {
            db.columns.push(Column { name: "Status".into(), kind: ColKind::Select, options: vec!["Not started".into(), "In progress".into(), "Done".into()] });
            db.group = "Status".into();
        }
        return;
    };
    db.group = gcol.name.clone();
    let rows = rows_of(app, db_id, db);
    let mut groups: Vec<String> = gcol.options.clone();
    for rid in &rows {
        if let Some(e) = app.ws.get(rid) {
            let v = e.page.prop(&gcol.name).to_string();
            if !v.is_empty() && !groups.contains(&v) {
                groups.push(v);
            }
        }
    }
    groups.insert(0, String::new());
    let mut moved: Option<(String, String)> = None;
    let mut open_row: Option<String> = None;
    let mut add_in: Option<String> = None;
    let col_w = 260.0;

    egui::ScrollArea::horizontal().id_salt(("dbb", db_id)).show(ui, |ui| {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing = vec2(12.0, 6.0);
            for g in &groups {
                let items: Vec<String> = rows.iter().filter(|r| app.ws.get(r).map(|e| e.page.prop(&gcol.name) == g.as_str()).unwrap_or(false)).cloned().collect();
                let frame = egui::Frame::new().inner_margin(egui::Margin::same(6)).corner_radius(6).fill(if g.is_empty() { Color32::TRANSPARENT } else { theme::block_bg(theme::tag_color(g), dark).gamma_multiply(0.5) });
                let (_, dropped) = ui.dnd_drop_zone::<String, ()>(frame, |ui| {
                    ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                    ui.set_width(col_w);
                    ui.set_min_height(120.0);
                    ui.horizontal(|ui| {
                        if g.is_empty() {
                            ui.label(RichText::new(format!("No {}", gcol.name)).size(13.0).color(c.text2));
                        } else {
                            chip(ui, g, dark, c);
                        }
                        ui.label(RichText::new(items.len().to_string()).size(13.0).color(c.text3));
                    });
                    ui.add_space(4.0);
                    for rid in &items {
                        let Some(e) = app.ws.get(rid) else { continue };
                        let title = e.page.display_title().to_string();
                        let icon = e.page.icon.clone();
                        let extra: Vec<(ColKind, String, String)> = db
                            .columns
                            .iter()
                            .filter(|col| col.name != gcol.name)
                            .map(|col| (col.kind, e.page.prop(&col.name).to_string(), col.name.clone()))
                            .filter(|(_, v, _)| !v.is_empty())
                            .collect();
                        let drag = ui.dnd_drag_source(Id::new(("card", rid)), rid.clone(), |ui| {
                            egui::Frame::new()
                                .fill(c.popup)
                                .corner_radius(6)
                                .stroke(Stroke::new(1.0, c.line))
                                .shadow(egui::epaint::Shadow { offset: [0, 1], blur: 3, spread: 0, color: Color32::from_black_alpha(18) })
                                .inner_margin(egui::Margin::symmetric(10, 8))
                                .show(ui, |ui| {
                                    ui.set_width(col_w - 20.0);
                                    ui.label(RichText::new(format!("{}{}", if icon.is_empty() { String::new() } else { format!("{icon} ") }, title)).size(14.0).family(theme::bold()).color(c.text));
                                    for (k, v, name) in &extra {
                                        match k {
                                            ColKind::Checkbox => {
                                                ui.label(RichText::new(format!("☑ {name}")).size(12.0).color(c.text2));
                                            }
                                            ColKind::Date => {
                                                ui.label(RichText::new(pretty_date(v)).size(12.0).color(c.text2));
                                            }
                                            ColKind::Select | ColKind::Multi => {
                                                ui.horizontal_wrapped(|ui| {
                                                    ui.spacing_mut().item_spacing.x = 4.0;
                                                    for t in v.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                                                        chip(ui, t, dark, c);
                                                    }
                                                });
                                            }
                                            _ => {
                                                ui.label(RichText::new(v).size(12.0).color(c.text2));
                                            }
                                        }
                                    }
                                });
                        });
                        let r = drag.response;
                        if r.clicked() {
                            open_row = Some(rid.clone());
                        }
                        r.context_menu(|ui| {
                            ui.label(RichText::new("Move to").size(11.0).color(c.text3));
                            for g2 in &groups {
                                if g2 != g && ui.button(if g2.is_empty() { format!("No {}", gcol.name) } else { g2.clone() }).clicked() {
                                    moved = Some((rid.clone(), g2.clone()));
                                    ui.close();
                                }
                            }
                        });
                    }
                    if ui.add(egui::Button::new(RichText::new("+  New").size(13.0).color(c.text3)).frame(false)).clicked() {
                        add_in = Some(g.clone());
                    }
                    });
                });
                if let Some(p) = dropped {
                    moved = Some(((*p).clone(), g.clone()));
                }
            }
        });
    });

    if let Some((rid, g)) = moved {
        if let Some(e) = app.ws.get_mut(&rid) {
            e.page.set_prop(&gcol.name, g);
        }
        app.ws.touch(&rid, now - 10.0);
    }
    if let Some(g) = add_in {
        let id = new_row(app, db_id, now, if g.is_empty() { None } else { Some((gcol.name.clone(), g)) });
        app.ed.pending_open = Some(id);
    }
    if let Some(r) = open_row {
        app.ed.pending_open = Some(r);
    }
}

/// Property list shown at the top of a row page.
pub fn row_props(app: &mut App, ui: &mut Ui, page: &mut Page, c: &Colors, now: f64) {
    let Some(mut db) = app.ws.get(&page.parent).and_then(|e| e.page.db.clone()) else { return };
    let dark = app.settings.dark;
    let mut db_changed = false;
    ui.add_space(4.0);
    for ci in 0..db.columns.len() {
        ui.horizontal(|ui| {
            ui.set_min_height(32.0);
            let (r, _) = ui.allocate_exact_size(vec2(160.0, 30.0), Sense::hover());
            let label = format!("{}  {}", db.columns[ci].kind.icon(), db.columns[ci].name);
            ui.painter().text(pos2(r.left(), r.center().y), egui::Align2::LEFT_CENTER, label, FontId::proportional(14.0), c.text2);
            let before = db.columns[ci].options.len();
            let w = ui.available_width() - 10.0;
            cell_ui(ui, Id::new(("rowprop", &page.id, ci)), &mut db.columns[ci], &mut page.props, w, dark, c);
            if db.columns[ci].options.len() != before {
                db_changed = true;
            }
        });
    }
    ui.add_space(6.0);
    ui.painter().hline(ui.max_rect().x_range(), ui.cursor().top(), Stroke::new(1.0, c.line));
    ui.add_space(8.0);
    if db_changed {
        let pid = page.parent.clone();
        if let Some(e) = app.ws.get_mut(&pid) {
            e.page.db = Some(db);
        }
        app.ws.touch(&pid, now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters() {
        let db = Db {
            columns: vec![
                Column { name: "Done".into(), kind: ColKind::Checkbox, options: vec![] },
                Column { name: "Due".into(), kind: ColKind::Date, options: vec![] },
                Column { name: "Tags".into(), kind: ColKind::Multi, options: vec![] },
            ],
            ..Db::empty()
        };
        let mut p = Page::new("x".into(), "Buy milk", "");
        p.set_prop("Due", "2026-01-05".into());
        p.set_prop("Tags", "Home, Errand".into());
        let f = |col: &str, op: &str, v: &str| Filter { col: col.into(), op: op.into(), value: v.into() };
        assert!(matches(&p, &db, &f("Name", "contains", "milk")));
        assert!(!matches(&p, &db, &f("Name", "is", "milk")));
        assert!(matches(&p, &db, &f("Done", "is not checked", "")));
        assert!(matches(&p, &db, &f("Due", "is before", "2026-02-01")));
        assert!(!matches(&p, &db, &f("Due", "is after", "2026-02-01")));
        assert!(matches(&p, &db, &f("Tags", "contains", "errand")));
        assert!(!matches(&p, &db, &f("Tags", "contains", "Err")));
        assert!(matches(&p, &db, &f("Due", "is not empty", "")));
    }
}
