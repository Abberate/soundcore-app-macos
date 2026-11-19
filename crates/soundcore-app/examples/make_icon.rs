//! Renders the app icon (1024×1024 PNG), drawn with signed distance functions and 4×4
//! supersampling. Original artwork: no brand logo or lettering is used.
//!
//!   cargo run --release -p soundcore-app --example make_icon -- <out.png> [style]
//!   cargo run --release -p soundcore-app --example make_icon -- --sheet <dir>
//!
//! Styles:
//! * dark3d:  deep navy ground with flowing cyan light, sculpted silver headphones;
//! * light3d: pale ground with flowing cyan light, sculpted graphite headphones;
//! * aurora-text: pale ground with flowing cyan light, black mark and lettering;
//! * night-text:  deep navy ground with flowing cyan light, white mark and lettering;
//! * aurora, wave, night: the earlier flat proposals.

use image::{Rgba, RgbaImage, imageops};
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{FontRef, MetadataProvider};

const SIZE: u32 = 1024;
const SS: u32 = 4;

type Rgb = [f32; 3];
type V3 = [f32; 3];

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

fn scale(c: Rgb, k: f32) -> Rgb {
    [c[0] * k, c[1] * k, c[2] * k]
}

fn norm(v: V3) -> V3 {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-6);
    [v[0] / l, v[1] / l, v[2] / l]
}

fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
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

// ---- flat mark used by the earlier proposals ------------------------------------------

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
#[allow(clippy::too_many_arguments)]
fn ribbon(
    px: f32,
    py: f32,
    base: f32,
    amp: f32,
    freq: f32,
    phase: f32,
    width: f32,
    slope: f32,
) -> f32 {
    let y = base + amp * (px * freq + phase).sin() + (px - 512.0) * slope;
    let d = (py - y) / width;
    (-d * d).exp()
}

// ---- sculpted headphones ----------------------------------------------------------------

const BAND_C: (f32, f32) = (512.0, 560.0);
const BAND_R: f32 = 232.0;
const BAND_W: f32 = 34.0;
const CUP_HW: f32 = 74.0;
const CUP_HH: f32 = 128.0;
const CUP_R: f32 = 62.0;
const CUP_Y: f32 = 672.0;
const CUP_DX: f32 = 236.0;

/// Which part of the sculpted mark a point is on, with its surface normal.
enum Hit {
    None,
    Band(V3),
    Cup { normal: V3, side: f32, depth: f32 },
}

fn cup_sdf(px: f32, py: f32, side: f32) -> f32 {
    rounded_rect(
        px,
        py,
        BAND_C.0 + side * CUP_DX,
        CUP_Y,
        CUP_HW,
        CUP_HH,
        CUP_R,
    )
}

fn sculpt(px: f32, py: f32) -> Hit {
    // Ear cups (drawn over the band ends).
    for side in [-1.0f32, 1.0] {
        let d = cup_sdf(px, py, side);
        if d <= 0.0 {
            let e = 1.0;
            let gx = cup_sdf(px + e, py, side) - cup_sdf(px - e, py, side);
            let gy = cup_sdf(px, py + e, side) - cup_sdf(px, py - e, side);
            let g = norm([gx, gy, 0.0]);
            // Rounded bevel 46 px wide, flat-ish top.
            let t = (1.0 + d / 46.0).clamp(0.0, 1.0);
            let normal = norm([g[0] * t, g[1] * t, (1.0 - t * t).max(0.0).sqrt() + 0.15]);
            return Hit::Cup {
                normal,
                side,
                depth: -d,
            };
        }
    }
    // Headband: a tube following an arc.
    let (dx, dy) = (px - BAND_C.0, py - BAND_C.1);
    let r = (dx * dx + dy * dy).sqrt();
    let t = (r - BAND_R) / BAND_W;
    if t.abs() <= 1.0 && py <= BAND_C.1 + 10.0 {
        let radial = [dx / r, dy / r];
        let normal = norm([radial[0] * t, radial[1] * t, (1.0 - t * t).sqrt()]);
        return Hit::Band(normal);
    }
    Hit::None
}

