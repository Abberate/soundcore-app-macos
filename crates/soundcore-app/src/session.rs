//! Connection engine: runs on its own thread, owns the RFCOMM link and the device
//! state, and publishes a [`Snapshot`] the UI reads every frame.
//!
//! * Opening the link: the vendor control service (channel found through SDP), after a
//!   short warm-up because IOBluetooth refuses channel opens right after launch.
//! * Reading: one full state request, repeated until it is answered.
//! * Writing: one request at a time, each waiting for its empty acknowledgement.
//! * Pushes from the headphones (mode button, battery) update the state as they come.
//! * Reconnection with back-off while the headphones are connected to the Mac.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, unbounded};
use soundcore_bt::{LinkEvent, Service};
use soundcore_protocol::{
    AmbientMode, Command, DeviceState, Equalizer, Event, Frame, Reassembler, SoundModes, frame,
    model, request,
};

const WARM_UP: Duration = Duration::from_secs(2);
const OPEN_TIMEOUT: Duration = Duration::from_secs(6);
const STATE_TIMEOUT: Duration = Duration::from_secs(3);
const ACK_TIMEOUT: Duration = Duration::from_secs(2);
const WAIT_POLL: Duration = Duration::from_secs(2);
const MAX_BACKOFF: Duration = Duration::from_secs(15);
const LOG_LINES: usize = 300;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub name: String,
    pub address: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Phase {
    /// No device chosen.
    Idle,
    /// Device chosen, but not connected to the Mac (switched off, out of range…).
    Waiting,
    Connecting,
    /// Channel open, waiting for the first state answer.
    Loading,
    Ready,
    /// The control channel would not open; another app most likely holds it.
    Busy,
    Failed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notice {
    /// A setting was not acknowledged.
    NoAck,
    /// The link dropped while in use.
    Lost,
}

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub phase: Phase,
    pub target: Option<Target>,
    pub state: DeviceState,
    /// Increments with each notice so the UI shows it once.
    pub notice: Option<(u64, Notice)>,
    pub log: Vec<String>,
    pub demo: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    SetAmbient(AmbientMode),
    SetEqualizer(Equalizer),
    Refresh,
}

enum Msg {
    Connect(Target),
    Disconnect,
    Act(Action),
    Link(u64, LinkEvent),
    Shutdown,
}

#[derive(Clone)]
pub struct SessionHandle {
    tx: Sender<Msg>,
    snap: Arc<Mutex<Snapshot>>,
}

type Repaint = Arc<dyn Fn() + Send + Sync>;

impl SessionHandle {
    pub fn spawn(repaint: impl Fn() + Send + Sync + 'static) -> Self {
        Self::start(Arc::new(repaint), false)
    }

    /// A simulated soundcore Q20i (screenshots, trying the app without headphones).
    pub fn spawn_demo(repaint: impl Fn() + Send + Sync + 'static) -> Self {
        Self::start(Arc::new(repaint), true)
    }

    fn start(repaint: Repaint, demo: bool) -> Self {
        let (tx, rx) = unbounded();
        let snap = Arc::new(Mutex::new(Snapshot {
            phase: Phase::Idle,
            target: None,
            state: DeviceState::default(),
            notice: None,
            log: Vec::new(),
            demo,
        }));
        let engine = Engine::new(snap.clone(), repaint, tx.clone(), demo);
        std::thread::Builder::new()
            .name("soundcore-session".into())
            .spawn(move || engine.run(rx))
            .expect("session thread");
        Self { tx, snap }
    }

    pub fn snapshot(&self) -> Snapshot {
        self.snap.lock().expect("snapshot").clone()
    }

    pub fn connect(&self, target: Target) {
        let _ = self.tx.send(Msg::Connect(target));
    }

    pub fn disconnect(&self) {
        let _ = self.tx.send(Msg::Disconnect);
    }

    pub fn act(&self, a: Action) {
        let _ = self.tx.send(Msg::Act(a));
    }

