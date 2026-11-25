//! Colours and fonts.

use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily};
use std::sync::Arc;

#[derive(Clone, Copy)]
pub struct Colors {
    pub bg: Color32,
    pub sidebar: Color32,
    pub text: Color32,
    pub text2: Color32,
    pub text3: Color32,
    pub hover: Color32,
    pub selected: Color32,
    pub line: Color32,
    pub accent: Color32,
    pub callout: Color32,
    pub code: Color32,
    pub inline_code: Color32,
    pub inline_code_fg: Color32,
    pub popup: Color32,
}

pub fn colors(dark: bool) -> Colors {
    if dark {
        Colors {
            bg: Color32::from_rgb(25, 25, 25),
            sidebar: Color32::from_rgb(32, 32, 32),
            text: Color32::from_rgb(212, 212, 212),
            text2: Color32::from_rgb(155, 155, 155),
            text3: Color32::from_rgb(110, 110, 110),
            hover: Color32::from_rgb(44, 44, 44),
            selected: Color32::from_rgb(50, 50, 50),
            line: Color32::from_rgb(47, 47, 47),
            accent: Color32::from_rgb(35, 131, 226),
            callout: Color32::from_rgb(37, 37, 37),
            code: Color32::from_rgb(32, 32, 32),
            inline_code: Color32::from_rgb(52, 52, 52),
            inline_code_fg: Color32::from_rgb(235, 87, 87),
            popup: Color32::from_rgb(37, 37, 37),
        }
    } else {
        Colors {
            bg: Color32::WHITE,
            sidebar: Color32::from_rgb(247, 247, 245),
            text: Color32::from_rgb(55, 53, 47),
            text2: Color32::from_rgb(120, 119, 116),
            text3: Color32::from_rgb(170, 169, 166),
            hover: Color32::from_rgb(236, 236, 234),
            selected: Color32::from_rgb(231, 231, 229),
            line: Color32::from_rgb(233, 233, 231),
            accent: Color32::from_rgb(35, 131, 226),
            callout: Color32::from_rgb(241, 241, 239),
            code: Color32::from_rgb(247, 246, 243),
            inline_code: Color32::from_rgb(238, 237, 233),
            inline_code_fg: Color32::from_rgb(235, 87, 87),
            popup: Color32::WHITE,
        }
    }
}

pub fn bold() -> FontFamily {
    FontFamily::Name("bold".into())
}

fn try_load(paths: &[&str]) -> Option<Vec<u8>> {
    if cfg!(target_arch = "wasm32") {
        return None; // the browser can't lend its fonts; the bundled ones are used
    }
    paths.iter().find_map(|p| std::fs::read(p).ok())
}

