//! Pages: controls, equalizer, settings and the connection screen.

use egui::{
    Align2, CornerRadius, Id, Painter, Rect, Sense, Shape, Stroke, StrokeKind, Ui, pos2, vec2,
};
use soundcore_protocol::{AmbientMode, Equalizer, eq, types};

use super::app::{Modal, SoundcoreApp, ThemePref, open_bluetooth_settings, open_url, pill};
use super::art;
use super::i18n::{Lang, T, preset_name, t};
use super::theme::{self, pal};
use super::widgets::{self, ButtonKind, card, fit_text, paragraph, paragraph_height};
use crate::session::{Action, Phase, Snapshot, Target};

const GAP: f32 = 20.0;
const PAD: f32 = 28.0;

fn mode_label(m: AmbientMode) -> &'static str {
    match m {
        AmbientMode::NoiseCanceling => t(T::ModeAnc),
        AmbientMode::Normal => t(T::ModeNormal),
        AmbientMode::Transparency => t(T::ModeTransparency),
    }
}

fn mode_desc(m: AmbientMode) -> &'static str {
    match m {
        AmbientMode::NoiseCanceling => t(T::DescAnc),
        AmbientMode::Normal => t(T::DescNormal),
        AmbientMode::Transparency => t(T::DescTransparency),
    }
}

fn mode_icon(p: &Painter, m: AmbientMode, c: egui::Pos2, size: f32, color: egui::Color32) {
    match m {
        AmbientMode::NoiseCanceling => art::icon_anc(p, c, size, color),
        AmbientMode::Normal => art::icon_normal(p, c, size, color),
        AmbientMode::Transparency => art::icon_transparency(p, c, size, color),
    }
}

/// Name of the equalizer setting currently on the headphones.
fn eq_name(e: &Equalizer) -> &'static str {
    if e.is_custom() {
        return t(T::Custom);
    }
    eq::by_id(e.preset)
        .map(|p| preset_name(p.key))
        .unwrap_or(t(T::Custom))
}

/// Gains → points of a smooth curve across `r` (±6 dB full scale).
fn curve_points(r: Rect, gains: &[i16; 8]) -> Vec<egui::Pos2> {
    let xs: Vec<f32> = (0..8)
        .map(|i| r.left() + r.width() * (i as f32 + 0.5) / 8.0)
        .collect();
    let y = |g: i16| r.center().y - (g as f32 / 60.0).clamp(-1.0, 1.0) * r.height() / 2.0;
    let pts: Vec<egui::Pos2> = xs.iter().zip(gains).map(|(x, g)| pos2(*x, y(*g))).collect();
    // Catmull-Rom through the band points, flat beyond the ends.
    let mut out = vec![pos2(r.left(), pts[0].y)];
    for i in 0..pts.len() - 1 {
        let p0 = if i == 0 { pts[0] } else { pts[i - 1] };
        let (p1, p2) = (pts[i], pts[i + 1]);
        let p3 = if i + 2 < pts.len() {
            pts[i + 2]
        } else {
            pts[i + 1]
        };
        for k in 0..12 {
            let s = k as f32 / 12.0;
            let s2 = s * s;
            let s3 = s2 * s;
            let f = |a: f32, b: f32, c: f32, d: f32| {
                0.5 * ((2.0 * b)
                    + (-a + c) * s
                    + (2.0 * a - 5.0 * b + 4.0 * c - d) * s2
                    + (-a + 3.0 * b - 3.0 * c + d) * s3)
            };
            out.push(pos2(f(p0.x, p1.x, p2.x, p3.x), f(p0.y, p1.y, p2.y, p3.y)));
        }
    }
    out.push(*pts.last().expect("points"));
    out.push(pos2(r.right(), pts[7].y));
    out
}

fn draw_curve(p: &Painter, r: Rect, gains: &[i16; 8], fill: bool) {
    let pl = pal();
    let pts = curve_points(r, gains);
    if fill {
        let base = r.center().y;
        for w in pts.windows(2) {
            let quad = vec![w[0], w[1], pos2(w[1].x, base), pos2(w[0].x, base)];
            p.add(Shape::convex_polygon(
                quad,
                pl.accent.gamma_multiply(0.10),
                Stroke::NONE,
            ));
        }
    }
    p.add(Shape::line(pts, Stroke::new(2.4, pl.accent)));
}

impl SoundcoreApp {
    // ---------------------------------------------------------------------------------
    // status
    // ---------------------------------------------------------------------------------