    pub fn shutdown(&self) {
        let _ = self.tx.send(Msg::Shutdown);
    }
}

struct Engine {
    snap: Arc<Mutex<Snapshot>>,
    repaint: Repaint,
    tx: Sender<Msg>,
    demo: bool,
    started: Instant,

    target: Option<Target>,
    phase: Phase,
    state: DeviceState,
    link: Option<soundcore_bt::Link>,
    generation: u64,
    reasm: Reassembler,

    open_deadline: Option<Instant>,
    state_sent: Option<Instant>,
    state_tries: u8,
    retry_at: Option<Instant>,
    backoff: Duration,

    queue: VecDeque<(Command, Vec<u8>)>,
    inflight: Option<(Command, Instant)>,
    notice_seq: u64,
    notice: Option<(u64, Notice)>,
    log: VecDeque<String>,
    dirty: bool,
}

fn hex(b: &[u8]) -> String {
    b.iter()
        .map(|x| format!("{x:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

impl Engine {
    fn new(snap: Arc<Mutex<Snapshot>>, repaint: Repaint, tx: Sender<Msg>, demo: bool) -> Self {
        Self {
            snap,
            repaint,
            tx,
            demo,
            started: Instant::now(),
            target: None,
            phase: Phase::Idle,
            state: DeviceState::default(),
            link: None,
            generation: 0,
            reasm: Reassembler::new(),
            open_deadline: None,
            state_sent: None,
            state_tries: 0,
            retry_at: None,
            backoff: Duration::from_secs(2),
            queue: VecDeque::new(),
            inflight: None,
            notice_seq: 0,
            notice: None,
            log: VecDeque::new(),
            dirty: true,
        }
    }

    fn run(mut self, rx: Receiver<Msg>) {
        if self.demo {
            self.demo_connect();
        }
        loop {
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(Msg::Shutdown) => {
                    self.close_link();
                    return;
                }
                Ok(m) => self.handle(m),
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => return,
            }
            self.tick();
            self.publish();
        }
    }

    fn log(&mut self, line: impl Into<String>) {
        let line = line.into();
        log::debug!("{line}");
        if self.log.len() == LOG_LINES {
            self.log.pop_front();
        }
        let t = self.started.elapsed().as_secs_f32();
        self.log.push_back(format!("{t:8.2}  {line}"));
        self.dirty = true;
    }

    fn set_phase(&mut self, p: Phase) {
        if self.phase != p {
            self.log(format!("phase {p:?}"));
            self.phase = p;
            self.dirty = true;
        }
    }

    fn notify(&mut self, n: Notice) {
        self.notice_seq += 1;
        self.notice = Some((self.notice_seq, n));
        self.dirty = true;
    }

    fn publish(&mut self) {
        if !self.dirty {
            return;
        }
        self.dirty = false;
        {
            let mut s = self.snap.lock().expect("snapshot");
            s.phase = self.phase.clone();
            s.target = self.target.clone();
            s.state = self.state.clone();
            s.notice = self.notice;
            s.log = self.log.iter().cloned().collect();
        }
        (self.repaint)();
    }

    // ---------------------------------------------------------------------------------
    // messages
    // ---------------------------------------------------------------------------------

    fn handle(&mut self, m: Msg) {
        match m {
            Msg::Connect(t) => {
                if self.demo {
                    return;
                }
                self.close_link();
                self.log(format!("connect {} ({})", t.name, t.address));
                self.state = DeviceState::new(model::from_name(&t.name));
                self.target = Some(t);
                self.backoff = Duration::from_secs(2);
                self.retry_at = Some(Instant::now());
                self.set_phase(Phase::Waiting);
            }
            Msg::Disconnect => {
                if self.demo {
                    return;
                }
                self.close_link();
                self.target = None;
                self.retry_at = None;
                self.state = DeviceState::default();
                self.set_phase(Phase::Idle);
            }
            Msg::Act(a) => self.act(a),
            Msg::Link(generation, ev) if generation == self.generation => self.on_link(ev),
            Msg::Link(..) | Msg::Shutdown => {}
        }
    }

    fn act(&mut self, a: Action) {
        match a {
            Action::SetAmbient(mode) => {
                let modes = self
                    .state
                    .sound_modes
                    .unwrap_or_else(|| SoundModes::new(mode))
                    .with_ambient(mode);
                self.state.set_sound_modes(modes);
                self.write(request::sound_modes(&modes));
            }
            Action::SetEqualizer(eq) => {
                self.state.set_equalizer(eq);
                // Only the latest curve matters while a slider is dragged.
                self.queue
                    .retain(|(c, _)| *c != soundcore_protocol::cmd::SET_EQUALIZER_APPLIED);
                self.write(request::equalizer(&eq));
            }
            Action::Refresh => {
                if self.phase == Phase::Ready {
                    self.send(request::state());
                } else if self.target.is_some() && !self.demo {
                    self.close_link();
                    self.retry_at = Some(Instant::now());
                    self.set_phase(Phase::Waiting);
                }
            }
        }
        self.dirty = true;
    }

    fn write(&mut self, bytes: Vec<u8>) {
        if self.demo {
            return;
        }
        let command = [bytes[5], bytes[6]];
        self.queue.push_back((command, bytes));
    }

    fn send(&mut self, bytes: Vec<u8>) {
        if self.link.is_none() {
            return;
        }
        self.log(format!("TX {}", hex(&bytes)));
        if let Some(link) = &self.link {
            link.write(bytes);
        }
    }

    // ---------------------------------------------------------------------------------
    // link
    // ---------------------------------------------------------------------------------

    fn open_link(&mut self) {
        let Some(t) = self.target.clone() else { return };
        // Exact model from the vendor service UUID when macOS has it cached.
        for s in soundcore_bt::services(&t.address) {
            if let Some(m) = s.uuids.iter().find_map(model::from_vendor_uuid) {
                self.state.model = Some(m);
            }
        }
        self.generation += 1;
        let generation = self.generation;
        let tx = self.tx.clone();
        self.reasm = Reassembler::new();
        self.link = Some(soundcore_bt::Link::open(
            &t.address,
            Service::Control,
            Box::new(move |ev| {
                let _ = tx.send(Msg::Link(generation, ev));
            }),
        ));
        self.open_deadline = Some(Instant::now() + OPEN_TIMEOUT);
        self.set_phase(Phase::Connecting);
    }

    fn close_link(&mut self) {
        if let Some(link) = self.link.take() {
            link.close();
        }
        self.generation += 1;
        self.open_deadline = None;
        self.state_sent = None;
        self.queue.clear();
        self.inflight = None;
    }

    fn retry_later(&mut self, phase: Phase) {
        self.close_link();
        self.set_phase(phase);
        self.retry_at = Some(Instant::now() + self.backoff);
        self.backoff = (self.backoff * 2).min(MAX_BACKOFF);
    }

    fn on_link(&mut self, ev: LinkEvent) {
        match ev {
            LinkEvent::Opened { channel } => {
                self.log(format!("channel {channel} open"));
                self.open_deadline = None;
                self.state_tries = 0;
                self.set_phase(Phase::Loading);
                self.request_state();
            }
            LinkEvent::Data(bytes) => {
                self.log(format!("RX {}", hex(&bytes)));
                for f in self.reasm.push(&bytes) {
                    self.on_frame(f);
                }
            }
            LinkEvent::Closed => {
                self.log("channel closed");
                let was_ready = self.phase == Phase::Ready;
                self.link = None;
                if was_ready {
                    self.notify(Notice::Lost);
                }
                self.backoff = Duration::from_secs(2);
                self.retry_later(Phase::Waiting);
            }
            LinkEvent::Failed(msg) => {
                self.log(format!("failed: {msg}"));
                self.link = None;
                let busy =
                    msg.contains("0x38f") || msg.contains("Exclusive") || msg.contains("Busy");
                let phase = if busy {
                    Phase::Busy
                } else {
                    Phase::Failed(msg)
                };
                self.retry_later(phase);
            }
        }
    }

    fn request_state(&mut self) {
        self.state_tries += 1;
        self.state_sent = Some(Instant::now());
        self.send(request::state());
    }

    fn on_frame(&mut self, f: Frame) {
        let ev = Event::parse(&f);
        match &ev {
            Event::Ack(c) => {
                if self.inflight.is_some_and(|(i, _)| i == *c) {
                    self.inflight = None;
                }
            }
            Event::State(_) => {
                self.state_sent = None;
                self.backoff = Duration::from_secs(2);
                self.set_phase(Phase::Ready);
            }
            Event::Unknown(f) => self.log(format!(
                "unhandled {:02x}{:02x}",
                f.command[0], f.command[1]
            )),
            _ => {}
        }
        if self.state.apply(&ev) {
            self.dirty = true;
        }
    }

    fn tick(&mut self) {
        let now = Instant::now();
        if self.demo {
            return;
        }
        // (Re)connect.
        if self.link.is_none()
            && let (Some(t), Some(at)) = (self.target.clone(), self.retry_at)
            && now >= at
        {
            if !soundcore_bt::is_connected(&t.address) {
                self.set_phase(Phase::Waiting);
                self.retry_at = Some(now + WAIT_POLL);
            } else if self.started.elapsed() < WARM_UP {
                self.retry_at = Some(self.started + WARM_UP);
            } else {
                self.retry_at = None;
                self.open_link();
            }
        }
        // Channel never opened: someone else holds it.
        if self.open_deadline.is_some_and(|d| now >= d) {
            self.log("no answer: channel probably in use by another app");
            self.retry_later(Phase::Busy);
        }
        // State answer missing.
        if self
            .state_sent
            .is_some_and(|s| now.duration_since(s) >= STATE_TIMEOUT)
        {
            if self.state_tries < 3 {
                self.request_state();
            } else {
                self.retry_later(Phase::Failed("no state answer".into()));
            }
        }
        // Writes, one at a time.
        if let Some((_, sent)) = self.inflight
            && now.duration_since(sent) >= ACK_TIMEOUT
        {
            self.inflight = None;
            self.log("write not acknowledged");
            self.notify(Notice::NoAck);
        }
        if self.inflight.is_none()
            && self.phase == Phase::Ready
            && let Some((c, bytes)) = self.queue.pop_front()
        {
            self.inflight = Some((c, now));
            self.send(bytes);
        }
    }

    // ---------------------------------------------------------------------------------
    // demo
    // ---------------------------------------------------------------------------------

    fn demo_connect(&mut self) {
        let scene = std::env::var("SOUNDCORE_SCENE").unwrap_or_default();
        self.target = Some(Target {
            name: "soundcore Q20i".into(),
            address: "00-00-00-00-00-00".into(),
        });
        self.state = DeviceState::new(model::by_id("A3004"));
        let mut body = vec![4, u8::from(scene == "charging")];
        body.extend_from_slice(b"01.10");
        body.extend_from_slice(b"3004DEMO00000000");
        let eq = if scene == "custom" {
            Equalizer::custom([40, 30, 10, 0, -10, 10, 30, 20])
        } else {
            Equalizer::default()
        };
        body.extend_from_slice(&eq.bytes());
        body.extend_from_slice(&SoundModes::new(AmbientMode::NoiseCanceling).bytes());
        let bytes = frame::encode_from_device(soundcore_protocol::cmd::STATE, &body);
        for f in Reassembler::new().push(&bytes) {
            self.state.apply(&Event::parse(&f));
        }
        match scene.as_str() {
            "waiting" => self.phase = Phase::Waiting,
            "busy" => self.phase = Phase::Busy,
            _ => self.phase = Phase::Ready,
        }
        self.log("demo device ready");
    }
}
