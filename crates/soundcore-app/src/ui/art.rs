//! Original vector artwork drawn in code: a generic over-ear headphone illustration and
//! the app's icons. No product photography or brand artwork is used.

use std::f32::consts::{PI, TAU};

use egui::{Color32, CornerRadius, Painter, Pos2, Rect, Shape, Stroke, pos2, vec2};

use super::theme::pal;

fn arc_points(c: Pos2, r: f32, a0: f32, a1: f32, n: usize) -> Vec<Pos2> {
    (0..=n)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / n as f32;
            c + vec2(a.cos(), a.sin()) * r
        })
        .collect()
}

fn ellipse(c: Pos2, rx: f32, ry: f32, n: usize) -> Vec<Pos2> {
    (0..n)
        .map(|i| {
            let a = TAU * i as f32 / n as f32;
            c + vec2(a.cos() * rx, a.sin() * ry)
        })
        .collect()
}

/// Over-ear headphones seen from the front, fitted in `r` (aspect ≈ 1 : 1).
pub fn headphones(p: &Painter, r: Rect, accent: Color32) {
    let pl = pal();
    let s = r.width().min(r.height()) / 200.0;
    let c = r.center();
    let at = |x: f32, y: f32| c + vec2(x * s, y * s);

    // Soft floor shadow.
    p.add(Shape::convex_polygon(
        ellipse(at(0.0, 92.0), 78.0 * s, 7.0 * s, 48),
        Color32::from_black_alpha(if pl.dark { 70 } else { 22 }),
        Stroke::NONE,
    ));

    // Headband: outer shell and inner padding.
    let band_c = at(0.0, 8.0);
    p.add(Shape::line(
        arc_points(band_c, 78.0 * s, PI * 1.03, PI * 1.97, 64),
        Stroke::new(15.0 * s, pl.product),
    ));
    p.add(Shape::line(
        arc_points(band_c, 70.0 * s, PI * 1.12, PI * 1.88, 64),
        Stroke::new(5.0 * s, pl.product_hi),
    ));

    for side in [-1.0f32, 1.0] {
        // Slider and yoke.
        let x = side * 76.0;
        p.line_segment(
            [at(x, 2.0), at(x, 30.0)],
            Stroke::new(8.0 * s, pl.product_hi),
        );
        p.add(Shape::line(
            vec![
                at(x - side * 2.0, 26.0),
                at(x + side * 6.0, 40.0),
                at(x + side * 4.0, 58.0),
            ],
            Stroke::new(5.0 * s, pl.product_hi),
        ));
        // Ear cup, cushion and accent ring.
        let cup = at(x + side * 2.0, 62.0);
        p.add(Shape::convex_polygon(
            ellipse(cup, 27.0 * s, 37.0 * s, 64),
            pl.product,
            Stroke::NONE,
        ));
        p.add(Shape::convex_polygon(
            ellipse(cup + vec2(-side * 9.0 * s, 0.0), 13.0 * s, 32.0 * s, 48),
            pl.product_hi,
            Stroke::NONE,
        ));
        p.add(Shape::closed_line(
            ellipse(cup + vec2(side * 3.0 * s, 0.0), 15.0 * s, 22.0 * s, 48),
            Stroke::new(2.0 * s, accent),
        ));
    }
}

/// Noise cancelling: a wave stopped by a shield-like arc.
pub fn icon_anc(p: &Painter, c: Pos2, size: f32, color: Color32) {
    let s = size / 24.0;
    let st = Stroke::new(1.9 * s, color);
    p.add(Shape::line(
        arc_points(c, 9.0 * s, -PI * 0.75, PI * 0.75, 32),
        st,
    ));
    p.add(Shape::line(
        arc_points(c, 4.5 * s, -PI * 0.75, PI * 0.75, 24),
        st,
    ));
    p.line_segment(
        [c + vec2(-9.0 * s, -9.0 * s), c + vec2(9.0 * s, 9.0 * s)],
        st,
    );
}

/// Normal: a plain note-like dot with a ring.
pub fn icon_normal(p: &Painter, c: Pos2, size: f32, color: Color32) {
    let s = size / 24.0;
    p.circle_stroke(c, 8.5 * s, Stroke::new(1.9 * s, color));
    p.circle_filled(c, 3.0 * s, color);
}

