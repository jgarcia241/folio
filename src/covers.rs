//! Page covers: gradients, solid colours and patterned pictures drawn in code,
//! plus pictures the person uploads (stored in the workspace's `_assets`).

use eframe::egui::{self, Color32, ColorImage, Rect, TextureHandle, pos2};

#[derive(Clone, Copy)]
pub enum Cover {
    None,
    Grad(Color32, Color32),
    Solid(Color32),
    Pattern(u8),
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// Index 0 is "no cover". Indices 1-5 match earlier versions; keep the order.
pub const COVERS: &[Cover] = &[
    Cover::None,
    Cover::Grad(rgb(250, 222, 201), rgb(214, 228, 248)),
    Cover::Grad(rgb(208, 232, 214), rgb(196, 220, 236)),
    Cover::Grad(rgb(236, 218, 245), rgb(250, 214, 220)),
    Cover::Grad(rgb(253, 236, 200), rgb(244, 201, 178)),
    Cover::Grad(rgb(52, 73, 94), rgb(100, 140, 170)),
    // more gradients
    Cover::Grad(rgb(255, 154, 139), rgb(255, 106, 136)),
    Cover::Grad(rgb(132, 250, 176), rgb(143, 211, 244)),
    Cover::Grad(rgb(161, 140, 209), rgb(251, 194, 235)),
    Cover::Grad(rgb(246, 211, 101), rgb(253, 160, 133)),
    Cover::Grad(rgb(33, 147, 176), rgb(109, 213, 237)),
    Cover::Grad(rgb(15, 32, 39), rgb(44, 83, 100)),
    Cover::Grad(rgb(238, 156, 167), rgb(255, 221, 225)),
    Cover::Grad(rgb(67, 206, 162), rgb(24, 90, 157)),
    Cover::Grad(rgb(255, 216, 155), rgb(25, 84, 123)),
    Cover::Grad(rgb(230, 218, 218), rgb(39, 64, 70)),
    // solids
    Cover::Solid(rgb(235, 87, 87)),
    Cover::Solid(rgb(242, 153, 74)),
    Cover::Solid(rgb(242, 201, 76)),
    Cover::Solid(rgb(111, 207, 151)),
    Cover::Solid(rgb(86, 204, 242)),
    Cover::Solid(rgb(47, 128, 237)),
    Cover::Solid(rgb(155, 81, 224)),
    Cover::Solid(rgb(55, 53, 47)),
    // patterned pictures
    Cover::Pattern(0),
    Cover::Pattern(1),
    Cover::Pattern(2),
    Cover::Pattern(3),
    Cover::Pattern(4),
    Cover::Pattern(5),
    Cover::Pattern(6),
    Cover::Pattern(7),
    Cover::Pattern(8),
    Cover::Pattern(9),
    Cover::Pattern(10),
    Cover::Pattern(11),
];

pub const PATTERN_NAMES: [&str; 12] = [
    "Waves", "Hills", "Dunes", "Night sky", "Shapes", "Stripes", "Dots", "Sunset", "Aurora", "Blueprint", "Contours", "Mesh",
];

pub fn get(i: u8) -> Cover {
    COVERS.get(i as usize).copied().unwrap_or(Cover::None)
}

pub fn gallery_indices() -> Vec<u8> {
    (0..COVERS.len() as u8).filter(|i| matches!(COVERS[*i as usize], Cover::Pattern(_))).collect()
}

pub fn color_indices() -> Vec<u8> {
    (1..COVERS.len() as u8).filter(|i| matches!(COVERS[*i as usize], Cover::Grad(..) | Cover::Solid(_))).collect()
}

// ------------------------------------------------------------ patterns

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn c3(r: u8, g: u8, b: u8) -> [f32; 3] {
    [r as f32, g as f32, b as f32]
}

fn hash(x: u32) -> u32 {
    let mut x = x.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    x
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Cheap 1-D value noise.
fn noise1(x: f32, seed: u32) -> f32 {
    let i = x.floor();
    let f = x - i;
    let a = (hash(i as i32 as u32 ^ seed) & 0xffff) as f32 / 65535.0;
    let b = (hash((i as i32 + 1) as u32 ^ seed) & 0xffff) as f32 / 65535.0;
    a + (b - a) * smooth(f)
}

/// Cheap 2-D value noise.
fn noise2(x: f32, y: f32, seed: u32) -> f32 {
    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    let fx = smooth(x - xi as f32);
    let fy = smooth(y - yi as f32);
    let h = |a: i32, b: i32| (hash((a as u32).wrapping_mul(73856093) ^ (b as u32).wrapping_mul(19349663) ^ seed) & 0xffff) as f32 / 65535.0;
    let top = h(xi, yi) + (h(xi + 1, yi) - h(xi, yi)) * fx;
    let bot = h(xi, yi + 1) + (h(xi + 1, yi + 1) - h(xi, yi + 1)) * fx;
    top + (bot - top) * fy
}

fn fbm(x: f32, y: f32, seed: u32) -> f32 {
    let mut v = 0.0;
    let mut amp = 0.5;
    let mut f = 1.0;
    for o in 0..4 {
        v += noise2(x * f, y * f, seed + o) * amp;
        f *= 2.0;
        amp *= 0.5;
    }
    v
}

pub const W: usize = 1024;
pub const H: usize = 256;

fn pixel(pattern: u8, x: usize, y: usize) -> [f32; 3] {
    let u = x as f32 / W as f32;
    let v = y as f32 / H as f32;
    match pattern {
        // Waves: layered teal bands on cream
        0 => {
            let bg = mix(c3(244, 236, 220), c3(250, 244, 232), v);
            let mut col = bg;
            let layers = [c3(160, 205, 210), c3(90, 160, 175), c3(40, 110, 140), c3(22, 70, 105)];
            for (k, lc) in layers.iter().enumerate() {
                let base = 0.35 + k as f32 * 0.17;
                let w = base + 0.06 * (u * 12.0 + k as f32 * 1.7).sin() + 0.03 * (u * 31.0 + k as f32).sin();
                if v > w {
                    col = *lc;
                    // white crest line
                    if v - w < 0.012 {
                        col = mix(*lc, c3(255, 255, 255), 0.75);
                    }
                }
            }
            col
        }
        // Hills: misty layered ridges
        1 => {
            let sky = mix(c3(250, 226, 205), c3(214, 230, 240), 1.0 - v);
            let mut col = sky;
            let sun = ((u - 0.72).powi(2) * 16.0 + (v - 0.32).powi(2) * 1.0).sqrt();
            if sun < 0.12 {
                col = mix(c3(255, 246, 225), col, sun / 0.12);
            }
            let ridges = [c3(178, 196, 196), c3(128, 156, 150), c3(80, 116, 106), c3(42, 74, 66)];
            for (k, rc) in ridges.iter().enumerate() {
                let h = 0.42 + k as f32 * 0.13 - 0.16 * noise1(u * (3.0 + k as f32 * 1.5), 11 + k as u32);
                if v > h {
                    col = mix(*rc, sky, 0.12 * (3 - k) as f32 / 3.0);
                }
            }
            col
        }
        // Dunes
        2 => {
            let sky = mix(c3(255, 190, 160), c3(255, 225, 200), v * 1.4);
            let mut col = sky;
            for k in 0..3 {
                let h = 0.45 + k as f32 * 0.17 + 0.08 * (u * (4.0 + k as f32) + k as f32 * 2.0).sin();
                if v > h {
                    let shade = ((u * (4.0 + k as f32) + k as f32 * 2.0).cos() * 0.5 + 0.5) * 0.5;
                    let base = [c3(232, 150, 100), c3(214, 120, 80), c3(186, 92, 64)][k];
                    col = mix(base, c3(120, 50, 40), shade * 0.6 + (v - h) * 0.6);
                }
            }
            col
        }
        // Night sky
        3 => {
            let mut col = mix(c3(30, 27, 75), c3(88, 50, 120), v + 0.2 * (u * 3.0).sin());
            let band = (-(v - 0.5 - 0.25 * (u * 2.5).sin()).powi(2) * 30.0).exp();
            col = mix(col, c3(150, 130, 200), band * 0.35 * fbm(u * 8.0, v * 4.0, 5));
            let cell = (x / 6) as u32 * 7919 + (y / 6) as u32 * 104729;
            let hv = hash(cell);
            if hv % 23 == 0 && x % 6 == (hv as usize >> 8) % 6 && y % 6 == (hv as usize >> 12) % 6 {
                col = mix(col, c3(255, 255, 245), 0.6 + ((hv >> 16) & 0xff) as f32 / 640.0);
            }
            col
        }
        // Geometric shapes
        4 => {
            let mut col = c3(246, 240, 228);
            let px = x as f32;
            let py = y as f32;
            let circ = |cx: f32, cy: f32, r: f32| ((px - cx).powi(2) + (py - cy).powi(2)).sqrt() < r;
            if px > 120.0 && px < 330.0 && py > 40.0 {
                col = c3(32, 52, 92);
            }
            if circ(260.0, 140.0, 95.0) {
                col = c3(226, 172, 60);
            }
            if circ(520.0, 256.0, 150.0) && py < 256.0 {
                col = c3(204, 96, 66);
            }
            if px > 640.0 && px < 980.0 && py > 70.0 && py < 150.0 {
                col = c3(32, 52, 92);
            }
            if circ(780.0, 110.0, 60.0) {
                col = c3(240, 230, 210);
            }
            if circ(900.0, 200.0, 40.0) {
                col = c3(226, 172, 60);
            }
            col
        }
        // Diagonal stripes
        5 => {
            let t = ((x + y) / 40) % 4;
            [c3(255, 205, 178), c3(255, 180, 162), c3(229, 152, 155), c3(181, 131, 141)][t]
        }
        // Polka dots
        6 => {
            let s = 32.0;
            let gx = (x as f32 % s) - s / 2.0;
            let gy = (y as f32 % s) - s / 2.0;
            let off = if (y as f32 / s) as i32 % 2 == 0 { 0.0 } else { 0.0 };
            if (gx + off).hypot(gy) < 5.0 { c3(255, 255, 255) } else { mix(c3(120, 170, 230), c3(150, 120, 220), u) }
        }
        // Sunset bands with a sun
        7 => {
            let bands = [c3(255, 214, 140), c3(255, 170, 110), c3(240, 120, 100), c3(180, 80, 110), c3(90, 50, 100)];
            let bi = ((v * 5.0) as usize).min(4);
            let mut col = bands[bi];
            let d = ((u - 0.5) * 4.0).hypot(v - 0.75);
            if d < 0.42 && v < 0.75 {
                col = mix(c3(255, 245, 200), c3(255, 200, 120), d / 0.42);
            }
            if v > 0.75 {
                col = mix(c3(60, 40, 90), c3(30, 25, 60), (v - 0.75) * 4.0);
                if ((y as i32 - 200) % 9 == 0) && (u - 0.5).abs() < 0.15 - (v - 0.75) * 0.3 {
                    col = c3(255, 210, 150);
                }
            }
            col
        }
        // Aurora
        8 => {
            let mut col = mix(c3(8, 18, 40), c3(10, 40, 60), v);
            let a = (-(v - 0.35 - 0.15 * (u * 6.0).sin() - 0.05 * (u * 17.0).sin()).powi(2) * 60.0).exp();
            let b = (-(v - 0.55 - 0.1 * (u * 4.0 + 1.0).sin()).powi(2) * 90.0).exp();
            col = mix(col, c3(70, 230, 160), a * (0.6 + 0.4 * fbm(u * 20.0, v * 2.0, 3)));
            col = mix(col, c3(160, 90, 220), b * 0.6);
            col
        }
        // Blueprint grid
        9 => {
            let mut col = c3(30, 74, 140);
            if x % 64 == 0 || y % 64 == 0 {
                col = c3(120, 160, 210);
            } else if x % 16 == 0 || y % 16 == 0 {
                col = c3(60, 104, 170);
            }
            col
        }
        // Topographic contours
        10 => {
            let n = fbm(u * 5.0, v * 1.5, 9) * 12.0;
            let f = n - n.floor();
            if f < 0.06 { c3(120, 140, 110) } else { mix(c3(232, 236, 220), c3(214, 226, 200), n / 12.0) }
        }
        // Mesh gradient
        _ => {
            let a = mix(c3(255, 175, 189), c3(255, 195, 160), u);
            let b = mix(c3(140, 180, 255), c3(180, 240, 210), u);
            mix(a, b, v + 0.15 * (u * 6.0).sin())
        }
    }
}

pub fn render_pattern(pattern: u8, w: usize, h: usize) -> ColorImage {
    let mut pixels = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let p = pixel(pattern, x * W / w, y * H / h);
            pixels.push(Color32::from_rgb(p[0] as u8, p[1] as u8, p[2] as u8));
        }
    }
    ColorImage::new([w, h], pixels)
}

/// Paint a texture so it fills `rect`, cropping the middle like a photo.
pub fn paint_fill(painter: &egui::Painter, rect: Rect, tex: &TextureHandle) {
    let [tw, th] = tex.size();
    let ta = tw as f32 / th.max(1) as f32;
    let ra = rect.width() / rect.height().max(1.0);
    let uv = if ra > ta {
        let h = ta / ra;
        Rect::from_min_max(pos2(0.0, 0.5 - h / 2.0), pos2(1.0, 0.5 + h / 2.0))
    } else {
        let w = ra / ta;
        Rect::from_min_max(pos2(0.5 - w / 2.0, 0.0), pos2(0.5 + w / 2.0, 1.0))
    };
    painter.image(tex.id(), rect, uv, Color32::WHITE);
}

/// Paint a gradient left to right.
pub fn paint_grad(painter: &egui::Painter, r: Rect, a: Color32, b: Color32) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(r.left_top(), a);
    mesh.colored_vertex(r.right_top(), b);
    mesh.colored_vertex(r.right_bottom(), b);
    mesh.colored_vertex(r.left_bottom(), a);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(mesh);
}