    /// Card shown while the headphones are not ready. Returns its bottom.
    #[allow(clippy::too_many_arguments)]
    fn status_card(
        &mut self,
        ui: &Ui,
        p: &Painter,
        x0: f32,
        cw: f32,
        y: f32,
        snap: &Snapshot,
        now: f64,
    ) -> f32 {
        let pl = pal();
        let (title, hint) = match &snap.phase {
            Phase::Ready | Phase::Idle => return y,
            Phase::Waiting => (t(T::PhaseWaiting), Some(t(T::HintWaiting))),
            Phase::Connecting => (t(T::PhaseConnecting), None),
            Phase::Loading => (t(T::PhaseLoading), None),
            Phase::Busy => (t(T::PhaseBusy), Some(t(T::HintBusy))),
            Phase::Failed(_) => (t(T::PhaseFailed), Some(t(T::HintWaiting))),
        };
        let text_w = cw - 2.0 * PAD - 44.0;
        let hint_h = hint
            .map(|h| paragraph_height(p, text_w, h, theme::regular(13.5)))
            .unwrap_or(0.0);
        let h = 64.0 + hint_h + if hint.is_some() { 64.0 } else { 0.0 };
        let r = Rect::from_min_size(pos2(x0, y), vec2(cw, h));
        card(p, r);
        // Spinner or warning.
        let c = pos2(r.left() + PAD + 12.0, r.top() + 34.0);
        if matches!(snap.phase, Phase::Busy | Phase::Failed(_)) {
            art::warning(p, c, 22.0, pl.warn);
        } else {
            let a0 = (now * 4.0) as f32;
            let pts: Vec<_> = (0..=24)
                .map(|i| {
                    let a = a0 + i as f32 / 24.0 * 4.4;
                    c + vec2(a.cos(), a.sin()) * 9.0
                })
                .collect();
            p.add(Shape::line(pts, Stroke::new(2.4, pl.accent)));
        }
        fit_text(
            p,
            pos2(r.left() + PAD + 36.0, r.top() + 34.0),
            Align2::LEFT_CENTER,
            title,
            theme::semibold(16.0),
            text_w,
            pl.text,
        );
        if let Some(hint) = hint {
            paragraph(
                p,
                pos2(r.left() + PAD + 36.0, r.top() + 58.0),
                text_w,
                hint,
                theme::regular(13.5),
                pl.text_dim,
            );
            let by = r.bottom() - 56.0;
            let retry = t(T::Retry);
            let bw = widgets::button_width(p, retry, 14.0);
            if widgets::button(
                ui,
                p,
                Rect::from_min_size(pos2(r.left() + PAD + 36.0, by), vec2(bw, 38.0)),
                retry,
                ButtonKind::Primary,
                true,
            ) {
                self.act(Action::Refresh);
            }
            let other = t(T::MyDevices);
            let ow = widgets::button_width(p, other, 14.0);
            if widgets::button(
                ui,
                p,
                Rect::from_min_size(pos2(r.left() + PAD + 48.0 + bw, by), vec2(ow, 38.0)),
                other,
                ButtonKind::Secondary,
                true,
            ) {
                self.modal = Modal::Devices;
            }
        }
        r.bottom() + GAP
    }

