//! Draws the app icons used by the web app (PWA): `cargo run --example make_icons`.
//! A white page with a folded corner and text lines on a blue gradient.

fn main() {
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("web/icons");
    std::fs::create_dir_all(&out).unwrap();
    for (name, size, maskable) in [("icon-180.png", 180, false), ("icon-192.png", 192, false), ("icon-512.png", 512, false), ("icon-maskable-512.png", 512, true)] {
        draw(size, maskable).save(out.join(name)).unwrap();
        println!("wrote {name}");
    }
}

fn draw(size: u32, maskable: bool) -> image::RgbaImage {
    let s = size as f32;
    let ss = 4; // supersampling for smooth edges
    let mut img = image::RgbaImage::new(size, size);
    for y in 0..size {
        for x in 0..size {
            let mut acc = [0f32; 4];
            for sy in 0..ss {
                for sx in 0..ss {
                    let u = (x as f32 + (sx as f32 + 0.5) / ss as f32) / s;
                    let v = (y as f32 + (sy as f32 + 0.5) / ss as f32) / s;
                    let c = sample(u, v, maskable);
                    for k in 0..4 {
                        acc[k] += c[k];
                    }
                }
            }
            let n = (ss * ss) as f32;
            img.put_pixel(x, y, image::Rgba([(acc[0] / n) as u8, (acc[1] / n) as u8, (acc[2] / n) as u8, (acc[3] / n) as u8]));
        }
    }
    img
}

fn rounded(u: f32, v: f32, x0: f32, y0: f32, x1: f32, y1: f32, r: f32) -> bool {
    let cx = u.clamp(x0 + r, x1 - r);
    let cy = v.clamp(y0 + r, y1 - r);
    u >= x0 && u <= x1 && v >= y0 && v <= y1 && ((u - cx).powi(2) + (v - cy).powi(2)).sqrt() <= r
}

fn sample(u: f32, v: f32, maskable: bool) -> [f32; 4] {
    // Background: full square for maskable / iOS (it rounds corners itself).
    let t = (u + v) / 2.0;
    let bg = [35.0 + (95.0 - 35.0) * t, 131.0 + (90.0 - 131.0) * t, 226.0 + (210.0 - 226.0) * t, 255.0];
    let _ = maskable;
    let mut c = bg;
    // Page
    let (x0, y0, x1, y1) = (0.28, 0.2, 0.72, 0.8);
    let fold = 0.11;
    let in_page = rounded(u, v, x0, y0, x1, y1, 0.035) && !(u > x1 - fold && v < y0 + fold && (u - (x1 - fold)) > (v - y0));
    if in_page {
        c = [255.0, 255.0, 255.0, 255.0];
        // folded corner
        if u > x1 - fold && v < y0 + fold {
            c = [214.0, 226.0, 245.0, 255.0];
        }
        // text lines
        let lines = [(0.36, 0.62), (0.45, 0.62), (0.54, 0.56), (0.63, 0.6)];
        for (k, (ly, lx1)) in lines.iter().enumerate() {
            let lx0 = 0.36;
            let h = if k == 0 { 0.035 } else { 0.022 };
            if v > *ly - h / 2.0 && v < *ly + h / 2.0 && u > lx0 && u < *lx1 {
                c = if k == 0 { [55.0, 53.0, 47.0, 255.0] } else { [170.0, 169.0, 166.0, 255.0] };
            }
        }
    }
    c
}
