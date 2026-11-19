//! Painter-based building blocks shared by every page.

use std::sync::Arc;

use egui::{
    Align2, Color32, CornerRadius, CursorIcon, FontId, Galley, Id, Painter, Pos2, Rect, Response,
    Sense, Stroke, StrokeKind, Ui, pos2, vec2,
};

use super::theme::{self, pal};

/// Set by `SOUNDCORE_CHECK_TEXT=1`: reports text that had to shrink or be cut.
pub fn audit_enabled() -> bool {
    std::env::var_os("SOUNDCORE_CHECK_TEXT").is_some()
}

fn audit(text: &str, need: f32, have: f32) {
    if audit_enabled() {
        eprintln!(
            "TEXT-FIT [{}] \"{text}\" needs {need:.0}, has {have:.0}",
            super::i18n::lang().code()
        );
    }
}

/// Lays `text` out on one line within `max_w`: shrinks down to 80 %, then ends with "…".
pub fn fitted(p: &Painter, text: &str, font: FontId, color: Color32, max_w: f32) -> Arc<Galley> {
    let g = p.layout_no_wrap(text.to_string(), font.clone(), color);
    if g.size().x <= max_w {
        return g;
    }
    let need = g.size().x;
    let mut f = font.clone();
    while f.size > font.size * 0.8 {
        f.size -= 0.5;
        let g = p.layout_no_wrap(text.to_string(), f.clone(), color);
        if g.size().x <= max_w {
            audit(text, need, max_w);
            return g;
        }
    }
    audit(text, need, max_w);
    let chars: Vec<char> = text.chars().collect();
    for n in (1..chars.len()).rev() {
        let s: String = chars[..n].iter().collect::<String>().trim_end().to_string() + "…";
        let g = p.layout_no_wrap(s, f.clone(), color);
        if g.size().x <= max_w {
            return g;
        }
    }
    p.layout_no_wrap("…".into(), f, color)
}

/// One line of text anchored at `pos`, fitted to `max_w`. Returns its rect.
pub fn fit_text(
    p: &Painter,
    pos: Pos2,
    align: Align2,
    text: &str,
    font: FontId,
    max_w: f32,
    color: Color32,
) -> Rect {
    let g = fitted(p, text, font, color, max_w);
    let rect = align.anchor_size(pos, g.size());
    p.galley(rect.min, g, color);
    rect
}

/// Wrapped paragraph; returns its height.
pub fn paragraph(
    p: &Painter,
    top_left: Pos2,
    width: f32,
    text: &str,
    font: FontId,
    color: Color32,
) -> f32 {
    let g = p.layout(text.to_string(), font, color, width);
    let h = g.size().y;
    p.galley(top_left, g, color);
    h
}

pub fn paragraph_height(p: &Painter, width: f32, text: &str, font: FontId) -> f32 {
    p.layout(text.to_string(), font, Color32::WHITE, width)
        .size()
        .y
}

pub fn card(p: &Painter, r: Rect) {
    let pl = pal();
    p.rect(
        r,
        CornerRadius::same(22),
        pl.surface,
        Stroke::new(
            1.0,
            if pl.dark {
                pl.stroke
            } else {
                Color32::TRANSPARENT
            },
        ),
        StrokeKind::Inside,
    );
}

pub fn section_title(p: &Painter, pos: Pos2, text: &str, max_w: f32) -> Rect {
    fit_text(
        p,
        pos,
        Align2::LEFT_TOP,
        text,
        theme::semibold(17.0),
        max_w,
        pal().text,
    )
}

/// Clickable area with a pointer cursor (and nothing when `enabled` is false).
pub fn hit(
    ui: &Ui,
    rect: Rect,
    id: impl std::hash::Hash + std::fmt::Debug,
    enabled: bool,
) -> Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let r = ui.interact(rect, Id::unique(("hit", id)), sense);
    if enabled && r.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    r
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    Primary,
    Secondary,
    Quiet,
    Danger,
}

pub fn button_width(p: &Painter, label: &str, size: f32) -> f32 {
    p.layout_no_wrap(label.to_string(), theme::medium(size), Color32::WHITE)
        .size()
        .x
        + 36.0
}

