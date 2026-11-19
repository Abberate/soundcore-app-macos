//! Colours and fonts. Light first, like the headphones' own app: airy grey ground,
//! white cards, one vivid blue for everything that is "on".

use std::cell::Cell;
use std::sync::Arc;

use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub dark: bool,
    pub bg: Color32,
    pub surface: Color32,
    pub surface_hi: Color32,
    pub stroke: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub text_faint: Color32,
    pub accent: Color32,
    /// Accent wash behind selected items.
    pub accent_soft: Color32,
    pub on_accent: Color32,
    pub good: Color32,
    pub warn: Color32,
    pub overlay: Color32,
    /// Headphone illustration.
    pub product: Color32,
    pub product_hi: Color32,
}

pub const LIGHT: Palette = Palette {
    dark: false,
    bg: Color32::from_rgb(0xF2, 0xF4, 0xF7),
    surface: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    surface_hi: Color32::from_rgb(0xEC, 0xEF, 0xF3),
    stroke: Color32::from_rgb(0xE1, 0xE5, 0xEB),
    text: Color32::from_rgb(0x11, 0x14, 0x19),
    text_dim: Color32::from_rgb(0x5B, 0x63, 0x70),
    text_faint: Color32::from_rgb(0x97, 0x9F, 0xAB),
    accent: Color32::from_rgb(0x15, 0x76, 0xFF),
    accent_soft: Color32::from_rgb(0xE4, 0xEE, 0xFF),
    on_accent: Color32::WHITE,
    good: Color32::from_rgb(0x16, 0xA3, 0x5A),
    warn: Color32::from_rgb(0xE2, 0x9B, 0x00),
    overlay: Color32::from_rgba_premultiplied(8, 12, 20, 110),
    product: Color32::from_rgb(0x23, 0x27, 0x2E),
    product_hi: Color32::from_rgb(0x3A, 0x40, 0x4A),
};

pub const DARK: Palette = Palette {
    dark: true,
    bg: Color32::from_rgb(0x0C, 0x0F, 0x14),
    surface: Color32::from_rgb(0x16, 0x1A, 0x21),
    surface_hi: Color32::from_rgb(0x1F, 0x24, 0x2D),
    stroke: Color32::from_rgb(0x27, 0x2D, 0x37),
    text: Color32::from_rgb(0xF1, 0xF3, 0xF6),
    text_dim: Color32::from_rgb(0xA2, 0xAA, 0xB6),
    text_faint: Color32::from_rgb(0x69, 0x71, 0x7E),
    accent: Color32::from_rgb(0x3B, 0x8C, 0xFF),
    accent_soft: Color32::from_rgb(0x15, 0x24, 0x3C),
    on_accent: Color32::WHITE,
    good: Color32::from_rgb(0x2E, 0xC2, 0x70),
    warn: Color32::from_rgb(0xF0, 0xB0, 0x2A),
    overlay: Color32::from_rgba_premultiplied(0, 0, 0, 150),
    product: Color32::from_rgb(0x3A, 0x40, 0x4B),
    product_hi: Color32::from_rgb(0x55, 0x5D, 0x6A),
};

thread_local! {
    static CURRENT: Cell<Palette> = const { Cell::new(LIGHT) };
}

pub fn set_palette(p: Palette) {
    CURRENT.with(|c| c.set(p));
}

/// Palette for the frame being drawn.
pub fn pal() -> Palette {
    CURRENT.with(|c| c.get())
}

const REGULAR_TTF: &[u8] = include_bytes!("../../assets/fonts/Poppins-Regular.ttf");
const MEDIUM_TTF: &[u8] = include_bytes!("../../assets/fonts/Poppins-Medium.ttf");
const SEMIBOLD_TTF: &[u8] = include_bytes!("../../assets/fonts/Poppins-SemiBold.ttf");

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    for (name, data) in [
        ("pp-regular", REGULAR_TTF),
        ("pp-medium", MEDIUM_TTF),
        ("pp-semibold", SEMIBOLD_TTF),
    ] {
        fonts
            .font_data
            .insert(name.into(), Arc::new(FontData::from_static(data)));
    }
    let proportional = fonts
        .families
        .get_mut(&FontFamily::Proportional)
        .expect("proportional family");
    proportional.insert(0, "pp-regular".into());
    // egui's own fonts stay as fallbacks (symbols, other scripts).
    let fallbacks: Vec<String> = proportional.iter().skip(1).cloned().collect();
    for (family, face) in [("medium", "pp-medium"), ("semibold", "pp-semibold")] {
        let mut list = vec![face.to_string()];
        list.extend(fallbacks.iter().cloned());
        fonts.families.insert(FontFamily::Name(family.into()), list);
    }
    ctx.set_fonts(fonts);
}

pub fn regular(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

pub fn medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("medium".into()))
}

pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("semibold".into()))
}