/// Transparency: sound coming in from both sides.
pub fn icon_transparency(p: &Painter, c: Pos2, size: f32, color: Color32) {
    let s = size / 24.0;
    let st = Stroke::new(1.9 * s, color);
    p.circle_filled(c, 2.6 * s, color);
    for (r, a) in [(6.0, 0.32), (10.5, 0.30)] {
        p.add(Shape::line(arc_points(c, r * s, -PI * a, PI * a, 16), st));
        p.add(Shape::line(
            arc_points(c, r * s, PI - PI * a, PI + PI * a, 16),
            st,
        ));
    }
}

/// Horizontal battery with `fraction` filled.
pub fn battery(p: &Painter, r: Rect, fraction: f32, color: Color32, charging: bool) {
    let pl = pal();
    let body = Rect::from_min_max(r.min, pos2(r.right() - 3.0, r.bottom()));
    p.rect_stroke(
        body,
        CornerRadius::same(4),
        Stroke::new(1.6, pl.text_dim),
        egui::StrokeKind::Inside,
    );
    p.rect_filled(
        Rect::from_center_size(
            pos2(r.right() - 1.0, r.center().y),
            vec2(2.5, r.height() * 0.4),
        ),
        CornerRadius::same(1),
        pl.text_dim,
    );
    let inner = body.shrink(3.0);
    let w = inner.width() * fraction.clamp(0.0, 1.0);
    if w > 0.5 {
        p.rect_filled(
            Rect::from_min_size(inner.min, vec2(w, inner.height())),
            CornerRadius::same(2),
            color,
        );
    }
    if charging {
        bolt(
            p,
            body.center(),
            body.height() * 0.95,
            if fraction > 0.45 { pl.surface } else { pl.text },
        );
    }
}

pub fn bolt(p: &Painter, c: Pos2, h: f32, color: Color32) {
    let s = h / 12.0;
    p.add(Shape::convex_polygon(
        vec![
            c + vec2(1.0 * s, -6.0 * s),
            c + vec2(-3.5 * s, 1.0 * s),
            c + vec2(0.0, 1.0 * s),
            c + vec2(-s, 6.0 * s),
            c + vec2(3.5 * s, -s),
            c + vec2(0.0, -s),
        ],
        color,
        Stroke::NONE,
    ));
}

pub fn chevron_down(p: &Painter, c: Pos2, size: f32, color: Color32) {
    let s = size / 2.0;
    p.add(Shape::line(
        vec![
            c + vec2(-s, -s * 0.45),
            c + vec2(0.0, s * 0.45),
            c + vec2(s, -s * 0.45),
        ],
        Stroke::new(1.8, color),
    ));
}

pub fn chevron_right(p: &Painter, c: Pos2, size: f32, color: Color32) {
    let s = size / 2.0;
    p.add(Shape::line(
        vec![
            c + vec2(-s * 0.45, -s),
            c + vec2(s * 0.45, 0.0),
            c + vec2(-s * 0.45, s),
        ],
        Stroke::new(1.8, color),
    ));
}

pub fn warning(p: &Painter, c: Pos2, size: f32, color: Color32) {
    p.circle_filled(c, size / 2.0, color);
    let s = size / 20.0;
    p.line_segment(
        [c + vec2(0.0, -5.0 * s), c + vec2(0.0, 1.5 * s)],
        Stroke::new(2.2 * s, Color32::WHITE),
    );
    p.circle_filled(c + vec2(0.0, 5.0 * s), 1.3 * s, Color32::WHITE);
}

/// Small three-slider glyph for the equalizer.
pub fn icon_eq(p: &Painter, c: Pos2, size: f32, color: Color32) {
    let s = size / 24.0;
    for (i, knob) in [(-1.0f32, -3.0f32), (0.0, 4.0), (1.0, -1.0)] {
        let x = c.x + i * 7.0 * s;
        p.line_segment(
            [pos2(x, c.y - 9.0 * s), pos2(x, c.y + 9.0 * s)],
            Stroke::new(1.6 * s, color.gamma_multiply(0.5)),
        );
        p.circle_filled(pos2(x, c.y + knob * s), 3.0 * s, color);
    }
}

/// The app mark: rounded square with a headphone glyph.
pub fn app_mark(p: &Painter, r: Rect) {
    let pl = pal();
    p.rect_filled(r, CornerRadius::same((r.width() * 0.28) as u8), pl.accent);
    let c = r.center() + vec2(0.0, r.height() * 0.04);
    let s = r.width() / 24.0;
    let st = Stroke::new(2.0 * s, Color32::WHITE);
    p.add(Shape::line(
        arc_points(c, 7.0 * s, PI * 1.05, PI * 1.95, 24),
        st,
    ));
    for side in [-1.0f32, 1.0] {
        p.rect_filled(
            Rect::from_center_size(c + vec2(side * 7.0 * s, 3.0 * s), vec2(3.4 * s, 7.0 * s)),
            CornerRadius::same((1.6 * s) as u8),
            Color32::WHITE,
        );
    }
}

