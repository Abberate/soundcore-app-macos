//! Diagnostics: lists paired devices and the RFCOMM services of one of them, then
//! optionally opens the control channel and sends a state request.
//!
//!   cargo run -p soundcore-bt --example probe                 # paired devices
//!   cargo run -p soundcore-bt --example probe -- <address>    # services (SDP cache)
//!   cargo run -p soundcore-bt --example probe -- <address> --open

use std::sync::mpsc;
use std::time::{Duration, Instant};

use soundcore_bt::{LinkEvent, Service, format_uuid, is_vendor_uuid};

/// Outbound frame: 08 ee 00 00 00 | command (2) | total length (u16 LE) | body | sum.
fn frame(command: [u8; 2], body: &[u8]) -> Vec<u8> {
    let len = (5 + 2 + 2 + body.len() + 1) as u16;
    let mut out = vec![0x08, 0xee, 0x00, 0x00, 0x00, command[0], command[1]];
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(body);
    let sum = out.iter().fold(0u8, |a, b| a.wrapping_add(*b));
    out.push(sum);
    out
}

fn hex(b: &[u8]) -> String {
    b.iter()
        .map(|x| format!("{x:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn run(args: Vec<String>) {
    let Some(address) = args.iter().find(|a| !a.starts_with("--")).cloned() else {
        for d in soundcore_bt::paired_devices() {
            println!(
                "{:<28} {}  connected={}  soundcore={}",
                d.name,
                d.address,
                d.connected,
                d.looks_like_soundcore()
            );
        }
        std::process::exit(0);
    };
    println!("connected: {}", soundcore_bt::is_connected(&address));
    for s in soundcore_bt::services(&address) {
        let uuids: Vec<String> = s
            .uuids
            .iter()
            .map(|u| {
                format!(
                    "{}{}",
                    format_uuid(u),
                    if is_vendor_uuid(u) { " (vendor)" } else { "" }
                )
            })
            .collect();
        println!(
            "service {:?} channel={:?} uuids={:?}",
            s.name, s.channel, uuids
        );
    }
    if !args.iter().any(|a| a == "--open") {
        std::process::exit(0);
    }
    std::thread::sleep(Duration::from_secs(2)); // IOBluetooth warm-up
    let (tx, rx) = mpsc::channel();
    let link = soundcore_bt::Link::open(
        &address,
        Service::Control,
        Box::new(move |e| {
            let _ = tx.send(e);
        }),
    );
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(LinkEvent::Opened { channel }) => {
                println!("opened channel {channel}");
                for cmd in [[0x01, 0x01], [0x01, 0x05]] {
                    let f = frame(cmd, &[]);
                    println!("TX {}", hex(&f));
                    link.write(f);
                    std::thread::sleep(Duration::from_millis(300));
                }
            }
            Ok(LinkEvent::Data(d)) => println!("RX {}", hex(&d)),
            Ok(other) => {
                println!("{other:?}");
                if matches!(other, LinkEvent::Failed(_) | LinkEvent::Closed) {
                    break;
                }
            }
            Err(_) => {}
        }
    }
    link.close();
    std::thread::sleep(Duration::from_millis(300));
    std::process::exit(0);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::thread::spawn(move || run(args));
    soundcore_bt::run_main_loop();
}
