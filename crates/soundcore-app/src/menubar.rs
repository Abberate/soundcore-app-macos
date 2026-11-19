//! Menu bar presence: a status item whose menu shows the device and offers quick
//! actions, and a main window that hides to the menu bar instead of quitting.
//!
//! * The red close button hides the window and removes the Dock icon; the app keeps
//!   running in the menu bar.
//! * "Open" brings the window and the Dock icon back.
//! * "Quit" (and ⌘Q) really quit.
//!
//! The menu is rebuilt each time it opens, from a model the app provides, so it always
//! shows the current state. All AppKit calls happen on the main thread.

use std::sync::atomic::{AtomicBool, Ordering};

/// Set when the user asked to quit: the next close request is let through.
static QUITTING: AtomicBool = AtomicBool::new(false);

pub fn quitting() -> bool {
    QUITTING.load(Ordering::SeqCst)
}

/// One line of the menu.
pub enum Line {
    /// Greyed-out information (device, battery).
    Info(String),
    /// A choice; `action` is handed back to the app when picked.
    Item {
        title: String,
        checked: bool,
        enabled: bool,
        action: u32,
    },
    Separator,
}

/// What the menu shows; "Open" and "Quit" are added at the bottom.
pub struct Model {
    pub lines: Vec<Line>,
    pub open: String,
    pub quit: String,
}

pub type Provider = Box<dyn Fn() -> Model>;
pub type Handler = Box<dyn Fn(u32)>;

#[cfg(target_os = "macos")]
mod imp {
    use std::cell::RefCell;
    use std::sync::atomic::Ordering;

    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol, ProtocolObject, Sel};
    use objc2::{
        AllocAnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel,
    };
    use objc2_app_kit::{
        NSApplication, NSApplicationActivationPolicy, NSControlStateValueOff,
        NSControlStateValueOn, NSImage, NSMenu, NSMenuDelegate, NSMenuItem, NSStatusBar,
        NSStatusItem, NSVariableStatusItemLength,
    };
    use objc2_foundation::{NSData, NSSize, NSString};

    use super::{Handler, Line, Provider, QUITTING};

    struct Ivars {
        provider: Provider,
        handler: Handler,
        ctx: egui::Context,
    }

    define_class!(
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "SoundcoreAppMenuBarTarget"]
        #[ivars = Ivars]
        struct Target;

        impl Target {
            #[unsafe(method(pick:))]
            fn pick(&self, item: &NSMenuItem) {
                (self.ivars().handler)(item.tag() as u32);
                self.ivars().ctx.request_repaint();
            }

            #[unsafe(method(openWindow:))]
            fn open_window(&self, _sender: Option<&AnyObject>) {
                show_window();
                self.ivars().ctx.request_repaint();
            }

            #[unsafe(method(quitApp:))]
            fn quit_app(&self, _sender: Option<&AnyObject>) {
                QUITTING.store(true, Ordering::SeqCst);
                // The window must be on screen for the app to process the close.
                show_window();
                self.ivars().ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                self.ivars().ctx.request_repaint();
            }
        }

        unsafe impl NSObjectProtocol for Target {}

        unsafe impl NSMenuDelegate for Target {
            #[unsafe(method(menuNeedsUpdate:))]
            fn menu_needs_update(&self, menu: &NSMenu) {
                self.rebuild(menu);
            }
        }
    );

    impl Target {
        fn create(mtm: MainThreadMarker, ivars: Ivars) -> Retained<Self> {
            let this = Self::alloc(mtm).set_ivars(ivars);
            unsafe { msg_send![super(this), init] }
        }

        fn as_any(&self) -> &AnyObject {
            let obj: &NSObject = self;
            obj
        }

        fn rebuild(&self, menu: &NSMenu) {
            let Some(mtm) = MainThreadMarker::new() else {
                return;
            };
            menu.removeAllItems();
            let model = (self.ivars().provider)();
            let add = |title: &str, action: Option<Sel>, key: &str| {
                let item = unsafe {
                    NSMenuItem::initWithTitle_action_keyEquivalent(
                        NSMenuItem::alloc(mtm),
                        &NSString::from_str(title),
                        action,
                        &NSString::from_str(key),
                    )
                };
                if action.is_some() {
                    unsafe { item.setTarget(Some(self.as_any())) };
                }
                menu.addItem(&item);
                item
            };
            for line in &model.lines {
                match line {
                    Line::Info(text) => add(text, None, "").setEnabled(false),
                    Line::Separator => menu.addItem(&NSMenuItem::separatorItem(mtm)),
                    Line::Item {
                        title,
                        checked,
                        enabled,
                        action,
                    } => {
                        let item = add(title, Some(sel!(pick:)), "");
                        item.setTag(*action as isize);
                        item.setState(if *checked {
                            NSControlStateValueOn
                        } else {
                            NSControlStateValueOff
                        });
                        item.setEnabled(*enabled);
                    }
                }
            }
            menu.addItem(&NSMenuItem::separatorItem(mtm));
            add(&model.open, Some(sel!(openWindow:)), "o");
            add(&model.quit, Some(sel!(quitApp:)), "q");
        }
    }

    /// What must stay alive while the app runs.
    type Installed = (Retained<NSStatusItem>, Retained<NSMenu>, Retained<Target>);

    thread_local! {
        static INSTALLED: RefCell<Option<Installed>> =
            const { RefCell::new(None) };
    }

    /// Adds the status item. `icon_png` is a black-on-transparent glyph (template image).
    pub fn install(
        ctx: &egui::Context,
        icon_png: &[u8],
        provider: Provider,
        handler: Handler,
    ) -> bool {
        let Some(mtm) = MainThreadMarker::new() else {
            return false;
        };
        let target = Target::create(
            mtm,
            Ivars {
                provider,
                handler,
                ctx: ctx.clone(),
            },
        );
        let menu = NSMenu::new(mtm);
        menu.setAutoenablesItems(false);
        menu.setDelegate(Some(ProtocolObject::from_ref(&*target)));
        let item = NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);
        if let Some(button) = item.button(mtm) {
            let data = NSData::with_bytes(icon_png);
            if let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) {
                image.setSize(NSSize::new(18.0, 18.0));
                image.setTemplate(true);
                button.setImage(Some(&image));
            }
        }
        item.setMenu(Some(&menu));
        INSTALLED.with(|s| *s.borrow_mut() = Some((item, menu, target)));
        true
    }

    /// Hides every window and the Dock icon; the status item stays.
    pub fn hide_window() {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let app = NSApplication::sharedApplication(mtm);
        for w in app.windows().iter() {
            w.orderOut(None);
        }
        app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    }

    /// Brings the window and the Dock icon back, in front.
    pub fn show_window() {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
        for w in app.windows().iter() {
            w.makeKeyAndOrderFront(None);
        }
        #[allow(deprecated)]
        app.activateIgnoringOtherApps(true);
    }
}

#[cfg(target_os = "macos")]
pub use imp::{hide_window, install, show_window};

#[cfg(not(target_os = "macos"))]
pub fn install(_: &egui::Context, _: &[u8], _: Provider, _: Handler) -> bool {
    false
}
#[cfg(not(target_os = "macos"))]
pub fn hide_window() {}
#[cfg(not(target_os = "macos"))]
pub fn show_window() {}
