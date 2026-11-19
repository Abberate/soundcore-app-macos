//! Renders the app icon (1024×1024 PNG) in one of three original styles, drawn with
//! signed distance functions and 4×4 supersampling. No brand artwork is used.
//!
//!   cargo run --release -p soundcore-app --example make_icon -- <out.png> [aurora|wave|night]
//!   cargo run --release -p soundcore-app --example make_icon -- --sheet <dir>
//!
//! * aurora: white ground, soft cyan/blue light ribbons, dark headphone mark;
//! * wave:   cyan-to-blue gradient, white headphones around equalizer bars;
//! * night:  deep navy ground, glowing cyan-to-blue headphones.

use image::{Rgba, RgbaImage, imageops};

const SIZE: u32 = 1024;
const SS: u32 = 4;

type Rgb = [f32; 3];

fn hex(c: u32) -> Rgb {
    [
        ((c >> 16) & 0xff) as f32,
        ((c >> 8) & 0xff) as f32,
        (c & 0xff) as f32,
    ]
}

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn rounded_rect(px: f32, py: f32, cx: f32, cy: f32, hw: f32, hh: f32, r: f32) -> f32 {
    let qx = (px - cx).abs() - (hw - r);
    let qy = (py - cy).abs() - (hh - r);
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    outside + qx.max(qy).min(0.0) - r
}

/// macOS icon grid: 824 px body, ~185 px corner radius.
fn body(px: f32, py: f32) -> f32 {
    rounded_rect(px, py, 512.0, 512.0, 412.0, 412.0, 185.0)
}

/// Headphone mark as a signed distance (negative inside).
fn headphones(px: f32, py: f32, scale: f32, band_w: f32) -> f32 {
    let (cx, cy) = (512.0, 548.0);
    let r = 236.0 * scale;
    let d = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
    let band = if py <= cy + 40.0 * scale {
        (d - r).abs() - band_w * scale
    } else {
        f32::MAX
    };
    let cups = [-1.0f32, 1.0]
        .iter()
        .map(|s| {
            rounded_rect(
                px,
                py,
                cx + s * 222.0 * scale,
                cy + 92.0 * scale,
                62.0 * scale,
                112.0 * scale,
                52.0 * scale,
            )
        })
        .fold(f32::MAX, f32::min);
    band.min(cups)
}

/// Five rounded equalizer bars between the ear cups.
fn bars(px: f32, py: f32) -> f32 {
    let heights = [70.0, 130.0, 190.0, 130.0, 70.0];
    heights
        .iter()
        .enumerate()
        .map(|(i, h)| {
            rounded_rect(
                px,
                py,
                512.0 + (i as f32 - 2.0) * 58.0,
                640.0,
                17.0,
                h / 2.0,
                17.0,
            )
        })
        .fold(f32::MAX, f32::min)
}

/// Soft light ribbon: a sine-shaped band with a gaussian falloff.
fn ribbon(px: f32, py: f32, base: f32, amp: f32, freq: f32, phase: f32, width: f32) -> f32 {
    let y = base + amp * (px * freq + phase).sin() + (px - 512.0) * 0.35;
    let d = (py - y) / width;
    (-d * d).exp()
}

fn shade(style: &str, px: f32, py: f32) -> Rgb {
    match style {
        "wave" => {
            let t = ((px + py) / 2048.0).clamp(0.0, 1.0);
            let mut c = mix(hex(0x48D6FF), hex(0x1257E8), t);
            let r = ribbon(px, py, 300.0, 60.0, 0.006, 0.8, 140.0) * 0.18;
            c = mix(c, [255.0; 3], r);
            let mark = headphones(px, py, 1.0, 30.0).min(bars(px, py));
            if mark <= 0.0 { [255.0; 3] } else { c }
        }
        "night" => {
            let t = (py / 1024.0).clamp(0.0, 1.0);
            let mut c = mix(hex(0x15203A), hex(0x05070D), t);
            let mark = headphones(px, py, 1.0, 30.0);
            // Glow around the mark.
            let glow = (-(mark.max(0.0)) / 46.0).exp() * 0.55;
            let ang = ((px - 512.0) / 520.0 + 0.5).clamp(0.0, 1.0);
            let neon = mix(hex(0x4FE3FF), hex(0x2F6BFF), ang);
            c = mix(c, neon, glow);
            if mark <= 0.0 {
                mix(neon, [255.0; 3], 0.12)
            } else {
                c
            }
        }
        _ => {
            // aurora
            let mut c = hex(0xFBFDFF);
            let ribbons = [
                (250.0, 70.0, 0.0050, 0.4, 120.0, 0x8FE6FF, 0.55),
                (430.0, 90.0, 0.0042, 2.1, 150.0, 0x5CB8FF, 0.38),
                (760.0, 60.0, 0.0060, 4.0, 130.0, 0x7DDCFF, 0.45),
            ];
            for (base, amp, freq, phase, width, col, k) in ribbons {
                c = mix(
                    c,
                    hex(col),
                    ribbon(px, py, base, amp, freq, phase, width) * k,
                );
            }
            let mark = headphones(px, py, 0.92, 26.0);
            if mark <= 0.0 { hex(0x12161D) } else { c }
        }
    }
}

fn render(style: &str) -> RgbaImage {
    let mut img = RgbaImage::new(SIZE, SIZE);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let (mut inside, mut acc) = (0u32, [0.0f32; 3]);
            for sy in 0..SS {
                for sx in 0..SS {
                    let px = x as f32 + (sx as f32 + 0.5) / SS as f32;
                    let py = y as f32 + (sy as f32 + 0.5) / SS as f32;
                    if body(px, py) <= 0.0 {
                        inside += 1;
                        let c = shade(style, px, py);
                        for i in 0..3 {
                            acc[i] += c[i];
                        }
                    }
                }
            }
            if inside == 0 {
                continue;
            }
            let n = inside as f32;
            let a = (inside as f32 / (SS * SS) as f32 * 255.0).round() as u8;
            img.put_pixel(
                x,
                y,
                Rgba([
                    (acc[0] / n) as u8,
                    (acc[1] / n) as u8,
                    (acc[2] / n) as u8,
                    a,
                ]),
            );
        }
    }
    img
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--sheet") {
        let dir = std::path::PathBuf::from(args.get(1).cloned().unwrap_or_else(|| ".".into()));
        let styles = ["aurora", "wave", "night"];
        let mut sheet = RgbaImage::from_pixel(3 * 560 + 80, 640, Rgba([236, 239, 244, 255]));
        for (i, s) in styles.iter().enumerate() {
            let img = render(s);
            img.save(dir.join(format!("icon-{}-{s}.png", i + 1)))
                .expect("write icon");
            let small = imageops::resize(&img, 512, 512, imageops::FilterType::Lanczos3);
            imageops::overlay(&mut sheet, &small, 64 + i as i64 * 560, 64);
        }
        sheet
            .save(dir.join("icon-proposals.png"))
            .expect("write sheet");
        println!(
            "wrote icon-proposals.png and three icons in {}",
            dir.display()
        );
        return;
    }
    let out = args
        .first()
        .cloned()
        .unwrap_or_else(|| "icon_1024.png".into());
    let style = args.get(1).cloned().unwrap_or_else(|| "aurora".into());
    render(&style).save(&out).expect("write icon");
    println!("wrote {out} ({style})");
}