/// Generic earbuds in front of their case, fitted in `r` (original drawing).
pub fn earbuds(p: &Painter, r: Rect, accent: Color32) {
    let pl = pal();
    let s = r.width().min(r.height()) / 200.0;
    let c = r.center();
    let at = |x: f32, y: f32| c + vec2(x * s, y * s);
    p.add(Shape::convex_polygon(
        ellipse(at(0.0, 92.0), 80.0 * s, 7.0 * s, 48),
        Color32::from_black_alpha(if pl.dark { 70 } else { 22 }),
        Stroke::NONE,
    ));
    // Case: rounded pebble with a lid line and a status light.
    let case = Rect::from_center_size(at(0.0, 52.0), vec2(150.0 * s, 74.0 * s));
    p.rect_filled(case, CornerRadius::same((30.0 * s) as u8), pl.product);
    p.hline(
        case.left() + 10.0 * s..=case.right() - 10.0 * s,
        case.top() + 26.0 * s,
        Stroke::new(1.5 * s, pl.product_hi),
    );
    p.circle_filled(at(0.0, 70.0), 3.0 * s, accent);
    // Two buds: round head with a short stem, mirrored.
    for side in [-1.0f32, 1.0] {
        let head = at(side * 38.0, -34.0);
        p.add(Shape::convex_polygon(
            ellipse(head, 26.0 * s, 24.0 * s, 48),
            pl.product,
            Stroke::NONE,
        ));
        p.add(Shape::convex_polygon(
            ellipse(
                head + vec2(side * 8.0 * s, -4.0 * s),
                12.0 * s,
                11.0 * s,
                32,
            ),
            pl.product_hi,
            Stroke::NONE,
        ));
        let stem = Rect::from_min_size(
            head + vec2(-side * 4.0 * s - 6.0 * s, 10.0 * s),
            vec2(12.0 * s, 40.0 * s),
        );
        p.rect_filled(stem, CornerRadius::same((6.0 * s) as u8), pl.product);
        p.circle_stroke(
            head + vec2(-side * 6.0 * s, 2.0 * s),
            9.0 * s,
            Stroke::new(2.0 * s, accent),
        );
    }
}

/// Generic portable speaker (original drawing).
pub fn speaker(p: &Painter, r: Rect, accent: Color32) {
    let pl = pal();
    let s = r.width().min(r.height()) / 200.0;
    let c = r.center();
    let at = |x: f32, y: f32| c + vec2(x * s, y * s);
    p.add(Shape::convex_polygon(
        ellipse(at(0.0, 80.0), 84.0 * s, 7.0 * s, 48),
        Color32::from_black_alpha(if pl.dark { 70 } else { 22 }),
        Stroke::NONE,
    ));
    let body = Rect::from_center_size(at(0.0, 8.0), vec2(176.0 * s, 132.0 * s));
    p.rect_filled(body, CornerRadius::same((40.0 * s) as u8), pl.product);
    for (x, rad) in [(-42.0, 30.0), (42.0, 30.0)] {
        p.circle_filled(at(x, 8.0), rad * s, pl.product_hi);
        p.circle_stroke(at(x, 8.0), rad * s, Stroke::new(2.0 * s, accent));
        p.circle_filled(at(x, 8.0), 9.0 * s, pl.product);
    }
}

/// The illustration matching a product kind.
pub fn product(p: &Painter, kind: soundcore_protocol::Kind, r: Rect, accent: Color32) {
    match kind {
        soundcore_protocol::Kind::Headphones => headphones(p, r, accent),
        soundcore_protocol::Kind::Earbuds => earbuds(p, r, accent),
        soundcore_protocol::Kind::Speaker => speaker(p, r, accent),
    }
}

/// Round "i" badge.
pub fn info(p: &Painter, c: Pos2, size: f32, color: Color32) {
    p.circle_filled(c, size / 2.0, color);
    let s = size / 20.0;
    p.circle_filled(c + vec2(0.0, -5.0 * s), 1.3 * s, Color32::WHITE);
    p.line_segment(
        [c + vec2(0.0, -1.5 * s), c + vec2(0.0, 5.0 * s)],
        Stroke::new(2.2 * s, Color32::WHITE),
    );
}
