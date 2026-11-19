use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicU64, Ordering};

use dispatch2::DispatchQueue;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol};
use objc2::{AllocAnyThread, DefinedClass, MainThreadMarker, define_class, msg_send};
use objc2_foundation::NSString;
use objc2_io_bluetooth::{
    IOBluetoothDevice, IOBluetoothRFCOMMChannel, IOBluetoothRFCOMMChannelDelegate,
    IOBluetoothSDPDataElement, IOBluetoothSDPServiceRecord,
};

use crate::{
    LinkEvent, LinkSink, PairedDevice, SPP_UUID, Service, ServiceInfo, is_vendor_uuid,
    normalize_address,
};

const K_IO_RETURN_SUCCESS: i32 = 0;

// ---------------------------------------------------------------------------------------
// main-thread plumbing
// ---------------------------------------------------------------------------------------

fn is_main_thread() -> bool {
    MainThreadMarker::new().is_some()
}

/// Runs `f` on the main thread (inline when already there, otherwise queued).
fn on_main(f: impl FnOnce() + Send + 'static) {
    if is_main_thread() {
        f();
    } else {
        DispatchQueue::main().exec_async(f);
    }
}

/// Runs `f` on the main thread and waits for its result.
///
/// Never call this while holding a lock the main thread may wait on.
pub fn on_main_sync<R: Send>(f: impl FnOnce() -> R + Send) -> R {
    if is_main_thread() {
        return f();
    }
    let mut out = None;
    DispatchQueue::main().exec_sync(|| out = Some(f()));
    out.expect("main queue closure did not run")
}

