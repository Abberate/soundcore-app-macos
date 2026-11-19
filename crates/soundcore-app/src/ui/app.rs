//! Window shell: header, tabs, modals, toasts, preferences and visual-check hooks.

use std::path::PathBuf;

use egui::{
    Align2, Color32, CornerRadius, Key, Painter, Pos2, Rect, ScrollArea, Sense, Stroke, StrokeKind,
    Ui, UiBuilder, ViewportCommand, pos2, vec2,
};
use soundcore_bt::PairedDevice;

use super::i18n::{Lang, T, set_lang, t};
use super::theme::{self, pal, set_palette};
use super::widgets::{self, ButtonKind, fit_text};
use super::{art, theme::DARK, theme::LIGHT};
use crate::session::{Action, Notice, Phase, SessionHandle, Snapshot, Target};

pub const APP_NAME: &str = "Soundcore App";
pub const HEADER_H: f32 = 64.0;
pub const TABS_H: f32 = 56.0;
/// Room for the window's traffic lights (the title bar is transparent).
const TRAFFIC_LIGHTS: f32 = 84.0;

const KEY_LAST_DEVICE: &str = "last_device";
const KEY_LANG: &str = "lang";
const KEY_THEME: &str = "theme";
const KEY_CUSTOM_EQ: &str = "custom_eq";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab {
    Controls,
    Equalizer,
    Settings,
}