/// Distance to the mark's silhouette (for shadows and glow).
fn silhouette(px: f32, py: f32) -> f32 {
    let (dx, dy) = (px - BAND_C.0, py - BAND_C.1);
    let r = (dx * dx + dy * dy).sqrt();
    let band = if py <= BAND_C.1 + 10.0 {
        (r - BAND_R).abs() - BAND_W
    } else {
        f32::MAX
    };
    band.min(cup_sdf(px, py, -1.0)).min(cup_sdf(px, py, 1.0))
}

/// Blinn–Phong with one key light from the top left and a cool rim light.
fn lit(base: Rgb, n: V3, spec_k: f32, rim: Rgb) -> Rgb {
    let l = norm([-0.45, -0.65, 0.62]);
    let h = norm([l[0], l[1], l[2] + 1.0]);
    let diffuse = dot(n, l).max(0.0);
    let spec = dot(n, h).max(0.0).powf(48.0) * spec_k;
    let fresnel = (1.0 - n[2]).clamp(0.0, 1.0).powf(2.0);
    let c = scale(base, 0.28 + 0.78 * diffuse);
    let c = mix(c, rim, fresnel * 0.55);
    [
        c[0] + 255.0 * spec,
        c[1] + 255.0 * spec,
        c[2] + 255.0 * spec,
    ]
}

fn sculpted(px: f32, py: f32, ground: Rgb, metal: Rgb, cushion: Rgb, rim: Rgb, glow: Rgb) -> Rgb {
    // Soft contact shadow under the mark.
    let shadow = (-(silhouette(px, py - 26.0).max(0.0)) / 34.0).exp() * 0.45;
    let mut c = mix(ground, scale(ground, 0.35), shadow);
    match sculpt(px, py) {
        Hit::None => {}
        Hit::Band(n) => c = lit(metal, n, 0.9, rim),
        Hit::Cup {
            normal,
            side,
            depth,
        } => {
            c = lit(metal, normal, 0.8, rim);
            // Cushion on the inner side and a thin light ring.
            let cx = BAND_C.0 + side * CUP_DX - side * 28.0;
            let cushion_d = rounded_rect(px, py, cx, CUP_Y, 30.0, CUP_HH - 26.0, 30.0);
            if cushion_d <= 0.0 && depth > 10.0 {
                let t = (1.0 + cushion_d / 20.0).clamp(0.0, 1.0);
                c = lit(cushion, norm([-side * t * 0.6, 0.0, 1.0]), 0.25, rim);
            }
            let ring = rounded_rect(
                px,
                py,
                BAND_C.0 + side * CUP_DX + side * 18.0,
                CUP_Y,
                22.0,
                70.0,
                22.0,
            );
            if ring.abs() <= 3.0 {
                c = mix(c, glow, 0.9);
            }
        }
    }
    c
}

// ---- mark and lettering for the named icons -------------------------------------------

const MARK_C: (f32, f32) = (512.0, 408.0);
const MARK_SCALE: f32 = 0.6;

fn named_mark(px: f32, py: f32) -> f32 {
    // `headphones` centres its arc at (512, 548); shift the point instead of the shape.
    let sx = 512.0 + (px - MARK_C.0) / MARK_SCALE;
    let sy = 548.0 + (py - MARK_C.1) / MARK_SCALE;
    headphones(sx, sy, 1.0, 30.0) * MARK_SCALE
}

const SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/Poppins-SemiBold.ttf");
const MEDIUM: &[u8] = include_bytes!("../assets/fonts/Poppins-Medium.ttf");

/// Collects a glyph outline as flattened contours in image coordinates.
struct Flatten {
    contours: Vec<Vec<(f32, f32)>>,
    current: Vec<(f32, f32)>,
    origin: (f32, f32),
    last: (f32, f32),
}

impl Flatten {
    fn point(&mut self, x: f32, y: f32) {
        let p = (self.origin.0 + x, self.origin.1 - y);
        self.current.push(p);
        self.last = (x, y);
    }
}