/// Runs the main run loop forever (for command-line tools; the GUI uses AppKit's loop).
pub fn run_main_loop() -> ! {
    loop {
        objc2_core_foundation::CFRunLoop::run();
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

// ---------------------------------------------------------------------------------------
// delegate class
// ---------------------------------------------------------------------------------------

struct Ivars {
    link: u64,
    sink: LinkSink,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "SoundcoreAppRfcommDelegate"]
    #[ivars = Ivars]
    struct RfcommDelegate;

    impl RfcommDelegate {
        /// IOBluetoothDevice async callback (`performSDPQuery:`).
        #[unsafe(method(sdpQueryComplete:status:))]
        fn sdp_query_complete(&self, _device: *mut AnyObject, status: i32) {
            let link = self.ivars().link;
            on_sdp_complete(link, status);
        }
    }

    unsafe impl NSObjectProtocol for RfcommDelegate {}

    unsafe impl IOBluetoothRFCOMMChannelDelegate for RfcommDelegate {
        #[unsafe(method(rfcommChannelData:data:length:))]
        unsafe fn rfcomm_data(
            &self,
            _channel: Option<&IOBluetoothRFCOMMChannel>,
            data: *mut c_void,
            length: usize,
        ) {
            if data.is_null() || length == 0 {
                return;
            }
            let bytes = unsafe { std::slice::from_raw_parts(data as *const u8, length) }.to_vec();
            (self.ivars().sink)(LinkEvent::Data(bytes));
        }

        #[unsafe(method(rfcommChannelOpenComplete:status:))]
        unsafe fn rfcomm_open_complete(
            &self,
            channel: Option<&IOBluetoothRFCOMMChannel>,
            status: i32,
        ) {
            let ev = if status == K_IO_RETURN_SUCCESS {
                let id = channel.map(|c| unsafe { c.getChannelID() }).unwrap_or_default();
                LinkEvent::Opened { channel: id }
            } else {
                LinkEvent::Failed(format!("RFCOMM open failed ({})", describe_ioreturn(status)))
            };
            (self.ivars().sink)(ev);
        }

        #[unsafe(method(rfcommChannelClosed:))]
        unsafe fn rfcomm_closed(&self, _channel: Option<&IOBluetoothRFCOMMChannel>) {
            let link = self.ivars().link;
            LINKS.with(|l| {
                if let Some(state) = l.borrow_mut().get_mut(&link) {
                    state.channel = None;
                }
            });
            (self.ivars().sink)(LinkEvent::Closed);
        }
    }
);

impl RfcommDelegate {
    fn new(link: u64, sink: LinkSink) -> Retained<Self> {
        let this = Self::alloc().set_ivars(Ivars { link, sink });
        unsafe { msg_send![super(this), init] }
    }

    fn as_any(&self) -> &AnyObject {
        let obj: &NSObject = self;
        obj
    }
}

// ---------------------------------------------------------------------------------------
// link registry (main thread only)
// ---------------------------------------------------------------------------------------

struct LinkState {
    device: Retained<IOBluetoothDevice>,
    service: Service,
    delegate: Retained<RfcommDelegate>,
    channel: Option<Retained<IOBluetoothRFCOMMChannel>>,
    sdp_retried: bool,
}

thread_local! {
    static LINKS: RefCell<HashMap<u64, LinkState>> = RefCell::new(HashMap::new());
}

static NEXT_LINK: AtomicU64 = AtomicU64::new(1);

fn send(link: u64, ev: LinkEvent) {
    LINKS.with(|l| {
        if let Some(s) = l.borrow().get(&link) {
            (s.delegate.ivars().sink)(ev);
        }
    });
}

fn device_for(address: &str) -> Option<Retained<IOBluetoothDevice>> {
    let ns = NSString::from_str(&normalize_address(address));
    unsafe { IOBluetoothDevice::deviceWithAddressString(Some(&ns)) }
}

/// 128-bit UUIDs of an SDP record's ServiceClassIDList (attribute 0x0001).
///
/// The attribute is a sequence of UUIDs, but IOBluetooth's cache may hand back a lone UUID.
fn service_class_uuids(record: &IOBluetoothSDPServiceRecord) -> Vec<[u8; 16]> {
    fn uuid_of(el: &IOBluetoothSDPDataElement) -> Option<[u8; 16]> {
        let u = unsafe { el.getUUIDValue()?.getUUIDWithLength(16)? };
        <[u8; 16]>::try_from(u.to_vec().as_slice()).ok()
    }
    let Some(attr) = (unsafe { record.getAttributeDataElement(0x0001) }) else {
        return Vec::new();
    };
    match unsafe { attr.getTypeDescriptor() } {
        // UUID
        3 => uuid_of(&attr).into_iter().collect(),
        // data element sequence / alternative
        6 | 7 => unsafe { attr.getArrayValue() }
            .map(|items| {
                items
                    .iter()
                    .filter_map(|o: Retained<AnyObject>| {
                        o.downcast::<IOBluetoothSDPDataElement>().ok()
                    })
                    .filter_map(|el| uuid_of(&el))
                    .collect()
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn records(device: &IOBluetoothDevice) -> Vec<Retained<IOBluetoothSDPServiceRecord>> {
    let Some(list) = (unsafe { device.services() }) else {
        return Vec::new();
    };
    list.iter()
        .filter_map(|o: Retained<AnyObject>| o.downcast::<IOBluetoothSDPServiceRecord>().ok())
        .collect()
}

fn record_channel(record: &IOBluetoothSDPServiceRecord) -> Option<u8> {
    let mut id: u8 = 0;
    (unsafe { record.getRFCOMMChannelID(&mut id) } == K_IO_RETURN_SUCCESS).then_some(id)
}

/// RFCOMM channel of the control service: vendor UUID first, standard SPP otherwise.
fn control_channel(device: &IOBluetoothDevice) -> Option<u8> {
    let recs = records(device);
    let find = |pred: &dyn Fn(&[u8; 16]) -> bool| {
        recs.iter().find_map(|r| {
            service_class_uuids(r)
                .iter()
                .any(pred)
                .then(|| record_channel(r))
                .flatten()
        })
    };
    find(&|u| is_vendor_uuid(u)).or_else(|| find(&|u| *u == SPP_UUID))
}

/// RFCOMM services advertised by a paired device (from the SDP cache).
pub fn services(address: &str) -> Vec<ServiceInfo> {
    let address = address.to_string();
    on_main_sync(move || {
        let Some(device) = device_for(&address) else {
            return Vec::new();
        };
        records(&device)
            .iter()
            .map(|r| ServiceInfo {
                name: unsafe { r.getServiceName() }
                    .map(|s| s.to_string())
                    .unwrap_or_default(),
                uuids: service_class_uuids(r),
                channel: record_channel(r),
            })
            .collect()
    })
}

/// Resolves the channel and starts the async open. Main thread.
fn start_open(link: u64) {
    enum Step {
        Open(Retained<IOBluetoothDevice>, Retained<RfcommDelegate>, u8),
        QuerySdp(Retained<IOBluetoothDevice>, Retained<RfcommDelegate>),
        Fail(String),
    }
    let step = LINKS.with(|l| {
        let mut l = l.borrow_mut();
        let state = l.get_mut(&link)?;
        let channel = match state.service {
            Service::Channel(c) => Some(c),
            Service::Control => control_channel(&state.device),
        };
        Some(match channel {
            Some(c) => Step::Open(state.device.clone(), state.delegate.clone(), c),
            None if !state.sdp_retried => {
                state.sdp_retried = true;
                Step::QuerySdp(state.device.clone(), state.delegate.clone())
            }
            None => Step::Fail("service not found on device (SDP)".into()),
        })
    });
    match step {
        None => {}
        Some(Step::Fail(msg)) => send(link, LinkEvent::Failed(msg)),
        Some(Step::QuerySdp(device, delegate)) => {
            let r = unsafe { device.performSDPQuery(Some(delegate.as_any())) };
            if r != K_IO_RETURN_SUCCESS {
                send(
                    link,
                    LinkEvent::Failed(format!("SDP query failed ({})", describe_ioreturn(r))),
                );
            }
        }
        Some(Step::Open(device, delegate, channel_id)) => {
            let mut channel: Option<Retained<IOBluetoothRFCOMMChannel>> = None;
            let r = unsafe {
                device.openRFCOMMChannelAsync_withChannelID_delegate(
                    Some(&mut channel),
                    channel_id,
                    Some(delegate.as_any()),
                )
            };
            if r != K_IO_RETURN_SUCCESS {
                send(
                    link,
                    LinkEvent::Failed(format!("RFCOMM open failed ({})", describe_ioreturn(r))),
                );
                return;
            }
            LINKS.with(|l| {
                if let Some(state) = l.borrow_mut().get_mut(&link) {
                    state.channel = channel;
                }
            });
        }
    }
}

fn on_sdp_complete(link: u64, status: i32) {
    if status != K_IO_RETURN_SUCCESS {
        send(
            link,
            LinkEvent::Failed(format!("SDP query failed ({})", describe_ioreturn(status))),
        );
        return;
    }
    start_open(link);
}

// ---------------------------------------------------------------------------------------
// public API
// ---------------------------------------------------------------------------------------

/// An RFCOMM link. Cheap handle, usable from any thread; closing happens on drop.
#[derive(Debug)]
pub struct Link {
    id: u64,
}

impl Link {
    /// Starts opening `service` on the paired device at `address`. Progress and data are
    /// reported through `sink` ([`LinkEvent::Opened`], [`LinkEvent::Data`], …).
    pub fn open(address: &str, service: Service, sink: LinkSink) -> Link {
        let id = NEXT_LINK.fetch_add(1, Ordering::Relaxed);
        let address = address.to_string();
        on_main(move || {
            let Some(device) = device_for(&address) else {
                sink(LinkEvent::Failed(format!("unknown device {address}")));
                return;
            };
            let delegate = RfcommDelegate::new(id, sink);
            LINKS.with(|l| {
                l.borrow_mut().insert(
                    id,
                    LinkState {
                        device,
                        service,
                        delegate,
                        channel: None,
                        sdp_retried: false,
                    },
                );
            });
            start_open(id);
        });
        Link { id }
    }

    /// Queues bytes for transmission (split to the channel MTU).
    pub fn write(&self, data: Vec<u8>) {
        let id = self.id;
        on_main(move || {
            let channel = LINKS.with(|l| l.borrow().get(&id).and_then(|s| s.channel.clone()));
            let Some(channel) = channel else {
                send(id, LinkEvent::Failed("write on a closed channel".into()));
                return;
            };
            let mtu = usize::from(unsafe { channel.getMTU() }).max(32);
            for chunk in data.chunks(mtu) {
                let mut buf = chunk.to_vec();
                let r = unsafe {
                    channel.writeSync_length(buf.as_mut_ptr() as *mut c_void, buf.len() as u16)
                };
                if r != K_IO_RETURN_SUCCESS {
                    send(
                        id,
                        LinkEvent::Failed(format!("write failed ({})", describe_ioreturn(r))),
                    );
                    return;
                }
            }
        });
    }

    pub fn close(&self) {
        let id = self.id;
        on_main(move || {
            let state = LINKS.with(|l| l.borrow_mut().remove(&id));
            if let Some(state) = state
                && let Some(channel) = state.channel
            {
                unsafe {
                    let _ = channel.setDelegate(None);
                    channel.closeChannel();
                }
            }
        });
    }
}

impl Drop for Link {
    fn drop(&mut self) {
        self.close();
    }
}

/// Paired devices known to macOS.
pub fn paired_devices() -> Vec<PairedDevice> {
    on_main_sync(|| {
        let Some(list) = (unsafe { IOBluetoothDevice::pairedDevices() }) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for obj in list.iter() {
            let obj: Retained<AnyObject> = obj;
            let Ok(dev) = obj.downcast::<IOBluetoothDevice>() else {
                continue;
            };
            unsafe {
                out.push(PairedDevice {
                    name: dev.name().to_string(),
                    address: dev
                        .addressString()
                        .map(|s| normalize_address(&s.to_string()))
                        .unwrap_or_default(),
                    connected: dev.isConnected(),
                    class_of_device: dev.classOfDevice(),
                });
            }
        }
        out
    })
}

/// Whether the device currently has a baseband (ACL) connection to this Mac.
pub fn is_connected(address: &str) -> bool {
    let address = address.to_string();
    on_main_sync(move || device_for(&address).is_some_and(|d| unsafe { d.isConnected() }))
}

/// Human-readable name of an `IOReturn` code.
pub fn describe_ioreturn(code: i32) -> String {
    let name = match code as u32 {
        0xE00002BC => "kIOReturnError",
        0xE00002C0 => "kIOReturnNoDevice",
        0xE00002C2 => "kIOReturnBadArgument",
        0xE00002C5 => "kIOReturnExclusiveAccess",
        0xE00002CD => "kIOReturnNotOpen",
        0xE00002D5 => "kIOReturnBusy",
        0xE00002D6 => "kIOReturnTimeout",
        0xE00002D8 => "kIOReturnNotPermitted",
        0xE00002E2 => "kIOReturnNotPermitted",
        _ => return format!("{code:#x}"),
    };
    format!("{name} {code:#x}")
}