impl Tab {
    pub const ALL: [Tab; 3] = [Tab::Controls, Tab::Equalizer, Tab::Settings];
    pub fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Modal {
    None,
    Devices,
    Language,
    Log,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ThemePref {
    System,
    Light,
    Dark,
}

pub struct SoundcoreApp {
    pub(super) session: SessionHandle,
    pub(super) tab: Tab,
    pub(super) modal: Modal,
    pub(super) paired: Vec<PairedDevice>,
    paired_at: Option<f64>,
    pub(super) show_all: bool,
    last_device: Option<Target>,
    auto_connect_done: bool,
    pub(super) lang_pref: Option<Lang>,
    pub(super) theme_pref: ThemePref,
    content_h: [f32; 4],
    pub(super) eq_drag: Option<usize>,
    pub(super) eq_draft: Option<[i16; 8]>,
    pub(super) eq_sent_at: f64,
    pub(super) custom_eq: [i16; 8],
    toast: Option<(String, f64)>,
    last_notice: u64,
    screenshot: Option<(PathBuf, u32)>,
    frames: u32,
    persist: bool,
    pub(super) demo: bool,
    /// Demo scene "connect": show the connection screen.
    force_connect: bool,
    title: String,
    pub(super) log_copied_at: f64,
}

fn parse_target(s: &str) -> Option<Target> {
    let (name, address) = s.split_once('\t')?;
    Some(Target {
        name: name.into(),
        address: address.into(),
    })
}

fn parse_eq(s: &str) -> Option<[i16; 8]> {
    let v: Vec<i16> = s.split(',').filter_map(|x| x.trim().parse().ok()).collect();
    <[i16; 8]>::try_from(v.as_slice()).ok()
}

impl SoundcoreApp {
    pub fn new(cc: &eframe::CreationContext<'_>, demo: bool) -> Self {
        theme::install_fonts(&cc.egui_ctx);
        let ctx = cc.egui_ctx.clone();
        let repaint = move || ctx.request_repaint();
        let session = if demo {
            SessionHandle::spawn_demo(repaint)
        } else {
            SessionHandle::spawn(repaint)
        };
        let get = |k: &str| cc.storage.and_then(|s| s.get_string(k));
        let env = |k: &str| std::env::var(k).ok();

        let lang_pref = env("SOUNDCORE_LANG")
            .or_else(|| get(KEY_LANG))
            .and_then(|c| Lang::from_code(&c));
        let theme_pref = match env("SOUNDCORE_THEME").or_else(|| get(KEY_THEME)).as_deref() {
            Some("light") => ThemePref::Light,
            Some("dark") => ThemePref::Dark,
            _ => ThemePref::System,
        };
        let screenshot = env("SOUNDCORE_SCREENSHOT").map(|p| {
            let frames = env("SOUNDCORE_FRAMES")
                .and_then(|f| f.parse().ok())
                .unwrap_or(40);
            (PathBuf::from(p), frames)
        });
        let test_run = demo || std::env::vars().any(|(k, _)| k.starts_with("SOUNDCORE_"));
        let mut app = Self {
            session,
            tab: Tab::Controls,
            modal: Modal::None,
            paired: Vec::new(),
            paired_at: None,
            show_all: false,
            last_device: get(KEY_LAST_DEVICE).and_then(|s| parse_target(&s)),
            auto_connect_done: demo,
            lang_pref,
            theme_pref,
            content_h: [0.0; 4],
            eq_drag: None,
            eq_draft: None,
            eq_sent_at: 0.0,
            custom_eq: get(KEY_CUSTOM_EQ)
                .and_then(|s| parse_eq(&s))
                .unwrap_or([0; 8]),
            toast: None,
            last_notice: 0,
            screenshot,
            frames: 0,
            persist: !test_run,
            demo,
            force_connect: false,
            title: String::new(),
            log_copied_at: -10.0,
        };
        if let Ok(scene) = std::env::var("SOUNDCORE_SCENE") {
            match scene.as_str() {
                "eq" | "custom" => app.tab = Tab::Equalizer,
                "settings" => app.tab = Tab::Settings,
                "devices" => app.modal = Modal::Devices,
                "language" => {
                    app.tab = Tab::Settings;
                    app.modal = Modal::Language;
                }
                "log" => {
                    app.tab = Tab::Settings;
                    app.modal = Modal::Log;
                }
                "connect" => app.force_connect = true,
                _ => {}
            }
        }
        app
    }

    pub(super) fn act(&self, a: Action) {
        self.session.act(a);
    }

    pub(super) fn connect(&mut self, target: Target) {
        self.last_device = Some(target.clone());
        self.force_connect = false;
        self.modal = Modal::None;
        self.tab = Tab::Controls;
        self.session.connect(target);
    }

    pub(super) fn disconnect(&mut self) {
        self.modal = Modal::None;
        self.session.disconnect();
        self.last_device = None;
    }

    pub(super) fn current_lang(&self) -> Lang {
        self.lang_pref.unwrap_or_else(Lang::system)
    }

    /// Paired devices, refreshed every few seconds while visible.
    pub(super) fn refresh_paired(&mut self, now: f64) {
        if self.paired_at.is_some_and(|t| now - t < 3.0) {
            return;
        }
        self.paired_at = Some(now);
        self.paired = if self.demo {
            vec![PairedDevice {
                name: "soundcore Q20i".into(),
                address: "00-00-00-00-00-00".into(),
                connected: true,
                class_of_device: 0,
            }]
        } else {
            soundcore_bt::paired_devices()
        };
    }

    pub(super) fn visible_devices(&self) -> Vec<PairedDevice> {
        self.paired
            .iter()
            .filter(|d| self.show_all || d.looks_like_soundcore())
            .cloned()
            .collect()
    }

    pub(super) fn show_toast(&mut self, text: &str, now: f64) {
        self.toast = Some((text.to_string(), now));
    }

    fn apply_theme(&self, ui: &Ui) {
        let dark = match self.theme_pref {
            ThemePref::Light => false,
            ThemePref::Dark => true,
            ThemePref::System => ui.ctx().system_theme() == Some(egui::Theme::Dark),
        };
        set_palette(if dark { DARK } else { LIGHT });
        ui.ctx().set_theme(if dark {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        });
    }

    // ---------------------------------------------------------------------------------
    // header and tabs
    // ---------------------------------------------------------------------------------

    fn header(&mut self, ui: &Ui, p: &Painter, screen: Rect, snap: &Snapshot) {
        let pl = pal();
        let y = screen.top() + HEADER_H / 2.0;
        let mark = Rect::from_center_size(
            pos2(screen.left() + TRAFFIC_LIGHTS + 16.0, y),
            vec2(30.0, 30.0),
        );
        art::app_mark(p, mark);
        let name_r = fit_text(
            p,
            pos2(mark.right() + 12.0, y),
            Align2::LEFT_CENTER,
            APP_NAME,
            theme::semibold(16.0),
            160.0,
            pl.text,
        );
        let tag = t(T::Unofficial);
        let tag_w = widgets::chip_width(p, tag) - 8.0;
        if screen.width() > 700.0 {
            let tr = Rect::from_min_size(pos2(name_r.right() + 10.0, y - 11.0), vec2(tag_w, 22.0));
            p.rect_filled(tr, CornerRadius::same(11), pl.surface_hi);
            fit_text(
                p,
                tr.center(),
                Align2::CENTER_CENTER,
                tag,
                theme::medium(11.5),
                tag_w - 12.0,
                pl.text_dim,
            );
        }

        // Device chip (right).
        let Some(target) = &snap.target else { return };
        let label = target.name.as_str();
        let w = (p
            .layout_no_wrap(label.into(), theme::medium(14.0), pl.text)
            .size()
            .x
            + 66.0)
            .min(280.0);
        let r = Rect::from_center_size(pos2(screen.right() - 24.0 - w / 2.0, y), vec2(w, 38.0));
        let resp = widgets::hit(ui, r, "device-chip", true);
        p.rect(
            r,
            CornerRadius::same(19),
            if resp.hovered() {
                pl.surface_hi
            } else {
                pl.surface
            },
            Stroke::new(1.0, pl.stroke),
            StrokeKind::Inside,
        );
        let dot = match snap.phase {
            Phase::Ready => pl.good,
            Phase::Busy | Phase::Failed(_) => pl.warn,
            _ => pl.text_faint,
        };
        p.circle_filled(pos2(r.left() + 18.0, y), 4.5, dot);
        fit_text(
            p,
            pos2(r.left() + 30.0, y),
            Align2::LEFT_CENTER,
            label,
            theme::medium(14.0),
            w - 62.0,
            pl.text,
        );
        art::chevron_down(p, pos2(r.right() - 18.0, y + 1.0), 10.0, pl.text_dim);
        if resp.clicked() {
            self.modal = Modal::Devices;
        }
    }

    fn tabs(&mut self, ui: &Ui, p: &Painter, screen: Rect) {
        let w = 420.0f32.min(screen.width() - 48.0);
        let r = Rect::from_center_size(
            pos2(
                screen.center().x,
                screen.top() + HEADER_H + TABS_H / 2.0 - 4.0,
            ),
            vec2(w, 42.0),
        );
        let labels = [t(T::TabControls), t(T::TabEqualizer), t(T::TabSettings)];
        if let Some(i) = widgets::segmented(ui, p, r, &labels, self.tab.index(), "tabs") {
            self.tab = Tab::ALL[i];
        }
    }

    // ---------------------------------------------------------------------------------
    // modals
    // ---------------------------------------------------------------------------------

    fn modal_frame(
        &mut self,
        ui: &Ui,
        p: &Painter,
        screen: Rect,
        w: f32,
        h: f32,
        title: &str,
    ) -> Rect {
        let pl = pal();
        p.rect_filled(screen, 0.0, pl.overlay);
        let r = Rect::from_center_size(
            screen.center(),
            vec2(w.min(screen.width() - 32.0), h.min(screen.height() - 32.0)),
        );
        // Swallow clicks outside the dialog.
        let outside = ui.interact(screen, egui::Id::unique("modal-backdrop"), Sense::click());
        if outside.clicked()
            && outside
                .interact_pointer_pos()
                .is_some_and(|q| !r.contains(q))
        {
            self.modal = Modal::None;
        }
        p.rect(
            r,
            CornerRadius::same(24),
            pl.surface,
            Stroke::new(1.0, pl.stroke),
            StrokeKind::Inside,
        );
        fit_text(
            p,
            r.left_top() + vec2(28.0, 28.0),
            Align2::LEFT_TOP,
            title,
            theme::semibold(19.0),
            r.width() - 56.0,
            pl.text,
        );
        r
    }

    fn modal_devices(&mut self, ui: &Ui, p: &Painter, screen: Rect, snap: &Snapshot, now: f64) {
        self.refresh_paired(now);
        let devices = self.visible_devices();
        let rows = devices.len().max(1) as f32;
        let h = 150.0 + rows * 64.0 + 70.0;
        let r = self.modal_frame(ui, p, screen, 520.0, h, t(T::MyDevices));
        let pl = pal();
        let x0 = r.left() + 28.0;
        let x1 = r.right() - 28.0;
        let mut y = r.top() + 74.0;
        if devices.is_empty() {
            let hh = widgets::paragraph(
                p,
                pos2(x0, y),
                x1 - x0,
                t(T::NoDevices),
                theme::regular(13.5),
                pl.text_dim,
            );
            y += hh.max(64.0);
        }
        let current = snap.target.as_ref().map(|t| t.address.clone());
        for d in &devices {
            let row = Rect::from_min_max(pos2(x0, y), pos2(x1, y + 64.0));
            let in_use = current.as_deref() == Some(d.address.as_str());
            p.circle_filled(pos2(x0 + 18.0, row.center().y), 18.0, pl.surface_hi);
            art::headphones(
                p,
                Rect::from_center_size(pos2(x0 + 18.0, row.center().y - 1.0), vec2(26.0, 26.0)),
                pl.accent,
            );
            let label = if in_use { t(T::InUse) } else { t(T::Switch) };
            let bw = widgets::button_width(p, label, 14.0).max(96.0);
            fit_text(
                p,
                pos2(x0 + 48.0, row.center().y - 9.0),
                Align2::LEFT_CENTER,
                &d.name,
                theme::medium(15.0),
                x1 - x0 - 48.0 - bw - 16.0,
                pl.text,
            );
            let sub = if d.connected {
                t(T::Connected)
            } else {
                t(T::NotConnected)
            };
            fit_text(
                p,
                pos2(x0 + 48.0, row.center().y + 11.0),
                Align2::LEFT_CENTER,
                sub,
                theme::regular(12.5),
                x1 - x0 - 48.0 - bw - 16.0,
                pl.text_dim,
            );
            let br = Rect::from_center_size(pos2(x1 - bw / 2.0, row.center().y), vec2(bw, 36.0));
            let kind = if in_use {
                ButtonKind::Secondary
            } else {
                ButtonKind::Primary
            };
            if widgets::button(ui, p, br, label, kind, !in_use) {
                self.connect(Target {
                    name: d.name.clone(),
                    address: d.address.clone(),
                });
            }
            y += 64.0;
            widgets::divider(p, x0, x1, y);
        }
        y += 12.0;
        let toggle = if self.show_all {
            t(T::ShowSoundcoreOnly)
        } else {
            t(T::ShowAllPaired)
        };
        let tw = widgets::button_width(p, toggle, 14.0);
        if widgets::button(
            ui,
            p,
            Rect::from_min_size(pos2(x0 - 14.0, y), vec2(tw.min(x1 - x0), 34.0)),
            toggle,
            ButtonKind::Quiet,
            true,
        ) {
            self.show_all = !self.show_all;
            self.paired_at = None;
        }
        let by = r.bottom() - 28.0 - 40.0;
        let close_w = widgets::button_width(p, t(T::Close), 14.0).max(100.0);
        if widgets::button(
            ui,
            p,
            Rect::from_min_size(pos2(x1 - close_w, by), vec2(close_w, 40.0)),
            t(T::Close),
            ButtonKind::Secondary,
            true,
        ) {
            self.modal = Modal::None;
        }
        let pair_w = widgets::button_width(p, t(T::PairNew), 14.0);
        if widgets::button(
            ui,
            p,
            Rect::from_min_size(pos2(x0, by), vec2(pair_w, 40.0)),
            t(T::PairNew),
            ButtonKind::Quiet,
            true,
        ) {
            open_bluetooth_settings();
        }
        if snap.target.is_some() && !self.demo {
            let dw = widgets::button_width(p, t(T::Disconnect), 14.0);
            let dx = x1 - close_w - 12.0 - dw;
            if dx > x0 + pair_w + 12.0
                && widgets::button(
                    ui,
                    p,
                    Rect::from_min_size(pos2(dx, by), vec2(dw, 40.0)),
                    t(T::Disconnect),
                    ButtonKind::Danger,
                    true,
                )
            {
                self.disconnect();
            }
        }
    }

    fn modal_language(&mut self, ui: &Ui, p: &Painter, screen: Rect) {
        let r = self.modal_frame(
            ui,
            p,
            screen,
            400.0,
            74.0 + 6.0 * 48.0 + 84.0,
            t(T::Language),
        );
        let pl = pal();
        let x0 = r.left() + 20.0;
        let x1 = r.right() - 20.0;
        let mut y = r.top() + 70.0;
        let mut options: Vec<(Option<Lang>, String)> = vec![(
            None,
            format!(
                "{} · {}",
                t(T::SystemLanguage),
                Lang::system().native_name()
            ),
        )];
        options.extend(
            Lang::ALL
                .iter()
                .map(|l| (Some(*l), l.native_name().to_string())),
        );
        for (i, (lang, label)) in options.iter().enumerate() {
            let row = Rect::from_min_max(pos2(x0, y), pos2(x1, y + 46.0));
            let resp = widgets::hit(ui, row, ("lang", i), true);
            let selected = self.lang_pref == *lang;
            if selected || resp.hovered() {
                p.rect_filled(
                    row,
                    CornerRadius::same(12),
                    if selected {
                        pl.accent_soft
                    } else {
                        pl.surface_hi
                    },
                );
            }
            fit_text(
                p,
                pos2(row.left() + 14.0, row.center().y),
                Align2::LEFT_CENTER,
                label,
                theme::medium(14.5),
                row.width() - 60.0,
                pl.text,
            );
            if selected {
                p.circle_filled(pos2(row.right() - 20.0, row.center().y), 5.0, pl.accent);
            }
            if resp.clicked() {
                self.lang_pref = *lang;
                self.modal = Modal::None;
            }
            y += 48.0;
        }
        let cw = widgets::button_width(p, t(T::Close), 14.0).max(100.0);
        if widgets::button(
            ui,
            p,
            Rect::from_min_size(
                pos2(r.right() - 28.0 - cw, r.bottom() - 68.0),
                vec2(cw, 40.0),
            ),
            t(T::Close),
            ButtonKind::Secondary,
            true,
        ) {
            self.modal = Modal::None;
        }
    }

    fn modal_log(&mut self, ui: &mut Ui, p: &Painter, screen: Rect, snap: &Snapshot, now: f64) {
        let r = self.modal_frame(ui, p, screen, 760.0, 560.0, t(T::Log));
        let pl = pal();
        let area = Rect::from_min_max(
            r.left_top() + vec2(24.0, 70.0),
            r.right_bottom() - vec2(24.0, 76.0),
        );
        p.rect_filled(area, CornerRadius::same(14), pl.surface_hi);
        let text = if snap.log.is_empty() {
            "—".to_string()
        } else {
            snap.log.join("\n")
        };
        ui.scope_builder(UiBuilder::new().max_rect(area.shrink(12.0)), |ui| {
            ScrollArea::both()
                .id_salt("log")
                .auto_shrink([false; 2])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(&text)
                            .monospace()
                            .size(11.5)
                            .color(pl.text_dim),
                    );
                });
        });
        let by = r.bottom() - 60.0;
        let cw = widgets::button_width(p, t(T::Close), 14.0).max(100.0);
        if widgets::button(
            ui,
            p,
            Rect::from_min_size(pos2(r.right() - 24.0 - cw, by), vec2(cw, 40.0)),
            t(T::Close),
            ButtonKind::Secondary,
            true,
        ) {
            self.modal = Modal::None;
        }
        let copy = if now - self.log_copied_at < 1.5 {
            t(T::Copied)
        } else {
            t(T::CopyLog)
        };
        let kw = widgets::button_width(p, copy, 14.0).max(100.0);
        if widgets::button(
            ui,
            p,
            Rect::from_min_size(pos2(r.left() + 24.0, by), vec2(kw, 40.0)),
            copy,
            ButtonKind::Primary,
            true,
        ) {
            ui.ctx().copy_text(text);
            self.log_copied_at = now;
        }
    }

    fn draw_toast(&mut self, p: &Painter, screen: Rect, now: f64) {
        let Some((text, at)) = &self.toast else {
            return;
        };
        let age = now - at;
        if age > 3.5 {
            self.toast = None;
            return;
        }
        let pl = pal();
        let alpha = ((3.5 - age) / 0.3).clamp(0.0, 1.0) as f32;
        let g = widgets::fitted(
            p,
            text,
            theme::medium(13.5),
            pl.on_accent,
            screen.width() - 120.0,
        );
        let r = Rect::from_center_size(
            pos2(screen.center().x, screen.bottom() - 48.0),
            g.size() + vec2(36.0, 20.0),
        );
        let bg = if pl.dark {
            Color32::from_rgb(0xF1, 0xF3, 0xF6)
        } else {
            Color32::from_rgb(0x1A, 0x1F, 0x27)
        };
        let fg = if pl.dark {
            Color32::from_rgb(0x11, 0x14, 0x19)
        } else {
            Color32::WHITE
        };
        p.rect_filled(
            r,
            CornerRadius::same((r.height() / 2.0) as u8),
            bg.gamma_multiply(alpha),
        );
        let g = widgets::fitted(
            p,
            text,
            theme::medium(13.5),
            fg.gamma_multiply(alpha),
            screen.width() - 120.0,
        );
        p.galley(r.center() - g.size() / 2.0, g, fg);
    }

    fn take_screenshot(&mut self, ui: &Ui) {
        let Some((path, frames)) = self.screenshot.clone() else {
            return;
        };
        self.frames += 1;
        if self.frames == frames {
            ui.ctx()
                .send_viewport_cmd(ViewportCommand::Screenshot(egui::ScreenshotCallback::new(
                    move |img| {
                        let [w, h] = img.size;
                        let rgba: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
                        if let Some(buf) = image::RgbaImage::from_raw(w as u32, h as u32, rgba) {
                            let _ = buf.save(&path);
                        }
                        std::process::exit(0);
                    },
                )));
        }
        ui.ctx().request_repaint();
    }

    /// Lays a page out in a scrollable column under the header.
    fn scrolled(
        &mut self,
        ui: &mut Ui,
        area: Rect,
        slot: usize,
        now: f64,
        snap: &Snapshot,
        page: Page,
    ) {
        let h = self.content_h[slot].max(area.height());
        ui.scope_builder(UiBuilder::new().max_rect(area), |ui| {
            ScrollArea::vertical()
                .id_salt(("page", slot))
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    let (rect, _) =
                        ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
                    let p = ui.painter().clone();
                    let cw = 720.0f32.min(rect.width() - 48.0);
                    let x0 = rect.center().x - cw / 2.0;
                    let top = rect.top() + 8.0;
                    let bottom = match page {
                        Page::Controls => self.page_controls(ui, &p, x0, cw, top, snap, now),
                        Page::Equalizer => self.page_equalizer(ui, &p, x0, cw, top, snap, now),
                        Page::Settings => self.page_settings(ui, &p, x0, cw, top, snap, now),
                        Page::Connect => self.connect_screen(ui, &p, rect, snap, now),
                    };
                    self.content_h[slot] = bottom - rect.top() + 40.0;
                });
        });
    }
}