    // ---------------------------------------------------------------------------------
    // controls
    // ---------------------------------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    pub(super) fn page_controls(
        &mut self,
        ui: &Ui,
        p: &Painter,
        x0: f32,
        cw: f32,
        top: f32,
        snap: &Snapshot,
        now: f64,
    ) -> f32 {
        let pl = pal();
        let st = &snap.state;
        let ready = snap.phase == Phase::Ready;
        let mut y = self.status_card(ui, p, x0, cw, top, snap, now);

        // Model this version recognises but cannot drive.
        if st.model.is_some_and(|m| m.caps.is_none()) {
            let body_h = paragraph_height(
                p,
                cw - 2.0 * PAD - 40.0,
                t(T::UnsupportedBody),
                theme::regular(13.0),
            );
            let r = Rect::from_min_size(pos2(x0, y), vec2(cw, body_h + 60.0));
            p.rect_filled(r, CornerRadius::same(22), pl.warn.gamma_multiply(0.14));
            art::warning(
                p,
                pos2(r.left() + PAD + 10.0, r.top() + 30.0),
                20.0,
                pl.warn,
            );
            fit_text(
                p,
                pos2(r.left() + PAD + 34.0, r.top() + 30.0),
                Align2::LEFT_CENTER,
                t(T::UnsupportedTitle),
                theme::semibold(15.0),
                cw - 2.0 * PAD - 40.0,
                pl.text,
            );
            paragraph(
                p,
                pos2(r.left() + PAD + 34.0, r.top() + 46.0),
                cw - 2.0 * PAD - 40.0,
                t(T::UnsupportedBody),
                theme::regular(13.0),
                pl.text_dim,
            );
            y = r.bottom() + GAP;
        }

        // Hero: illustration, name, battery.
        let hero = Rect::from_min_size(pos2(x0, y), vec2(cw, 236.0));
        card(p, hero);
        let art_r = Rect::from_min_size(hero.left_top() + vec2(24.0, 18.0), vec2(200.0, 200.0));
        p.circle_filled(art_r.center(), 92.0, pl.accent_soft);
        art::headphones(p, art_r.shrink(14.0), pl.accent);
        let tx = art_r.right() + 24.0;
        let tw = hero.right() - PAD - tx;
        let name = snap
            .target
            .as_ref()
            .map(|t| t.name.as_str())
            .unwrap_or("soundcore");
        fit_text(
            p,
            pos2(tx, hero.top() + 52.0),
            Align2::LEFT_CENTER,
            name,
            theme::semibold(26.0),
            tw,
            pl.text,
        );
        let mut sub = st.model.map(|m| m.id.to_string()).unwrap_or_default();
        if let Some(fw) = &st.firmware {
            if !sub.is_empty() {
                sub.push_str(" · ");
            }
            sub.push_str(&format!("{} {fw}", t(T::Firmware)));
        }
        fit_text(
            p,
            pos2(tx, hero.top() + 84.0),
            Align2::LEFT_CENTER,
            &sub,
            theme::regular(13.5),
            tw,
            pl.text_dim,
        );
        // Battery.
        let by = hero.top() + 136.0;
        fit_text(
            p,
            pos2(tx, by - 22.0),
            Align2::LEFT_CENTER,
            &t(T::Battery).to_uppercase(),
            theme::medium(11.5),
            tw,
            pl.text_faint,
        );
        match st.battery {
            Some(b) if ready => {
                let pct = b.percent();
                let color = if b.charging {
                    pl.good
                } else if pct <= 20 {
                    pl.warn
                } else {
                    pl.text
                };
                art::battery(
                    p,
                    Rect::from_min_size(pos2(tx, by - 9.0), vec2(40.0, 19.0)),
                    pct as f32 / 100.0,
                    color,
                    b.charging,
                );
                let r = fit_text(
                    p,
                    pos2(tx + 52.0, by),
                    Align2::LEFT_CENTER,
                    &format!("{pct} %"),
                    theme::semibold(22.0),
                    90.0,
                    pl.text,
                );
                if b.charging {
                    pill(
                        p,
                        pos2(r.right() + 12.0, by - 12.0),
                        t(T::Charging),
                        pl.good,
                        pl.good.gamma_multiply(0.14),
                    );
                }
            }
            _ => {
                fit_text(
                    p,
                    pos2(tx, by),
                    Align2::LEFT_CENTER,
                    "—",
                    theme::semibold(22.0),
                    60.0,
                    pl.text_faint,
                );
            }
        }
        // Connection status.
        let (label, color) = if ready {
            (t(T::Connected), pl.good)
        } else {
            (t(T::NotConnected), pl.text_faint)
        };
        let sr = Rect::from_min_size(pos2(tx, hero.bottom() - 54.0), vec2(0.0, 26.0));
        p.circle_filled(pos2(sr.left() + 5.0, sr.center().y), 4.5, color);
        fit_text(
            p,
            pos2(sr.left() + 16.0, sr.center().y),
            Align2::LEFT_CENTER,
            label,
            theme::medium(13.0),
            tw - 16.0,
            pl.text_dim,
        );
        y = hero.bottom() + GAP;

        // Noise control.
        let current = st.sound_modes.map(|m| m.ambient);
        let modes = st
            .model
            .and_then(|m| m.caps)
            .map(|c| c.ambient_modes.to_vec())
            .unwrap_or_else(|| AmbientMode::ALL.to_vec());
        let desc = current.map(mode_desc).unwrap_or("");
        let desc_h = paragraph_height(p, cw - 2.0 * PAD, desc, theme::regular(13.5));
        let nc = Rect::from_min_size(pos2(x0, y), vec2(cw, 66.0 + 128.0 + 16.0 + desc_h + PAD));
        card(p, nc);
        widgets::section_title(
            p,
            nc.left_top() + vec2(PAD, PAD),
            t(T::NoiseControl),
            cw - 2.0 * PAD,
        );
        let n = modes.len() as f32;
        let tile_w = (cw - 2.0 * PAD - (n - 1.0) * 14.0) / n;
        for (i, m) in modes.iter().enumerate() {
            let r = Rect::from_min_size(
                pos2(
                    nc.left() + PAD + i as f32 * (tile_w + 14.0),
                    nc.top() + 66.0,
                ),
                vec2(tile_w, 128.0),
            );
            let selected = current == Some(*m);
            let resp = widgets::hit(ui, r, ("mode", i), ready);
            let (fill, stroke) = if selected {
                (pl.accent_soft, pl.accent)
            } else if resp.hovered() {
                (pl.surface_hi, pl.stroke)
            } else {
                (pl.surface, pl.stroke)
            };
            p.rect(
                r,
                CornerRadius::same(18),
                fill,
                Stroke::new(if selected { 1.6 } else { 1.0 }, stroke),
                StrokeKind::Inside,
            );
            let c = pos2(r.center().x, r.top() + 46.0);
            p.circle_filled(c, 25.0, if selected { pl.accent } else { pl.surface_hi });
            mode_icon(
                p,
                *m,
                c,
                26.0,
                if selected { pl.on_accent } else { pl.text_dim },
            );
            fit_text(
                p,
                pos2(r.center().x, r.top() + 98.0),
                Align2::CENTER_CENTER,
                mode_label(*m),
                theme::medium(14.0),
                r.width() - 16.0,
                if selected { pl.text } else { pl.text_dim },
            );
            if resp.clicked() && !selected {
                self.act(Action::SetAmbient(*m));
            }
        }
        paragraph(
            p,
            pos2(nc.left() + PAD, nc.top() + 66.0 + 128.0 + 16.0),
            cw - 2.0 * PAD,
            desc,
            theme::regular(13.5),
            pl.text_dim,
        );
        y = nc.bottom() + GAP;

        // Equalizer summary + tips on the headphones.
        let two = cw >= 620.0;
        let half = if two { (cw - GAP) / 2.0 } else { cw };
        let tips = [t(T::TipModeButton), t(T::TipVoiceAssistant), t(T::TipLed)];
        let tip_w = half - 2.0 * PAD - 18.0;
        let tips_h: f32 = tips
            .iter()
            .map(|s| paragraph_height(p, tip_w, s, theme::regular(13.0)) + 12.0)
            .sum();
        let eq_h: f32 = 210.0;
        let tips_card_h = 66.0 + tips_h + PAD - 12.0;
        let row_h = if two { eq_h.max(tips_card_h) } else { eq_h };

        let eq_r = Rect::from_min_size(pos2(x0, y), vec2(half, row_h));
        card(p, eq_r);
        let resp = widgets::hit(ui, eq_r, "eq-card", true);
        widgets::section_title(
            p,
            eq_r.left_top() + vec2(PAD, PAD),
            t(T::TabEqualizer),
            half - 2.0 * PAD - 30.0,
        );
        art::chevron_right(
            p,
            pos2(eq_r.right() - PAD - 4.0, eq_r.top() + PAD + 11.0),
            12.0,
            pl.text_dim,
        );
        let e = st.equalizer.unwrap_or_default();
        fit_text(
            p,
            eq_r.left_top() + vec2(PAD, PAD + 30.0),
            Align2::LEFT_TOP,
            eq_name(&e),
            theme::medium(14.0),
            half - 2.0 * PAD,
            pl.accent,
        );
        let graph = Rect::from_min_max(
            eq_r.left_top() + vec2(PAD, PAD + 66.0),
            eq_r.right_bottom() - vec2(PAD, PAD),
        );
        p.hline(
            graph.left()..=graph.right(),
            graph.center().y,
            Stroke::new(1.0, pl.stroke),
        );
        draw_curve(p, graph, &e.gains(), true);
        if resp.clicked() {
            self.tab = super::app::Tab::Equalizer;
        }

        let (tx0, ty0) = if two {
            (x0 + half + GAP, y)
        } else {
            (x0, eq_r.bottom() + GAP)
        };
        let tips_r = Rect::from_min_size(
            pos2(tx0, ty0),
            vec2(half, if two { row_h } else { tips_card_h }),
        );
        card(p, tips_r);
        widgets::section_title(
            p,
            tips_r.left_top() + vec2(PAD, PAD),
            t(T::OnYourHeadphones),
            half - 2.0 * PAD,
        );
        let mut ty = tips_r.top() + 66.0;
        for tip in tips {
            p.circle_filled(pos2(tips_r.left() + PAD + 4.0, ty + 10.0), 3.0, pl.accent);
            ty += paragraph(
                p,
                pos2(tips_r.left() + PAD + 18.0, ty),
                tip_w,
                tip,
                theme::regular(13.0),
                pl.text_dim,
            ) + 12.0;
        }
        tips_r.bottom()
    }

    // ---------------------------------------------------------------------------------
    // equalizer
    // ---------------------------------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    pub(super) fn page_equalizer(
        &mut self,
        ui: &Ui,
        p: &Painter,
        x0: f32,
        cw: f32,
        top: f32,
        snap: &Snapshot,
        now: f64,
    ) -> f32 {
        let pl = pal();
        let ready = snap.phase == Phase::Ready;
        let mut y = self.status_card(ui, p, x0, cw, top, snap, now);
        let current = snap.state.equalizer.unwrap_or_default();

        // Curve editor first: it is what people come for.
        let editor = Rect::from_min_size(pos2(x0, y), vec2(cw, 400.0));
        card(p, editor);
        widgets::section_title(
            p,
            editor.left_top() + vec2(PAD, PAD),
            t(T::YourCurve),
            cw - 2.0 * PAD - 150.0,
        );
        fit_text(
            p,
            editor.left_top() + vec2(PAD, PAD + 28.0),
            Align2::LEFT_TOP,
            eq_name(&current),
            theme::medium(14.0),
            cw - 2.0 * PAD - 150.0,
            pl.accent,
        );
        if current.is_custom() || self.eq_draft.is_some() {
            let rw = widgets::button_width(p, t(T::Reset), 14.0);
            let rr = Rect::from_min_size(
                pos2(editor.right() - PAD - rw, editor.top() + PAD),
                vec2(rw, 36.0),
            );
            if widgets::button(ui, p, rr, t(T::Reset), ButtonKind::Secondary, ready) {
                self.custom_eq = [0; 8];
                self.eq_draft = None;
                self.act(Action::SetEqualizer(Equalizer::custom([0; 8])));
            }
        }
        let graph = Rect::from_min_max(
            editor.left_top() + vec2(PAD + 34.0, PAD + 86.0),
            editor.right_bottom() - vec2(PAD + 8.0, PAD + 70.0),
        );
        // Grid: +6, 0, −6 dB.
        for (g, label) in [(60, "+6"), (30, ""), (0, "0"), (-30, ""), (-60, "−6")] {
            let gy = graph.center().y - g as f32 / 60.0 * graph.height() / 2.0;
            let stroke = if g == 0 {
                Stroke::new(1.2, pl.stroke)
            } else {
                Stroke::new(1.0, pl.stroke.gamma_multiply(0.6))
            };
            p.hline(graph.left()..=graph.right(), gy, stroke);
            if !label.is_empty() {
                fit_text(
                    p,
                    pos2(graph.left() - 12.0, gy),
                    Align2::RIGHT_CENTER,
                    label,
                    theme::regular(11.5),
                    30.0,
                    pl.text_faint,
                );
            }
        }
        let gains = self.eq_draft.unwrap_or_else(|| current.gains());
        draw_curve(p, graph, &gains, true);
        // Handles.
        let mut new_gains = gains;
        let mut changed = false;
        for i in 0..8 {
            let x = graph.left() + graph.width() * (i as f32 + 0.5) / 8.0;
            let gy = graph.center().y - gains[i] as f32 / 60.0 * graph.height() / 2.0;
            let column = Rect::from_min_max(
                pos2(x - 22.0, graph.top() - 14.0),
                pos2(x + 22.0, graph.bottom() + 14.0),
            );
            let resp = ui.interact(
                column,
                Id::unique(("eq-band", i)),
                if ready {
                    Sense::click_and_drag()
                } else {
                    Sense::hover()
                },
            );
            if ready && resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
            }
            let active = resp.dragged() || self.eq_drag == Some(i);
            if (resp.dragged() || resp.clicked())
                && let Some(pos) = resp.interact_pointer_pos()
            {
                let v =
                    ((graph.center().y - pos.y) / (graph.height() / 2.0) * 60.0).clamp(-60.0, 60.0);
                let v = ((v / 5.0).round() * 5.0) as i16;
                if v != new_gains[i] {
                    new_gains[i] = v;
                    changed = true;
                }
                self.eq_drag = Some(i);
            }
            if resp.drag_stopped() || (resp.clicked() && !resp.dragged()) {
                self.eq_drag = None;
                changed = true;
            }
            p.circle_filled(pos2(x, gy), if active { 10.0 } else { 8.0 }, pl.surface);
            p.circle_stroke(
                pos2(x, gy),
                if active { 10.0 } else { 8.0 },
                Stroke::new(2.4, pl.accent),
            );
            if active || resp.hovered() {
                let db = gains[i] as f32 / 10.0;
                let label = if db > 0.0 {
                    format!("+{db:.1}")
                } else {
                    format!("{db:.1}")
                };
                pill(
                    p,
                    pos2(x - 22.0, gy - 40.0),
                    &label,
                    pl.on_accent,
                    pl.accent,
                );
            }
            let hz = types::BAND_HZ[i];
            let label = if hz >= 1000 {
                format!("{}k", hz as f32 / 1000.0)
            } else {
                hz.to_string()
            };
            fit_text(
                p,
                pos2(x, graph.bottom() + 24.0),
                Align2::CENTER_CENTER,
                &label,
                theme::regular(12.0),
                44.0,
                pl.text_faint,
            );
        }
        if changed && ready {
            self.eq_draft = Some(new_gains);
            self.custom_eq = new_gains;
            // Send while dragging (throttled) and once released.
            if self.eq_drag.is_none() || now - self.eq_sent_at > 0.25 {
                self.eq_sent_at = now;
                self.act(Action::SetEqualizer(Equalizer::custom(new_gains)));
            }
        }
        if self.eq_drag.is_none()
            && self.eq_draft.is_some()
            && current.is_custom()
            && current.gains() == self.eq_draft.unwrap_or_default()
        {
            self.eq_draft = None;
        }
        paragraph(
            p,
            pos2(editor.left() + PAD, editor.bottom() - PAD - 22.0),
            cw - 2.0 * PAD,
            t(T::CustomHint),
            theme::regular(12.5),
            pl.text_faint,
        );
        y = editor.bottom() + GAP;

        // Presets.
        let mut items: Vec<(String, Option<u16>)> = eq::PRESETS
            .iter()
            .map(|p| (preset_name(p.key).to_string(), Some(p.id)))
            .collect();
        items.push((t(T::Custom).to_string(), None));
        let inner_w = cw - 2.0 * PAD;
        // Flow layout.
        let mut rows: Vec<Vec<(usize, f32)>> = vec![Vec::new()];
        let mut line_w = 0.0;
        for (i, (label, _)) in items.iter().enumerate() {
            let w = widgets::chip_width(p, label).min(inner_w);
            if line_w > 0.0 && line_w + 10.0 + w > inner_w {
                rows.push(Vec::new());
                line_w = 0.0;
            }
            line_w += if line_w > 0.0 { 10.0 } else { 0.0 } + w;
            rows.last_mut().expect("row").push((i, w));
        }
        let sub_h = paragraph_height(p, inner_w, t(T::EqSubtitle), theme::regular(13.0));
        let presets_r = Rect::from_min_size(
            pos2(x0, y),
            vec2(
                cw,
                PAD + 30.0 + sub_h + 18.0 + rows.len() as f32 * 46.0 + PAD - 10.0,
            ),
        );
        card(p, presets_r);
        widgets::section_title(
            p,
            presets_r.left_top() + vec2(PAD, PAD),
            t(T::Presets),
            inner_w,
        );
        paragraph(
            p,
            presets_r.left_top() + vec2(PAD, PAD + 30.0),
            inner_w,
            t(T::EqSubtitle),
            theme::regular(13.0),
            pl.text_dim,
        );
        let mut cy = presets_r.top() + PAD + 30.0 + sub_h + 18.0;
        for row in rows {
            let mut cx = presets_r.left() + PAD;
            for (i, w) in row {
                let (label, id) = &items[i];
                let selected = match id {
                    Some(id) => !current.is_custom() && current.preset == *id,
                    None => current.is_custom(),
                };
                let r = Rect::from_min_size(pos2(cx, cy), vec2(w, 36.0));
                if widgets::chip(ui, p, r, label, selected, ("preset", i)) && ready && !selected {
                    self.eq_draft = None;
                    let e = match id {
                        Some(id) => Equalizer::preset(*id).unwrap_or_default(),
                        None => Equalizer::custom(self.custom_eq),
                    };
                    self.act(Action::SetEqualizer(e));
                }
                cx += w + 10.0;
            }
            cy += 46.0;
        }
        presets_r.bottom()
    }

    // ---------------------------------------------------------------------------------
    // settings
    // ---------------------------------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    pub(super) fn page_settings(
        &mut self,
        ui: &Ui,
        p: &Painter,
        x0: f32,
        cw: f32,
        top: f32,
        snap: &Snapshot,
        now: f64,
    ) -> f32 {
        let pl = pal();
        let st = &snap.state;
        let mut y = top;
        let inner = cw - 2.0 * PAD;
        let dash = |o: Option<String>| o.unwrap_or_else(|| "—".into());

        // Device.
        let rows = [
            (
                t(T::DeviceName),
                snap.target.as_ref().map(|t| t.name.clone()),
            ),
            (
                t(T::Model),
                st.model.map(|m| format!("{} ({})", m.name, m.id)),
            ),
            (t(T::Firmware), st.firmware.clone()),
            (t(T::Serial), st.serial.clone()),
            (
                t(T::Address),
                snap.target
                    .as_ref()
                    .map(|t| t.address.replace('-', ":").to_uppercase()),
            ),
        ];
        let dev = Rect::from_min_size(
            pos2(x0, y),
            vec2(
                cw,
                PAD + 40.0 + rows.len() as f32 * 48.0 + 20.0 + 40.0 + PAD,
            ),
        );
        card(p, dev);
        widgets::section_title(p, dev.left_top() + vec2(PAD, PAD), t(T::SecDevice), inner);
        let mut ry = dev.top() + PAD + 40.0;
        for (i, (label, value)) in rows.into_iter().enumerate() {
            widgets::info_row(
                p,
                Rect::from_min_size(pos2(dev.left() + PAD, ry), vec2(inner, 48.0)),
                label,
                &dash(value),
            );
            ry += 48.0;
            if i < 4 {
                widgets::divider(p, dev.left() + PAD, dev.right() - PAD, ry);
            }
        }
        let by = dev.bottom() - PAD - 40.0;
        let mut bx = dev.left() + PAD;
        for (label, kind, action) in [
            (t(T::Refresh), ButtonKind::Secondary, 0),
            (t(T::ViewLog), ButtonKind::Secondary, 1),
            (t(T::Disconnect), ButtonKind::Danger, 2),
        ] {
            if action == 2 && self.demo {
                continue;
            }
            let w = widgets::button_width(p, label, 14.0);
            if widgets::button(
                ui,
                p,
                Rect::from_min_size(pos2(bx, by), vec2(w, 40.0)),
                label,
                kind,
                true,
            ) {
                match action {
                    0 => self.act(Action::Refresh),
                    1 => self.modal = Modal::Log,
                    _ => self.disconnect(),
                }
            }
            bx += w + 12.0;
        }
        y = dev.bottom() + GAP;

        // App.
        let app = Rect::from_min_size(pos2(x0, y), vec2(cw, PAD + 40.0 + 56.0 + 56.0 + PAD - 8.0));
        card(p, app);
        widgets::section_title(p, app.left_top() + vec2(PAD, PAD), t(T::SecApp), inner);
        let lang_row = Rect::from_min_size(
            pos2(app.left() + PAD, app.top() + PAD + 40.0),
            vec2(inner, 56.0),
        );
        let resp = widgets::hit(ui, lang_row, "lang-row", true);
        if resp.hovered() {
            p.rect_filled(
                lang_row.expand2(vec2(10.0, 0.0)),
                CornerRadius::same(12),
                pl.surface_hi,
            );
        }
        let value = match self.lang_pref {
            Some(l) => l.native_name().to_string(),
            None => format!(
                "{} · {}",
                t(T::SystemLanguage),
                Lang::system().native_name()
            ),
        };
        widgets::info_row(
            p,
            lang_row.with_max_x(lang_row.right() - 20.0),
            t(T::Language),
            &value,
        );
        art::chevron_right(
            p,
            pos2(lang_row.right() - 4.0, lang_row.center().y),
            11.0,
            pl.text_dim,
        );
        if resp.clicked() {
            self.modal = Modal::Language;
        }
        widgets::divider(p, app.left() + PAD, app.right() - PAD, lang_row.bottom());
        let ar = Rect::from_min_size(pos2(app.left() + PAD, lang_row.bottom()), vec2(inner, 56.0));
        fit_text(
            p,
            pos2(ar.left(), ar.center().y),
            Align2::LEFT_CENTER,
            t(T::Appearance),
            theme::regular(14.0),
            inner * 0.4,
            pl.text_dim,
        );
        let seg_w = 300.0f32.min(inner * 0.6);
        let seg = Rect::from_min_size(
            pos2(ar.right() - seg_w, ar.center().y - 19.0),
            vec2(seg_w, 38.0),
        );
        let selected = match self.theme_pref {
            ThemePref::System => 0,
            ThemePref::Light => 1,
            ThemePref::Dark => 2,
        };
        if let Some(i) = widgets::segmented(
            ui,
            p,
            seg,
            &[t(T::ThemeSystem), t(T::ThemeLight), t(T::ThemeDark)],
            selected,
            "theme",
        ) {
            self.theme_pref = [ThemePref::System, ThemePref::Light, ThemePref::Dark][i];
        }
        y = app.bottom() + GAP;

        // About.
        let texts = [t(T::Disclaimer), t(T::Credits), t(T::Trademarks)];
        let texts_h: f32 = texts
            .iter()
            .map(|s| paragraph_height(p, inner, s, theme::regular(13.0)) + 10.0)
            .sum();
        let about = Rect::from_min_size(
            pos2(x0, y),
            vec2(cw, PAD + 40.0 + 30.0 + texts_h + 10.0 + 40.0 + PAD),
        );
        card(p, about);
        widgets::section_title(p, about.left_top() + vec2(PAD, PAD), t(T::SecAbout), inner);
        let mut ay = about.top() + PAD + 40.0;
        fit_text(
            p,
            pos2(about.left() + PAD, ay),
            Align2::LEFT_TOP,
            &format!(
                "{} · {} {}",
                super::app::APP_NAME,
                t(T::Version),
                env!("CARGO_PKG_VERSION")
            ),
            theme::medium(14.0),
            inner,
            pl.text,
        );
        ay += 30.0;
        for (i, s) in texts.iter().enumerate() {
            let color = if i == 0 { pl.text } else { pl.text_dim };
            ay += paragraph(
                p,
                pos2(about.left() + PAD, ay),
                inner,
                s,
                theme::regular(13.0),
                color,
            ) + 10.0;
        }
        let label = "OpenSCQ30";
        let w = widgets::button_width(p, label, 14.0);
        if widgets::button(
            ui,
            p,
            Rect::from_min_size(
                pos2(about.left() + PAD, about.bottom() - PAD - 40.0),
                vec2(w, 40.0),
            ),
            label,
            ButtonKind::Secondary,
            true,
        ) {
            open_url("https://github.com/Oppzippy/OpenSCQ30");
        }
        let _ = now;
        about.bottom()
    }

    // ---------------------------------------------------------------------------------
    // connection screen
    // ---------------------------------------------------------------------------------

    pub(super) fn connect_screen(
        &mut self,
        ui: &Ui,
        p: &Painter,
        rect: Rect,
        _snap: &Snapshot,
        now: f64,
    ) -> f32 {
        let pl = pal();
        self.refresh_paired(now);
        let cw = 540.0f32.min(rect.width() - 48.0);
        let x0 = rect.center().x - cw / 2.0;
        let mut y = rect.top() + 24.0;
        let art_r = Rect::from_center_size(pos2(rect.center().x, y + 96.0), vec2(190.0, 190.0));
        p.circle_filled(art_r.center(), 92.0, pl.accent_soft);
        art::headphones(p, art_r.shrink(12.0), pl.accent);
        y = art_r.bottom() + 22.0;
        fit_text(
            p,
            pos2(rect.center().x, y),
            Align2::CENTER_TOP,
            super::app::APP_NAME,
            theme::semibold(30.0),
            cw,
            pl.text,
        );
        y += 46.0;
        let h = paragraph_height(p, cw, t(T::Tagline), theme::regular(15.0));
        let g = p.layout(
            t(T::Tagline).to_string(),
            theme::regular(15.0),
            pl.text_dim,
            cw,
        );
        p.galley(pos2(rect.center().x - g.size().x / 2.0, y), g, pl.text_dim);
        y += h + 30.0;

        // Devices.
        fit_text(
            p,
            pos2(x0 + 4.0, y),
            Align2::LEFT_TOP,
            &t(T::MyDevices).to_uppercase(),
            theme::medium(12.0),
            cw,
            pl.text_faint,
        );
        y += 26.0;
        let devices = self.visible_devices();
        let list_h = if devices.is_empty() {
            paragraph_height(p, cw - 2.0 * PAD, t(T::NoDevices), theme::regular(13.5)) + 2.0 * 22.0
        } else {
            devices.len() as f32 * 72.0
        };
        let list = Rect::from_min_size(pos2(x0, y), vec2(cw, list_h));
        card(p, list);
        if devices.is_empty() {
            paragraph(
                p,
                list.left_top() + vec2(PAD, 22.0),
                cw - 2.0 * PAD,
                t(T::NoDevices),
                theme::regular(13.5),
                pl.text_dim,
            );
        }
        for (i, d) in devices.iter().enumerate() {
            let row = Rect::from_min_size(pos2(x0, y + i as f32 * 72.0), vec2(cw, 72.0));
            p.circle_filled(pos2(row.left() + 44.0, row.center().y), 22.0, pl.surface_hi);
            art::headphones(
                p,
                Rect::from_center_size(
                    pos2(row.left() + 44.0, row.center().y - 1.0),
                    vec2(32.0, 32.0),
                ),
                pl.accent,
            );
            let bw = widgets::button_width(p, t(T::Connect), 14.0).max(110.0);
            fit_text(
                p,
                pos2(row.left() + 80.0, row.center().y - 10.0),
                Align2::LEFT_CENTER,
                &d.name,
                theme::medium(15.5),
                cw - 80.0 - bw - 40.0,
                pl.text,
            );
            let sub = if d.connected {
                t(T::Connected)
            } else {
                t(T::NotConnected)
            };
            fit_text(
                p,
                pos2(row.left() + 80.0, row.center().y + 12.0),
                Align2::LEFT_CENTER,
                sub,
                theme::regular(12.5),
                cw - 80.0 - bw - 40.0,
                pl.text_dim,
            );
            let br = Rect::from_center_size(
                pos2(row.right() - 24.0 - bw / 2.0, row.center().y),
                vec2(bw, 40.0),
            );
            if widgets::button(ui, p, br, t(T::Connect), ButtonKind::Primary, true) {
                self.connect(Target {
                    name: d.name.clone(),
                    address: d.address.clone(),
                });
            }
            if i + 1 < devices.len() {
                widgets::divider(p, row.left() + 80.0, row.right() - 24.0, row.bottom());
            }
        }
        y = list.bottom() + 12.0;
        let toggle = if self.show_all {
            t(T::ShowSoundcoreOnly)
        } else {
            t(T::ShowAllPaired)
        };
        let tw = widgets::button_width(p, toggle, 14.0);
        if widgets::button(
            ui,
            p,
            Rect::from_min_size(pos2(x0 - 6.0, y), vec2(tw.min(cw / 2.0 + 40.0), 36.0)),
            toggle,
            ButtonKind::Quiet,
            true,
        ) {
            self.show_all = !self.show_all;
            self.refresh_paired(f64::MAX);
        }
        let pw = widgets::button_width(p, t(T::PairNew), 14.0);
        if widgets::button(
            ui,
            p,
            Rect::from_min_size(
                pos2(x0 + cw - pw + 6.0, y),
                vec2(pw.min(cw / 2.0 - 40.0), 36.0),
            ),
            t(T::PairNew),
            ButtonKind::Quiet,
            true,
        ) {
            open_bluetooth_settings();
        }
        y += 56.0;

        // Language and disclaimer.
        let lang = format!("{} · {}", t(T::Language), self.current_lang().native_name());
        let lw = widgets::button_width(p, &lang, 14.0);
        if widgets::button(
            ui,
            p,
            Rect::from_center_size(pos2(rect.center().x, y + 18.0), vec2(lw, 36.0)),
            &lang,
            ButtonKind::Secondary,
            true,
        ) {
            self.modal = Modal::Language;
        }
        y += 56.0;
        let g = p.layout(
            t(T::Disclaimer).to_string(),
            theme::regular(12.0),
            pl.text_faint,
            cw,
        );
        let dh = g.size().y;
        p.galley(
            pos2(rect.center().x - g.size().x / 2.0, y),
            g,
            pl.text_faint,
        );
        y + dh
    }
}
