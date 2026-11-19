# Soundcore App for macOS (unofficial) — v1.0.0

A native macOS companion app, written in Rust, for **soundcore headphones**. The fully supported model today is the **soundcore Q20i** (A3004).
Its look takes after the headphones' own mobile app: a light, airy layout with white cards and one vivid blue. It also has a dark mode, and it runs in five languages.

> **Unofficial, non-certified app.** It is not developed, affiliated with, endorsed or certified by Anker Innovations or soundcore. Use it at your own risk.

| Controls | Equalizer | Settings |
|---|---|---|
| ![](docs/screenshots/controls.png) | ![](docs/screenshots/equalizer.png) | ![](docs/screenshots/settings.png) |

## Features

* **Battery**: the level reported by the headphones (in steps of 20 %), with a charging indicator.
* **Noise control**: noise cancelling, normal or transparency. Presses on the headphones' mode button show up in the app straight away.
* **Equalizer**:
  * the headphones' 22 presets;
  * your own curve on 8 bands (100 Hz to 12.8 kHz, ±6 dB). You drag the points on the graph, and changes reach the headphones as you drag.
* **Device details**: model, firmware, serial number and Bluetooth address.
* **My devices**: switch between paired headphones, or open the macOS Bluetooth settings to pair new ones.
* **Tips** on the headphones' own buttons and lights.
* **Languages**: English, French, Spanish, German and Italian. The app follows the macOS language by default.
* **Appearance**: System, Light or Dark.
* **Shortcuts**: ⌘1…⌘3 switch tabs; Esc closes a dialog.

## Compatibility

* **macOS 11 Big Sur or later**, on Apple Silicon and Intel Macs (universal binary).
* **Headphones**: the soundcore Q20i is fully supported. Other soundcore models are recognised, and the app says that their settings are not supported yet.

## Install

**From the installer:** open `Soundcore-App-1.0.0.dmg` and drag *Soundcore App* onto *Applications*. The installer window shows the steps, and *Lisez-moi - Read me.txt* gives the details in French and English.

* The app is not signed by an identified Apple developer, so macOS may block the first launch. Go to System Settings > Privacy & Security, click **Open Anyway** and confirm. On macOS 14 or earlier, right-click the app and choose **Open** instead.
* Allow **Bluetooth** access when macOS asks.
* The headphones must be paired with the Mac and connected as an audio device.
* **Only one app at a time can use the headphones' controls.** If the soundcore app is open on your phone, the Mac app explains it and connects as soon as the channel is free.

**From source** (needs Rust via `rustup` and the Xcode command-line tools):

```sh
scripts/bundle.sh       # → dist/Soundcore App.app (native architecture)
scripts/make_dmg.sh     # → dist/Soundcore-App-<version>.dmg (universal installer)
```

`make_dmg.sh` needs both macOS targets: `rustup target add x86_64-apple-darwin aarch64-apple-darwin`.

## CLI: `soundcorectl`

It ships inside `Soundcore App.app/Contents/MacOS/`. You can also run it with `cargo run --bin soundcorectl -- …`:

```sh
soundcorectl list
soundcorectl status | monitor
soundcorectl mode anc|normal|transparency
soundcorectl eq 17                     # preset id (0 = soundcore Signature … 21)
soundcorectl eq custom 3 2 1 0 0 1 2 3 # 8 gains in dB, −6…6
soundcorectl --device aa-bb-cc-dd-ee-ff status
```

## Demo mode and visual checks

```sh
cargo run -- --demo                                          # simulated soundcore Q20i
SOUNDCORE_DEMO=1 SOUNDCORE_LANG=de SOUNDCORE_THEME=dark cargo run
SOUNDCORE_DEMO=1 SOUNDCORE_SCENE=eq cargo run
#   scenes: eq, custom, settings, devices, language, log, connect, waiting, busy, charging
SOUNDCORE_DEMO=1 SOUNDCORE_SCREENSHOT=/tmp/shot.png cargo run  # saves a screenshot, then quits
SOUNDCORE_DEMO=1 SOUNDCORE_CHECK_TEXT=1 cargo run              # reports text that had to shrink
```

Demo and test runs never write to your saved preferences.

## Architecture

```
crates/
  soundcore-protocol/  Pure protocol, no I/O, unit-tested: framing and checksum, reassembly,
                       typed requests and events, equalizer presets, models, DeviceState
  soundcore-bt/        macOS transport: IOBluetooth through objc2 (SDP lookup of the vendor
                       service, RFCOMM channel, writes); `probe` example for diagnostics
  soundcore-app/       egui app (ui/: shell, pages, art, i18n, theme, widgets), session
                       engine, `soundcorectl` CLI, icon generator
```

* **Bluetooth threading.** IOBluetooth runs on top of CoreBluetooth, which delivers its callbacks on the main queue. Every IOBluetooth call is dispatched to the main thread through GCD, and the session engine runs on its own thread.
* **Finding the service.** The headphones advertise a vendor RFCOMM service whose UUID ends with the product number (`0cf12d31-fac3-4553-bd80-d6832e7b3004` on the Q20i). The app reads it from the SDP cache, which also identifies the exact model.
* **Wire format.**
  * To the headphones: `08 ee 00 00 00 | command (2) | length (u16 LE) | body | sum`. From the headphones, the header is `09 ff 00 00 01`.
  * `length` counts the whole frame, and the last byte is the 8-bit sum of all the bytes before it.
  * Writes are acknowledged by an empty frame with the same command. Button presses and battery changes arrive as pushes.

## Credits and licences

* **Protocol:** documented by [OpenSCQ30](https://github.com/Oppzippy/OpenSCQ30) (Oppzippy, GPL-3.0). This app is an independent implementation built on that documentation. Thank you!
* **Code:** **GPL-3.0-or-later** (see `LICENSE`).
* **Font:** Poppins (SIL OFL 1.1).
* **Artwork:** the headphone illustration and the icons are original, drawn in code.
* *soundcore* and *Anker* are trademarks of Anker Innovations.