#[derive(Clone, Copy)]
enum Page {
    Controls,
    Equalizer,
    Settings,
    Connect,
}

pub fn open_url(url: &str) {
    let _ = std::process::Command::new("open").arg(url).spawn();
}

pub fn open_bluetooth_settings() {
    open_url("x-apple.systempreferences:com.apple.BluetoothSettings");
}

impl eframe::App for SoundcoreApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let now = ui.input(|i| i.time);
        set_lang(self.current_lang());
        self.apply_theme(ui);
        let screen = ui.max_rect();
        let pl = pal();
        let p = ui.painter().clone();
        p.rect_filled(screen, 0.0, pl.bg);
        let snap = self.session.snapshot();

        // Auto-connect to the last device once.
        if !self.auto_connect_done {
            self.auto_connect_done = true;
            if let Some(t) = self.last_device.clone() {
                self.session.connect(t);
            }
        }
        // Notices.
        if let Some((seq, n)) = snap.notice
            && seq != self.last_notice
        {
            self.last_notice = seq;
            let text = match n {
                Notice::NoAck => t(T::NoAck),
                Notice::Lost => t(T::Lost),
            };
            self.show_toast(text, now);
        }
        // Window title: the headphones' own name.
        let title = snap
            .target
            .as_ref()
            .map(|t| t.name.clone())
            .unwrap_or_else(|| APP_NAME.to_string());
        if title != self.title {
            ui.ctx()
                .send_viewport_cmd(ViewportCommand::Title(title.clone()));
            self.title = title;
        }
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            self.modal = Modal::None;
        }
        if self.modal == Modal::None && snap.target.is_some() {
            for (k, tab) in [
                (Key::Num1, Tab::Controls),
                (Key::Num2, Tab::Equalizer),
                (Key::Num3, Tab::Settings),
            ] {
                if ui.input(|i| i.modifiers.command && i.key_pressed(k)) {
                    self.tab = tab;
                }
            }
        }

        let connect_screen = snap.target.is_none() || self.force_connect;
        // Header strip.
        self.header(ui, &p, screen, &snap);
        if connect_screen {
            let area = Rect::from_min_max(pos2(screen.left(), screen.top() + HEADER_H), screen.max);
            self.scrolled(ui, area, 3, now, &snap, Page::Connect);
        } else {
            self.tabs(ui, &p, screen);
            let area = Rect::from_min_max(
                pos2(screen.left(), screen.top() + HEADER_H + TABS_H),
                screen.max,
            );
            let page = match self.tab {
                Tab::Controls => Page::Controls,
                Tab::Equalizer => Page::Equalizer,
                Tab::Settings => Page::Settings,
            };
            self.scrolled(ui, area, self.tab.index(), now, &snap, page);
        }

        // Modals on top.
        let fg = ui.ctx().layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::unique("fg"),
        ));
        match self.modal {
            Modal::None => {}
            Modal::Devices => {
                let fg_ui = ui.new_child(UiBuilder::new().layer_id(fg.layer_id()).max_rect(screen));
                self.modal_devices(&fg_ui, &fg, screen, &snap, now);
            }
            Modal::Language => {
                let fg_ui = ui.new_child(UiBuilder::new().layer_id(fg.layer_id()).max_rect(screen));
                self.modal_language(&fg_ui, &fg, screen);
            }
            Modal::Log => {
                let mut fg_ui =
                    ui.new_child(UiBuilder::new().layer_id(fg.layer_id()).max_rect(screen));
                self.modal_log(&mut fg_ui, &fg, screen, &snap, now);
            }
        }
        self.draw_toast(&fg, screen, now);
        if self.toast.is_some() || snap.phase != Phase::Ready {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(250));
        }
        self.take_screenshot(ui);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if !self.persist {
            return;
        }
        let device = self
            .last_device
            .as_ref()
            .map(|t| format!("{}\t{}", t.name, t.address))
            .unwrap_or_default();
        storage.set_string(KEY_LAST_DEVICE, device);
        storage.set_string(
            KEY_LANG,
            self.lang_pref
                .map(|l| l.code().to_string())
                .unwrap_or_else(|| "system".into()),
        );
        let theme = match self.theme_pref {
            ThemePref::System => "system",
            ThemePref::Light => "light",
            ThemePref::Dark => "dark",
        };
        storage.set_string(KEY_THEME, theme.into());
        storage.set_string(
            KEY_CUSTOM_EQ,
            self.custom_eq
                .iter()
                .map(|g| g.to_string())
                .collect::<Vec<_>>()
                .join(","),
        );
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.session.shutdown();
    }
}

/// A small status pill (used on cards).
pub fn pill(p: &Painter, pos: Pos2, text: &str, fg: Color32, bg: Color32) -> Rect {
    let g = p.layout_no_wrap(text.to_string(), theme::medium(12.0), fg);
    let r = Rect::from_min_size(pos, g.size() + vec2(20.0, 8.0));
    p.rect_filled(r, CornerRadius::same((r.height() / 2.0) as u8), bg);
    p.galley(r.center() - g.size() / 2.0, g, fg);
    r
}