impl OutlinePen for Flatten {
    fn move_to(&mut self, x: f32, y: f32) {
        if self.current.len() > 2 {
            self.contours.push(std::mem::take(&mut self.current));
        }
        self.current.clear();
        self.point(x, y);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.point(x, y);
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        let (x0, y0) = self.last;
        for i in 1..=12 {
            let t = i as f32 / 12.0;
            let u = 1.0 - t;
            self.point(
                u * u * x0 + 2.0 * u * t * cx0 + t * t * x,
                u * u * y0 + 2.0 * u * t * cy0 + t * t * y,
            );
        }
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        let (x0, y0) = self.last;
        for i in 1..=16 {
            let t = i as f32 / 16.0;
            let u = 1.0 - t;
            let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            self.point(
                a * x0 + b * cx0 + c * cx1 + d * x,
                a * y0 + b * cy0 + c * cy1 + d * y,
            );
        }
    }

    fn close(&mut self) {
        if self.current.len() > 2 {
            self.contours.push(std::mem::take(&mut self.current));
        }
    }
}

/// Lays out one line centred on `cx`, its baseline at `baseline`, `width` wide.
/// Returns the outline contours.
fn line(
    font: &[u8],
    text: &str,
    width: f32,
    tracking: f32,
    cx: f32,
    baseline: f32,
) -> Vec<Vec<(f32, f32)>> {
    let font = FontRef::new(font).expect("font");
    let charmap = font.charmap();
    let outlines = font.outline_glyphs();
    let measure = |size: f32| {
        let metrics = font.glyph_metrics(Size::new(size), LocationRef::default());
        let n = text.chars().count() as f32;
        text.chars()
            .filter_map(|c| charmap.map(c))
            .map(|g| metrics.advance_width(g).unwrap_or(0.0))
            .sum::<f32>()
            + tracking * size * (n - 1.0)
    };
    // Size that makes the line exactly `width` wide.
    let size = 100.0 * width / measure(100.0);
    let metrics = font.glyph_metrics(Size::new(size), LocationRef::default());
    let mut pen = Flatten {
        contours: Vec::new(),
        current: Vec::new(),
        origin: (cx - width / 2.0, baseline),
        last: (0.0, 0.0),
    };
    for c in text.chars() {
        let Some(gid) = charmap.map(c) else { continue };
        if let Some(glyph) = outlines.get(gid) {
            let _ = glyph.draw(
                DrawSettings::unhinted(Size::new(size), LocationRef::default()),
                &mut pen,
            );
            pen.close();
        }
        pen.origin.0 += metrics.advance_width(gid).unwrap_or(0.0) + tracking * size;
    }
    pen.contours
}

/// Non-zero winding coverage of `contours` (4 sub-scanlines, exact spans).
fn rasterize(contours: &[Vec<(f32, f32)>]) -> Vec<f32> {
    let n = SIZE as usize;
    let mut cov = vec![0.0f32; n * n];
    let mut edges = Vec::new();
    for c in contours {
        for i in 0..c.len() {
            let (a, b) = (c[i], c[(i + 1) % c.len()]);
            if a.1 != b.1 {
                edges.push((a, b));
            }
        }
    }
    let ymin = edges
        .iter()
        .map(|e| e.0.1.min(e.1.1))
        .fold(f32::MAX, f32::min)
        .max(0.0) as usize;
    let ymax = (edges
        .iter()
        .map(|e| e.0.1.max(e.1.1))
        .fold(0.0, f32::max)
        .ceil() as usize)
        .min(n - 1);
    for y in ymin..=ymax {
        for k in 0..4 {
            let sy = y as f32 + (k as f32 + 0.5) / 4.0;
            let mut xs: Vec<(f32, i32)> = edges
                .iter()
                .filter(|(a, b)| sy >= a.1.min(b.1) && sy < a.1.max(b.1))
                .map(|(a, b)| {
                    let t = (sy - a.1) / (b.1 - a.1);
                    (a.0 + t * (b.0 - a.0), if b.1 > a.1 { 1 } else { -1 })
                })
                .collect();
            xs.sort_by(|p, q| p.0.total_cmp(&q.0));
            let (mut wind, mut start) = (0, 0.0);
            for (x, d) in xs {
                let before = wind;
                wind += d;
                if before == 0 && wind != 0 {
                    start = x;
                } else if before != 0 && wind == 0 {
                    let row = &mut cov[y * n..(y + 1) * n];
                    let (xa, xb) = (start.max(0.0), x.min(n as f32));
                    let mut px = xa.floor() as usize;
                    while (px as f32) < xb && px < n {
                        let overlap = (xb.min(px as f32 + 1.0) - xa.max(px as f32)).max(0.0);
                        row[px] += overlap * 0.25;
                        px += 1;
                    }
                }
            }
        }
    }
    cov
}

