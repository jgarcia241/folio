//! Draws the app icons: `cargo run --example make_icons`.
//! A white page with a folded corner, a bookmark ribbon and text lines on a
//! glowing red gradient. Writes the web/PWA icons and the desktop icon.

use std::path::Path;

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let web = root.join("web/icons");
    std::fs::create_dir_all(&web).unwrap();
    // iOS / Android round the corners themselves: full-bleed squares.
    for (name, size, scale) in [("icon-180.png", 180, 1.0), ("icon-192.png", 192, 1.0), ("icon-512.png", 512, 1.0), ("icon-maskable-512.png", 512, 0.8)] {
        draw(size, Style::FullBleed { scale }).save(web.join(name)).unwrap();
        println!("wrote web/icons/{name}");
    }
    // Desktop (Dock / taskbar): rounded square with a margin and a soft shadow.
    std::fs::create_dir_all(root.join("assets")).unwrap();
    draw(512, Style::Desktop).save(root.join("assets/icon-desktop.png")).unwrap();
    println!("wrote assets/icon-desktop.png");
}

#[derive(Clone, Copy)]
enum Style {
    FullBleed { scale: f32 },
    Desktop,
}

type C = [f32; 4];

fn mix(a: C, b: C, t: f32) -> C {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t, a[3] + (b[3] - a[3]) * t]
}

/// Paint `top` (straight alpha) over `base`.
fn over(base: C, top: C) -> C {
    let a = top[3] / 255.0;
    let ba = base[3] / 255.0;
    let out_a = a + ba * (1.0 - a);
    if out_a <= 0.0 {
        return [0.0; 4];
    }
    let ch = |i: usize| (top[i] * a + base[i] * ba * (1.0 - a)) / out_a;
    [ch(0), ch(1), ch(2), out_a * 255.0]
}