/// Roboto (Apache License 2.0, see assets/fonts/LICENSE-Roboto.txt), used on the
/// web and when the computer doesn't have the usual system fonts.
const BUNDLED_REGULAR: &[u8] = include_bytes!("../assets/fonts/Roboto-Regular.ttf");
const BUNDLED_BOLD: &[u8] = include_bytes!("../assets/fonts/Roboto-Bold.ttf");

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    let regular = try_load(&[
        "/System/Library/Fonts/Supplemental/Arial.ttf",
        "C:\\Windows\\Fonts\\segoeui.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    ])
    .or_else(|| Some(BUNDLED_REGULAR.to_vec()));
    let boldf = try_load(&[
        "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
        "C:\\Windows\\Fonts\\segoeuib.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
    ])
    .or_else(|| Some(BUNDLED_BOLD.to_vec()));
    let mono = try_load(&[
        "/System/Library/Fonts/Menlo.ttc",
        "C:\\Windows\\Fonts\\consola.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
    ]);

    let base_prop = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let mut prop = base_prop.clone();
    if let Some(r) = regular {
        fonts.font_data.insert("ui-regular".into(), Arc::new(FontData::from_owned(r)));
        prop.insert(0, "ui-regular".into());
    }
    let mut boldv = prop.clone();
    if let Some(b) = boldf {
        fonts.font_data.insert("ui-bold".into(), Arc::new(FontData::from_owned(b)));
        boldv.insert(0, "ui-bold".into());
    }
    if let Some(m) = mono {
        fonts.font_data.insert("ui-mono".into(), Arc::new(FontData::from_owned(m)));
        if let Some(v) = fonts.families.get_mut(&FontFamily::Monospace) {
            v.insert(0, "ui-mono".into());
        }
    }
    fonts.families.insert(FontFamily::Proportional, prop);
    fonts.families.insert(bold(), boldv);
    ctx.set_fonts(fonts);
}

pub fn apply(ctx: &egui::Context, dark: bool) {
    let c = colors(dark);
    let mut v = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    v.panel_fill = c.bg;
    v.window_fill = c.popup;
    v.extreme_bg_color = c.bg;
    v.override_text_color = None;
    v.widgets.noninteractive.fg_stroke.color = c.text;
    v.widgets.inactive.fg_stroke.color = c.text;
    v.widgets.hovered.fg_stroke.color = c.text;
    v.widgets.active.fg_stroke.color = c.text;
    v.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    v.widgets.inactive.bg_fill = Color32::TRANSPARENT;
    v.widgets.hovered.weak_bg_fill = c.hover;
    v.widgets.hovered.bg_fill = c.hover;
    v.widgets.active.weak_bg_fill = c.selected;
    v.widgets.hovered.bg_stroke = egui::Stroke::NONE;
    v.widgets.inactive.bg_stroke = egui::Stroke::NONE;
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, c.line);
    v.selection.bg_fill = if dark { Color32::from_rgb(35, 70, 110) } else { Color32::from_rgb(203, 226, 247) };
    v.selection.stroke = egui::Stroke::new(1.0, c.text);
    v.text_cursor.stroke = egui::Stroke::new(1.5, c.text);
    v.window_stroke = egui::Stroke::new(1.0, c.line);
    v.window_corner_radius = egui::CornerRadius::same(8);
    v.menu_corner_radius = egui::CornerRadius::same(8);
    v.hyperlink_color = c.accent;
    ctx.set_visuals(v);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(6.0, 2.0);
        s.spacing.button_padding = egui::vec2(6.0, 3.0);
        s.interaction.selectable_labels = false;
        use egui::{FontId, TextStyle};
        s.text_styles.insert(TextStyle::Body, FontId::proportional(14.0));
        s.text_styles.insert(TextStyle::Button, FontId::proportional(14.0));
        s.text_styles.insert(TextStyle::Small, FontId::proportional(12.0));
        s.text_styles.insert(TextStyle::Monospace, FontId::monospace(13.5));
    });
}

pub const COLOR_NAMES: [&str; 10] = ["default", "gray", "brown", "orange", "yellow", "green", "blue", "purple", "pink", "red"];

/// Block / tag background for a colour index.
pub fn block_bg(i: u8, dark: bool) -> Color32 {
    let light = [
        Color32::TRANSPARENT,
        Color32::from_rgb(241, 241, 239),
        Color32::from_rgb(244, 238, 238),
        Color32::from_rgb(251, 236, 221),
        Color32::from_rgb(251, 243, 219),
        Color32::from_rgb(237, 243, 236),
        Color32::from_rgb(226, 238, 250),
        Color32::from_rgb(240, 233, 248),
        Color32::from_rgb(249, 238, 243),
        Color32::from_rgb(253, 235, 236),
    ];
    let darkc = [
        Color32::TRANSPARENT,
        Color32::from_rgb(47, 47, 47),
        Color32::from_rgb(74, 50, 40),
        Color32::from_rgb(92, 59, 35),
        Color32::from_rgb(86, 67, 40),
        Color32::from_rgb(36, 61, 48),
        Color32::from_rgb(20, 58, 78),
        Color32::from_rgb(60, 45, 73),
        Color32::from_rgb(78, 44, 60),
        Color32::from_rgb(82, 46, 42),
    ];
    let i = (i as usize).min(9);
    if dark { darkc[i] } else { light[i] }
}

/// Stable colour for a select / tag option.
pub fn tag_color(name: &str) -> u8 {
    let mut h: u32 = 5381;
    for b in name.bytes() {
        h = h.wrapping_mul(33) ^ b as u32;
    }
    1 + (h % 9) as u8
}