/// Paints the app's name under the mark.
fn letter(img: &mut RgbaImage, name: Rgb, accent: Rgb) {
    let soundcore = rasterize(&line(SEMIBOLD, "Soundcore", 560.0, 0.0, 512.0, 678.0));
    let app = rasterize(&line(MEDIUM, "APP", 190.0, 0.32, 512.0, 772.0));
    for (i, p) in img.pixels_mut().enumerate() {
        for (cov, color) in [(soundcore[i], name), (app[i], accent)] {
            let a = cov.clamp(0.0, 1.0);
            if a > 0.0 && p[3] > 0 {
                for c in 0..3 {
                    p[c] = (p[c] as f32 * (1.0 - a) + color[c] * a).round() as u8;
                }
            }
        }
    }
}

fn flow_ground(px: f32, py: f32, dark: bool) -> Rgb {
    if dark {
        let t = (py / 1024.0).clamp(0.0, 1.0);
        let mut g = mix(hex(0x0A1724), hex(0x0D2A3C), t);
        let flows = [
            (940.0, 110.0, 0.0034, 0.6, 120.0, -0.5, 0x4FD8FF, 0.8),
            (1070.0, 130.0, 0.0029, 2.4, 140.0, -0.5, 0x2E9BDC, 0.6),
            (1190.0, 90.0, 0.0041, 4.1, 100.0, -0.5, 0x8AE9FF, 0.4),
            (160.0, 70.0, 0.0050, 1.3, 130.0, -0.5, 0x1C5D86, 0.35),
        ];
        for (base, amp, freq, phase, width, slope, col, k) in flows {
            g = mix(
                g,
                hex(col),
                ribbon(px, py, base, amp, freq, phase, width, slope) * k,
            );
        }
        g
    } else {
        let mut g = hex(0xFAFCFF);
        let flows = [
            (930.0, 110.0, 0.0034, 0.6, 130.0, -0.5, 0x8FE3FF, 0.62),
            (1070.0, 130.0, 0.0029, 2.4, 150.0, -0.5, 0x6CC4F5, 0.45),
            (180.0, 70.0, 0.0050, 1.3, 140.0, -0.5, 0xC2EEFF, 0.55),
        ];
        for (base, amp, freq, phase, width, slope, col, k) in flows {
            g = mix(
                g,
                hex(col),
                ribbon(px, py, base, amp, freq, phase, width, slope) * k,
            );
        }
        g
    }
}

// ---- styles -----------------------------------------------------------------------------