/// Signed distance to a rounded rectangle (negative inside).
fn sd_round_rect(u: f32, v: f32, x0: f32, y0: f32, x1: f32, y1: f32, r: f32) -> f32 {
    let cx = (x0 + x1) / 2.0;
    let cy = (y0 + y1) / 2.0;
    let hx = (x1 - x0) / 2.0 - r;
    let hy = (y1 - y0) / 2.0 - r;
    let dx = (u - cx).abs() - hx;
    let dy = (v - cy).abs() - hy;
    let outside = (dx.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt();
    outside + dx.max(dy).min(0.0) - r
}

fn coverage(d: f32, px: f32) -> f32 {
    (0.5 - d / px).clamp(0.0, 1.0)
}

fn draw(size: u32, style: Style) -> image::RgbaImage {
    let s = size as f32;
    let px = 1.0 / s;
    let mut img = image::RgbaImage::new(size, size);
    for y in 0..size {
        for x in 0..size {
            let u = (x as f32 + 0.5) / s;
            let v = (y as f32 + 0.5) / s;
            let c = sample(u, v, px, style);
            img.put_pixel(x, y, image::Rgba([c[0] as u8, c[1] as u8, c[2] as u8, c[3] as u8]));
        }
    }
    img
}

fn sample(u: f32, v: f32, px: f32, style: Style) -> C {
    // Where the coloured tile is, and how big the artwork is inside it.
    let (tile, art) = match style {
        Style::FullBleed { scale } => ((0.0, 0.0, 1.0, 1.0, 0.0), scale),
        Style::Desktop => ((0.1, 0.09, 0.9, 0.89, 0.18), 0.8),
    };
    let (tx0, ty0, tx1, ty1, tr) = tile;
    let mut c: C = [0.0, 0.0, 0.0, 0.0];

    if let Style::Desktop = style {
        // Soft shadow under the tile.
        let d = sd_round_rect(u, v - 0.015, tx0, ty0, tx1, ty1, tr);
        let a = (1.0 - (d / 0.05).clamp(0.0, 1.0)).powi(2) * 0.35;
        c = over(c, [0.0, 0.0, 0.0, 255.0 * a]);
    }
    let tile_d = if tr > 0.0 { sd_round_rect(u, v, tx0, ty0, tx1, ty1, tr) } else { -1.0 };
    let tile_cov = if tr > 0.0 { coverage(tile_d, px) } else { 1.0 };
    if tile_cov <= 0.0 {
        return c;
    }

    // Tile-local coordinates (0..1 across the tile).
    let lu = (u - tx0) / (tx1 - tx0);
    let lv = (v - ty0) / (ty1 - ty0);

    // Background: deep crimson to warm coral, with a glow at the top left.
    let deep: C = [150.0, 12.0, 38.0, 255.0];
    let coral: C = [255.0, 92.0, 72.0, 255.0];
    let mut bg = mix(coral, deep, (lu * 0.45 + lv * 0.75).clamp(0.0, 1.0));
    let glow = (1.0 - ((lu - 0.22).powi(2) + (lv - 0.12).powi(2)).sqrt() / 0.75).clamp(0.0, 1.0);
    bg = mix(bg, [255.0, 170.0, 140.0, 255.0], glow * glow * 0.45);
    // Subtle diagonal sheen.
    let sheen = ((lu + lv - 0.55).abs() < 0.12) as i32 as f32 * 0.06;
    bg = mix(bg, [255.0, 255.0, 255.0, 255.0], sheen);

    // Artwork (page) in tile coordinates, centred and scaled.
    let to_art = |a: f32| 0.5 + (a - 0.5) / art;
    let au = to_art(lu);
    let av = to_art(lv);
    let apx = px / (tx1 - tx0) / art;

    let (x0, y0, x1, y1) = (0.27, 0.18, 0.73, 0.82);
    let fold = 0.12;
    let mut col = bg;

    // Page shadow.
    let sd = sd_round_rect(au - 0.012, av - 0.03, x0, y0, x1, y1, 0.045);
    let sh = (1.0 - (sd / 0.06).clamp(0.0, 1.0)).powi(2) * 0.38;
    col = over(col, [70.0, 0.0, 15.0, 255.0 * sh]);

    // Page body (with the top-right corner cut for the fold).
    let pd = sd_round_rect(au, av, x0, y0, x1, y1, 0.045);
    let cut = (au - (x1 - fold)) - (av - y0); // > 0 above the fold diagonal
    let in_fold_box = au > x1 - fold && av < y0 + fold;
    let page_cov = coverage(pd, apx) * if in_fold_box { coverage(cut, apx) } else { 1.0 };
    if page_cov > 0.0 {
        let mut paper = mix([255.0, 255.0, 255.0, 255.0], [246.0, 240.0, 238.0, 255.0], (av - y0) / (y1 - y0));
        // Text lines: a bold title and grey lines.
        let lines: [(f32, f32, f32, C); 5] = [
            (0.385, 0.6, 0.042, [60.0, 40.0, 42.0, 255.0]),
            (0.49, 0.64, 0.026, [196.0, 186.0, 186.0, 255.0]),
            (0.565, 0.6, 0.026, [196.0, 186.0, 186.0, 255.0]),
            (0.64, 0.64, 0.026, [196.0, 186.0, 186.0, 255.0]),
            (0.715, 0.52, 0.026, [196.0, 186.0, 186.0, 255.0]),
        ];
        for (ly, lx1, h, lc) in lines {
            let d = sd_round_rect(au, av, 0.345, ly - h / 2.0, lx1, ly + h / 2.0, h / 2.0);
            paper = mix(paper, lc, coverage(d, apx));
        }
        col = mix(col, paper, page_cov);
    }
    // Folded corner.
    if in_fold_box && cut <= 0.0 + apx {
        let fd = -cut;
        let f_cov = coverage(-fd, apx).min(coverage(pd, apx).max(if in_fold_box { 1.0 } else { 0.0 }));
        let fold_col = mix([255.0, 214.0, 210.0, 255.0], [240.0, 170.0, 165.0, 255.0], ((au - (x1 - fold)) / fold).clamp(0.0, 1.0));
        let inside_corner = au >= x1 - fold && av <= y0 + fold && (au - (x1 - fold)) <= (av - y0) + apx;
        if inside_corner {
            col = mix(col, fold_col, f_cov);
        }
    }
    // Bookmark ribbon hanging from the top edge.
    let (rx0, rx1, ry0, ry1) = (0.6, 0.665, 0.15, 0.36);
    if au > rx0 - apx && au < rx1 + apx && av > ry0 && av < ry1 + 0.05 {
        let mid = (rx0 + rx1) / 2.0;
        let notch = ry1 - (0.04 - (au - mid).abs() * 1.2).max(0.0);
        let inside = au >= rx0 && au <= rx1 && av <= notch;
        if inside {
            let shade = mix([214.0, 30.0, 52.0, 255.0], [168.0, 14.0, 36.0, 255.0], (au - rx0) / (rx1 - rx0));
            col = mix(col, shade, 1.0);
        }
    }

    // Edge highlight on the tile.
    if tr > 0.0 {
        let rim = (1.0 - ((-tile_d) / 0.012).clamp(0.0, 1.0)) * 0.25 * (1.0 - lv);
        col = mix(col, [255.0, 255.0, 255.0, 255.0], rim);
    }
    col[3] = 255.0 * tile_cov;
    over(c, col)
}
