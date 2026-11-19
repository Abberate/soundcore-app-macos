//! Renders the app icon (1024×1024 PNG): a blue squircle with a white headphone glyph.
//! Original artwork, drawn with signed distance functions and 4×4 supersampling.
//!
//!   cargo run --release -p soundcore-app --example make_icon -- packaging/icon_1024.png

use image::{Rgba, RgbaImage};

const SIZE: u32 = 1024;
const SS: u32 = 4;

fn rounded_rect(px: f32, py: f32, cx: f32, cy: f32, hw: f32, hh: f32, r: f32) -> f32 {
    let qx = (px - cx).abs() - (hw - r);
    let qy = (py - cy).abs() - (hh - r);
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    outside + qx.max(qy).min(0.0) - r
}

/// Headphone glyph: an upper arc for the band, two rounded cups.
fn glyph(px: f32, py: f32) -> bool {
    let (cx, cy) = (512.0, 548.0);
    let band_r = 236.0;
    let band_w = 30.0;
    let d = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
    let band = (d - band_r).abs() <= band_w && py <= cy + 40.0;
    let cups = [-1.0f32, 1.0]
        .iter()
        .any(|s| rounded_rect(px, py, cx + s * 222.0, cy + 92.0, 62.0, 112.0, 52.0) <= 0.0);
    band || cups
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "icon_1024.png".into());
    let mut img = RgbaImage::new(SIZE, SIZE);
    let top = [0x3B as f32, 0x95 as f32, 0xFF as f32];
    let bottom = [0x0B as f32, 0x5C as f32, 0xE6 as f32];
    for y in 0..SIZE {
        for x in 0..SIZE {
            let (mut body, mut mark) = (0u32, 0u32);
            for sy in 0..SS {
                for sx in 0..SS {
                    let px = x as f32 + (sx as f32 + 0.5) / SS as f32;
                    let py = y as f32 + (sy as f32 + 0.5) / SS as f32;
                    // macOS icon grid: 824 px body, ~185 px corner radius.
                    if rounded_rect(px, py, 512.0, 512.0, 412.0, 412.0, 185.0) <= 0.0 {
                        body += 1;
                        if glyph(px, py) {
                            mark += 1;
                        }
                    }
                }
            }
            if body == 0 {
                continue;
            }
            let n = (SS * SS) as f32;
            let t = ((y as f32 - 100.0) / 824.0).clamp(0.0, 1.0);
            let m = mark as f32 / body as f32;
            let mut c = [0u8; 3];
            for i in 0..3 {
                let bg = top[i] + (bottom[i] - top[i]) * t;
                c[i] = (bg * (1.0 - m) + 255.0 * m).round() as u8;
            }
            let a = (body as f32 / n * 255.0).round() as u8;
            img.put_pixel(x, y, Rgba([c[0], c[1], c[2], a]));
        }
    }
    img.save(&out).expect("write icon");
    println!("wrote {out}");
}