fn shade(style: &str, px: f32, py: f32) -> Rgb {
    match style {
        "aurora-text" => {
            if named_mark(px, py) <= 0.0 {
                hex(0x12161D)
            } else {
                flow_ground(px, py, false)
            }
        }
        "night-text" => {
            if named_mark(px, py) <= 0.0 {
                [250.0, 252.0, 255.0]
            } else {
                flow_ground(px, py, true)
            }
        }
        "dark3d" => {
            // Navy to deep teal, light flowing up from the bottom left.
            let t = (py / 1024.0).clamp(0.0, 1.0);
            let mut g = mix(hex(0x0A1724), hex(0x0D2A3C), t);
            let flows = [
                (920.0, 120.0, 0.0035, 0.6, 110.0, -0.55, 0x4FD8FF, 0.85),
                (1060.0, 140.0, 0.0030, 2.4, 130.0, -0.55, 0x2E9BDC, 0.65),
                (1180.0, 90.0, 0.0040, 4.1, 90.0, -0.55, 0x8AE9FF, 0.45),
            ];
            for (base, amp, freq, phase, width, slope, col, k) in flows {
                g = mix(
                    g,
                    hex(col),
                    ribbon(px, py, base, amp, freq, phase, width, slope) * k,
                );
            }
            sculpted(
                px,
                py,
                g,
                hex(0xE9EEF4),
                hex(0x1C2733),
                hex(0x9FE6FF),
                hex(0x4FD8FF),
            )
        }
        "light3d" => {
            let mut g = hex(0xF6FAFE);
            let flows = [
                (900.0, 120.0, 0.0035, 0.6, 120.0, -0.55, 0x8FE3FF, 0.6),
                (1050.0, 140.0, 0.0030, 2.4, 140.0, -0.55, 0x6CC4F5, 0.45),
                (300.0, 80.0, 0.0040, 4.1, 120.0, -0.55, 0xC9F0FF, 0.5),
            ];
            for (base, amp, freq, phase, width, slope, col, k) in flows {
                g = mix(
                    g,
                    hex(col),
                    ribbon(px, py, base, amp, freq, phase, width, slope) * k,
                );
            }
            sculpted(
                px,
                py,
                g,
                hex(0x2B313B),
                hex(0x161A20),
                hex(0xBFE9FF),
                hex(0x23B8F0),
            )
        }
        "wave" => {
            let t = ((px + py) / 2048.0).clamp(0.0, 1.0);
            let mut c = mix(hex(0x48D6FF), hex(0x1257E8), t);
            let r = ribbon(px, py, 300.0, 60.0, 0.006, 0.8, 140.0, 0.35) * 0.18;
            c = mix(c, [255.0; 3], r);
            let mark = headphones(px, py, 1.0, 30.0).min(bars(px, py));
            if mark <= 0.0 { [255.0; 3] } else { c }
        }
        "night" => {
            let t = (py / 1024.0).clamp(0.0, 1.0);
            let mut c = mix(hex(0x15203A), hex(0x05070D), t);
            let mark = headphones(px, py, 1.0, 30.0);
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
                    ribbon(px, py, base, amp, freq, phase, width, 0.35) * k,
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
            let px = |v: f32| (v / n).clamp(0.0, 255.0) as u8;
            img.put_pixel(x, y, Rgba([px(acc[0]), px(acc[1]), px(acc[2]), a]));
        }
    }
    match style {
        "aurora-text" => letter(&mut img, hex(0x12161D), hex(0x12161D)),
        "night-text" => letter(&mut img, [250.0, 252.0, 255.0], [250.0, 252.0, 255.0]),
        _ => {}
    }
    img
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--sheet") {
        let dir = std::path::PathBuf::from(args.get(1).cloned().unwrap_or_else(|| ".".into()));
        let styles = ["aurora-text", "night-text"];
        let mut sheet = RgbaImage::from_pixel(
            styles.len() as u32 * 560 + 80,
            640,
            Rgba([214, 220, 228, 255]),
        );
        for (i, s) in styles.iter().enumerate() {
            let img = render(s);
            img.save(dir.join(format!("{s}.png"))).expect("write icon");
            let small = imageops::resize(&img, 512, 512, imageops::FilterType::Lanczos3);
            imageops::overlay(&mut sheet, &small, 64 + i as i64 * 560, 64);
        }
        sheet
            .save(dir.join("comparaison.png"))
            .expect("write sheet");
        println!(
            "wrote comparaison.png and {} icons in {}",
            styles.len(),
            dir.display()
        );
        return;
    }
    let out = args
        .first()
        .cloned()
        .unwrap_or_else(|| "icon_1024.png".into());
    let style = args.get(1).cloned().unwrap_or_else(|| "aurora-text".into());
    render(&style).save(&out).expect("write icon");
    println!("wrote {out} ({style})");
}
