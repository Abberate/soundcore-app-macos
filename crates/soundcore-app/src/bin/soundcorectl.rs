//! Command-line control for soundcore headphones.
//!
//!   soundcorectl list
//!   soundcorectl [--device <address>] status
//!   soundcorectl [--device <address>] mode anc|normal|transparency
//!   soundcorectl [--device <address>] eq <preset-id>|custom <8 gains in dB>
//!   soundcorectl [--device <address>] monitor

use std::time::{Duration, Instant};

use soundcore_app::session::{Action, Phase, SessionHandle, Target};
use soundcore_protocol::{AmbientMode, Equalizer, eq};

fn usage() -> ! {
    eprintln!(
        "usage: soundcorectl list\n       soundcorectl [--device <address>] status|monitor\n       soundcorectl [--device <address>] mode anc|normal|transparency\n       soundcorectl [--device <address>] eq <0-21> | eq custom <8 gains in dB, -6..6>"
    );
    std::process::exit(2);
}

fn pick_device(wanted: Option<&str>) -> Target {
    let devices = soundcore_bt::paired_devices();
    let d = match wanted {
        Some(a) => devices
            .into_iter()
            .find(|d| d.address == soundcore_bt::normalize_address(a)),
        None => devices.into_iter().find(|d| d.looks_like_soundcore()),
    };
    match d {
        Some(d) => Target {
            name: d.name,
            address: d.address,
        },
        None => {
            eprintln!("no paired soundcore device found (try: soundcorectl list)");
            std::process::exit(1);
        }
    }
}

fn wait_ready(s: &SessionHandle, timeout: Duration) {
    let start = Instant::now();
    loop {
        let snap = s.snapshot();
        match snap.phase {
            Phase::Ready => return,
            Phase::Busy if start.elapsed() > Duration::from_secs(8) => {
                eprintln!(
                    "the control channel is in use by another app (the soundcore app on a phone?)"
                );
                std::process::exit(1);
            }
            _ if start.elapsed() > timeout => {
                eprintln!(
                    "timed out ({:?}); are the headphones connected to this Mac?",
                    snap.phase
                );
                std::process::exit(1);
            }
            _ => std::thread::sleep(Duration::from_millis(100)),
        }
    }
}

fn print_status(s: &SessionHandle) {
    let snap = s.snapshot();
    let st = &snap.state;
    if let Some(t) = &snap.target {
        println!("device     {} ({})", t.name, t.address);
    }
    if let Some(m) = st.model {
        println!("model      {} {}", m.id, m.name);
    }
    if let Some(b) = st.battery {
        println!(
            "battery    {} %{}",
            b.percent(),
            if b.charging { " (charging)" } else { "" }
        );
    }
    if let Some(m) = st.sound_modes {
        println!("mode       {:?}", m.ambient);
    }
    if let Some(e) = st.equalizer {
        let name = eq::by_id(e.preset).map(|p| p.key).unwrap_or("custom");
        let gains: Vec<String> = e
            .gains()
            .iter()
            .map(|g| format!("{:+.1}", *g as f32 / 10.0))
            .collect();
        println!("equalizer  {name} [{}]", gains.join(" "));
    }
    println!("firmware   {}", st.firmware.as_deref().unwrap_or("?"));
    println!("serial     {}", st.serial.as_deref().unwrap_or("?"));
}

fn run(args: Vec<String>) {
    let mut device = None;
    let mut rest = Vec::new();
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        if a == "--device" {
            device = it.next();
        } else {
            rest.push(a);
        }
    }
    let Some(cmd) = rest.first().cloned() else {
        usage()
    };
    if cmd == "list" {
        for d in soundcore_bt::paired_devices() {
            println!(
                "{:<28} {}  {}{}",
                d.name,
                d.address,
                if d.connected { "connected" } else { "-" },
                if d.looks_like_soundcore() {
                    "  soundcore"
                } else {
                    ""
                }
            );
        }
        std::process::exit(0);
    }
    let target = pick_device(device.as_deref());
    let s = SessionHandle::spawn(|| {});
    s.connect(target);
    wait_ready(&s, Duration::from_secs(20));
    let settle = |s: &SessionHandle| {
        std::thread::sleep(Duration::from_millis(1500));
        s.act(Action::Refresh);
        std::thread::sleep(Duration::from_millis(800));
        print_status(s);
    };
    match cmd.as_str() {
        "status" => print_status(&s),
        "mode" => {
            let m = match rest.get(1).map(String::as_str) {
                Some("anc") => AmbientMode::NoiseCanceling,
                Some("normal") => AmbientMode::Normal,
                Some("transparency") => AmbientMode::Transparency,
                _ => usage(),
            };
            s.act(Action::SetAmbient(m));
            settle(&s);
        }
        "eq" => {
            let e = match rest.get(1).map(String::as_str) {
                Some("custom") => {
                    let g: Vec<i16> = rest[2..]
                        .iter()
                        .filter_map(|v| v.parse::<f32>().ok())
                        .map(|v| (v.clamp(-6.0, 6.0) * 10.0).round() as i16)
                        .collect();
                    let Ok(g) = <[i16; 8]>::try_from(g.as_slice()) else {
                        usage()
                    };
                    Equalizer::custom(g)
                }
                Some(id) => id
                    .parse()
                    .ok()
                    .and_then(Equalizer::preset)
                    .unwrap_or_else(|| usage()),
                None => usage(),
            };
            s.act(Action::SetEqualizer(e));
            settle(&s);
        }
        "monitor" => {
            let mut last = String::new();
            loop {
                let snap = s.snapshot();
                let line = format!(
                    "{:?} {:?} {:?}",
                    snap.phase,
                    snap.state.battery,
                    snap.state.sound_modes.map(|m| m.ambient)
                );
                if line != last {
                    println!("{line}");
                    last = line;
                }
                std::thread::sleep(Duration::from_millis(200));
            }
        }
        _ => usage(),
    }
    s.shutdown();
    std::thread::sleep(Duration::from_millis(300));
    std::process::exit(0);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::thread::spawn(move || run(args));
    soundcore_bt::run_main_loop();
}