pub fn button(ui: &Ui, p: &Painter, r: Rect, label: &str, kind: ButtonKind, enabled: bool) -> bool {
    let pl = pal();
    let resp = hit(
        ui,
        r,
        ("btn", label, r.min.x as i32, r.min.y as i32),
        enabled,
    );
    let hovered = resp.hovered() && enabled;
    let (fill, fg) = match kind {
        ButtonKind::Primary => (pl.accent, pl.on_accent),
        ButtonKind::Secondary => (pl.surface_hi, pl.text),
        ButtonKind::Quiet => (Color32::TRANSPARENT, pl.accent),
        ButtonKind::Danger => (
            Color32::from_rgba_unmultiplied(0xE5, 0x48, 0x4D, 30),
            Color32::from_rgb(0xE5, 0x48, 0x4D),
        ),
    };
    let fill = if hovered {
        match kind {
            ButtonKind::Quiet => pl.accent_soft,
            _ => lerp(
                fill,
                if pl.dark {
                    Color32::WHITE
                } else {
                    Color32::BLACK
                },
                0.07,
            ),
        }
    } else {
        fill
    };
    let fg = if enabled { fg } else { fg.gamma_multiply(0.45) };
    p.rect_filled(r, CornerRadius::same((r.height() / 2.0) as u8), fill);
    fit_text(
        p,
        r.center(),
        Align2::CENTER_CENTER,
        label,
        theme::medium(14.0),
        r.width() - 24.0,
        fg,
    );
    resp.clicked()
}

/// Pill-shaped choice used for presets and tabs.
pub fn chip(
    ui: &Ui,
    p: &Painter,
    r: Rect,
    label: &str,
    selected: bool,
    id: impl std::hash::Hash + std::fmt::Debug,
) -> bool {
    let pl = pal();
    let resp = hit(ui, r, ("chip", id), true);
    let (fill, fg, stroke) = if selected {
        (pl.accent, pl.on_accent, Color32::TRANSPARENT)
    } else if resp.hovered() {
        (pl.accent_soft, pl.text, pl.stroke)
    } else {
        (pl.surface, pl.text, pl.stroke)
    };
    p.rect(
        r,
        CornerRadius::same((r.height() / 2.0) as u8),
        fill,
        Stroke::new(1.0, stroke),
        StrokeKind::Inside,
    );
    fit_text(
        p,
        r.center(),
        Align2::CENTER_CENTER,
        label,
        theme::medium(13.0),
        r.width() - 20.0,
        fg,
    );
    resp.clicked()
}

pub fn chip_width(p: &Painter, label: &str) -> f32 {
    p.layout_no_wrap(label.to_string(), theme::medium(13.0), Color32::WHITE)
        .size()
        .x
        + 30.0
}

/// Segmented control; returns the index clicked.
pub fn segmented(
    ui: &Ui,
    p: &Painter,
    r: Rect,
    labels: &[&str],
    selected: usize,
    id: &str,
) -> Option<usize> {
    let pl = pal();
    p.rect_filled(
        r,
        CornerRadius::same((r.height() / 2.0) as u8),
        pl.surface_hi,
    );
    let n = labels.len() as f32;
    let w = (r.width() - 8.0) / n;
    let mut out = None;
    for (i, label) in labels.iter().enumerate() {
        let cell = Rect::from_min_size(
            pos2(r.left() + 4.0 + i as f32 * w, r.top() + 4.0),
            vec2(w, r.height() - 8.0),
        );
        let resp = hit(ui, cell, (id, i), true);
        if i == selected {
            p.rect_filled(
                cell,
                CornerRadius::same((cell.height() / 2.0) as u8),
                pl.surface,
            );
            if !pl.dark {
                p.rect_stroke(
                    cell,
                    CornerRadius::same((cell.height() / 2.0) as u8),
                    Stroke::new(1.0, pl.stroke),
                    StrokeKind::Inside,
                );
            }
        } else if resp.hovered() {
            p.rect_filled(
                cell,
                CornerRadius::same((cell.height() / 2.0) as u8),
                pl.surface.gamma_multiply(0.5),
            );
        }
        let color = if i == selected { pl.text } else { pl.text_dim };
        fit_text(
            p,
            cell.center(),
            Align2::CENTER_CENTER,
            label,
            theme::medium(13.5),
            cell.width() - 14.0,
            color,
        );
        if resp.clicked() {
            out = Some(i);
        }
    }
    out
}

/// Thin row separator.
pub fn divider(p: &Painter, x0: f32, x1: f32, y: f32) {
    p.hline(x0..=x1, y, Stroke::new(1.0, pal().stroke));
}

/// Label on the left, value on the right.
pub fn info_row(p: &Painter, r: Rect, label: &str, value: &str) {
    let pl = pal();
    let lw = (r.width() * 0.45).min(240.0);
    fit_text(
        p,
        pos2(r.left(), r.center().y),
        Align2::LEFT_CENTER,
        label,
        theme::regular(14.0),
        lw,
        pl.text_dim,
    );
    fit_text(
        p,
        pos2(r.right(), r.center().y),
        Align2::RIGHT_CENTER,
        value,
        theme::medium(14.0),
        r.width() - lw - 16.0,
        pl.text,
    );
}

pub fn lerp(a: Color32, b: Color32, t: f32) -> Color32 {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(
        m(a.r(), b.r()),
        m(a.g(), b.g()),
        m(a.b(), b.b()),
        m(a.a(), b.a()),
    )
}
