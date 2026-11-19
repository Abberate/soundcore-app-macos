//! Soundcore App for macOS (unofficial): companion for soundcore headphones.

/// Default window size; `SOUNDCORE_WINDOW=WxH` overrides it (visual checks).
fn window_size() -> [f32; 2] {
    std::env::var("SOUNDCORE_WINDOW")
        .ok()
        .and_then(|v| {
            let (w, h) = v.split_once('x')?;
            Some([w.parse().ok()?, h.parse().ok()?])
        })
        .unwrap_or([980.0, 760.0])
}

fn demo() -> bool {
    std::env::args().any(|a| a == "--demo") || std::env::var_os("SOUNDCORE_DEMO").is_some()
}

fn test_run() -> bool {
    demo() || std::env::vars_os().any(|(k, _)| k.to_string_lossy().starts_with("SOUNDCORE_"))
}

/// Dock and app switcher icon.
///
/// Without one, eframe swaps its default egui logo into the Dock at launch. Inside the
/// .app bundle it gets an empty icon so macOS keeps the bundle's AppIcon.icns, as in
/// Finder and Launchpad; a bare `cargo run` binary uses the same artwork from the PNG.
fn app_icon() -> egui::IconData {
    let bundled = std::env::current_exe()
        .map(|p| p.to_string_lossy().contains(".app/Contents/MacOS/"))
        .unwrap_or(false);
    if bundled {
        return egui::IconData::default();
    }
    image::load_from_memory(include_bytes!("../../../packaging/icon_1024.png"))
        .map(|img| {
            let img = img.into_rgba8();
            let (width, height) = img.dimensions();
            egui::IconData {
                rgba: img.into_raw(),
                width,
                height,
            }
        })
        .unwrap_or_default()
}

fn main() -> eframe::Result {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    if let Some(out) = std::env::var_os("SOUNDCORE_DMG_BG") {
        // Installer artwork (see scripts/make_dmg.sh).
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size(soundcore_app::ui::dmg_bg::SIZE)
                .with_resizable(false)
                .with_decorations(false)
                .with_icon(egui::IconData::default()),
            persistence_path: Some(std::env::temp_dir().join("soundcore-app-test.ron")),
            ..Default::default()
        };
        return eframe::run_native(
            "Soundcore App installer art",
            options,
            Box::new(move |cc| {
                Ok(Box::new(soundcore_app::ui::dmg_bg::DmgBackground::new(
                    cc,
                    out.into(),
                )))
            }),
        );
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(soundcore_app::ui::APP_NAME)
            .with_app_id("io.github.soundcore-mac")
            .with_icon(app_icon())
            .with_inner_size(window_size())
            .with_min_inner_size([760.0, 580.0])
            .with_fullsize_content_view(true)
            .with_titlebar_shown(false)
            .with_title_shown(false)
            .with_titlebar_buttons_shown(true)
            .with_fullscreen(std::env::var_os("SOUNDCORE_FULLSCREEN").is_some()),
        // Demo and visual-check runs keep their own throw-away storage.
        persistence_path: test_run().then(|| std::env::temp_dir().join("soundcore-app-test.ron")),
        ..Default::default()
    };
    let demo = demo();
    eframe::run_native(
        soundcore_app::ui::APP_NAME,
        options,
        Box::new(move |cc| Ok(Box::new(soundcore_app::ui::SoundcoreApp::new(cc, demo)))),
    )
}
