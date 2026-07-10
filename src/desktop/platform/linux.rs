//! Linux backend policy. Portable Tao cursor polling and window-event release
//! handling remain in `fallback`; this module owns the genuinely Linux/X11
//! pieces: selecting the live backend and querying X11's primary-button mask
//! so a release over dead space cannot strand a drag. Tao's live event-loop
//! target is authoritative: X11 exposes global window geometry and a global
//! cursor, while Wayland deliberately exposes neither. WSLg follows whichever
//! backend Tao actually selected and has no special runtime branch.
//!
//! The pointer query rides a first-party x11rb connection rather than tao's
//! re-exported xlib FFI: that re-export is not part of tao's semver contract,
//! so a tao minor could strand this leg mid-2.x. A second client connection
//! to the X server is ordinary (xdotool works this way) and observation-only;
//! it never changes server state, so it cannot fight tao's own connection.

use std::sync::OnceLock;

use dioxus::prelude::*;
use dioxus_desktop::tao::platform::unix::EventLoopWindowTargetExtUnix;
use dioxus_desktop::use_wry_event_handler;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConnectionExt, KeyButMask, Window};
use x11rb::rust_connection::RustConnection;

use crate::core::{DndContext, JoinedWindow};

use super::super::bridge::{subscribed_generation, BridgeGeneration};
use super::{fallback, GlobalCapability};

#[derive(Clone, Copy, Debug, PartialEq)]
struct X11PointerSample {
    global: crate::core::Point,
    primary_pressed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum X11ReleaseAction {
    Wait,
    ObservePressed,
    Release,
}

fn x11_release_action(
    pressed_generation: Option<BridgeGeneration>,
    current_generation: BridgeGeneration,
    primary_pressed: bool,
) -> X11ReleaseAction {
    if primary_pressed {
        X11ReleaseAction::ObservePressed
    } else if pressed_generation == Some(current_generation) {
        X11ReleaseAction::Release
    } else {
        X11ReleaseAction::Wait
    }
}

/// The sampling connection plus the root window it queries. One per process:
/// every window's dead-space leg samples the same global pointer, and an X
/// connection per window would multiply file descriptors for no information.
struct X11Pointer {
    connection: RustConnection,
    root: Window,
}

/// Connect once, verdict cached for the process lifetime. `None` means the
/// display could not be opened at all (no X server, refused auth) - the leg
/// then simply never engages, exactly as when tao had no X connection to
/// lend. It is NOT a backend verdict: capability selection stays with tao's
/// `is_wayland()`, and callers only reach here after that gate said X11.
fn x11_pointer() -> Option<&'static X11Pointer> {
    static POINTER: OnceLock<Option<X11Pointer>> = OnceLock::new();
    POINTER
        .get_or_init(|| {
            let (connection, screen) = x11rb::connect(None).ok()?;
            let root = connection.setup().roots.get(screen)?.root;
            Some(X11Pointer { connection, root })
        })
        .as_ref()
}

fn query_x11_pointer(pointer: &X11Pointer) -> Option<X11PointerSample> {
    // A failed request or reply is a transient sample miss, retried by the
    // caller's loop - never a reason to reclassify the backend.
    let reply = pointer
        .connection
        .query_pointer(pointer.root)
        .ok()?
        .reply()
        .ok()?;
    Some(X11PointerSample {
        global: crate::core::Point::new(f64::from(reply.root_x), f64::from(reply.root_y)),
        primary_pressed: u16::from(reply.mask) & u16::from(KeyButMask::BUTTON1) != 0,
    })
}

fn global_capability_for_backend(is_wayland: bool) -> GlobalCapability {
    if is_wayland {
        GlobalCapability::Unavailable
    } else {
        GlobalCapability::Available
    }
}

pub(super) fn use_global_capability() -> Signal<GlobalCapability> {
    let mut capability = use_signal(GlobalCapability::default);
    use_wry_event_handler(move |_, target| {
        if *capability.peek() == GlobalCapability::Unknown {
            // The first event-loop target owns the immutable backend decision;
            // API failures later in the session cannot reclassify X11 as Wayland.
            capability.set(global_capability_for_backend(target.is_wayland()));
        }
    });
    capability
}

pub(super) fn use_portable_legs<T: Clone + PartialEq + 'static>(
    joined: JoinedWindow<T>,
    ctx: DndContext<T>,
    capability: Signal<GlobalCapability>,
) {
    fallback::use_cursor_poller_leg(joined, ctx, capability);
    fallback::use_release_leg(joined, ctx, capability);
    use_x11_dead_space_release(joined, ctx, capability);
}

fn use_x11_dead_space_release<T: Clone + PartialEq + 'static>(
    joined: JoinedWindow<T>,
    ctx: DndContext<T>,
    capability: Signal<GlobalCapability>,
) {
    let mut x11_confirmed = use_signal(|| false);
    let mut pressed_generation = use_signal(|| None::<BridgeGeneration>);
    use_wry_event_handler(move |_, target| {
        if !*x11_confirmed.peek() && !target.is_wayland() {
            // Tao's live event-loop target owns the backend verdict; only a
            // confirmed X11 session may ever open the sampling connection.
            // Under XWayland an X connect would succeed anyway, so gating on
            // the connect instead of on tao would misclassify Wayland.
            x11_confirmed.set(true);
        }
    });

    let _release_observer = use_resource(move || {
        let connection = x11_confirmed().then(x11_pointer).flatten();
        let generation = subscribed_generation(joined);
        let should_watch = generation.is_some_and(|generation| {
            fallback::poller_owns_generation(joined, &ctx, capability, generation)
        });
        // Query synchronously only after this composite generation owns every
        // bridge gate (including mouse/pen rather than touch). This observes
        // the initiating press without a first 30 ms blind interval; a
        // transient miss is retried by the async loop.
        let first_sample = if should_watch {
            connection.and_then(query_x11_pointer)
        } else {
            None
        };
        async move {
            let Some((connection, generation)) =
                connection.zip(generation.filter(|_| should_watch))
            else {
                return;
            };
            let mut first_sample = first_sample;
            loop {
                if !fallback::poller_owns_generation(joined, &ctx, capability, generation) {
                    break;
                }
                let sample = match first_sample.take() {
                    Some(sample) => sample,
                    None => {
                        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
                        if !fallback::poller_owns_generation(joined, &ctx, capability, generation) {
                            break;
                        }
                        let Some(sample) = query_x11_pointer(connection) else {
                            continue;
                        };
                        sample
                    }
                };
                let pressed_generation_now = *pressed_generation.peek();
                match x11_release_action(pressed_generation_now, generation, sample.primary_pressed)
                {
                    X11ReleaseAction::Wait => {}
                    X11ReleaseAction::ObservePressed => {
                        if pressed_generation_now != Some(generation) {
                            // Press evidence lives across task restarts but is
                            // keyed to this composite generation. A modifier
                            // update retains ownership; drag N+1 cannot use it.
                            pressed_generation.set(Some(generation));
                        }
                    }
                    X11ReleaseAction::Release => {
                        // Retire this run's proof before delivery invokes user
                        // code. A synchronously started N+1 can then install
                        // its own proof without this N callback erasing it.
                        pressed_generation.set(None);
                        // The same captured generation owns both the observed
                        // press and this final mutation; completion or a new
                        // begin invalidates it immediately before the call.
                        if fallback::poller_owns_generation(joined, &ctx, capability, generation) {
                            joined.world.drop_at_global(sample.global);
                        }
                        break;
                    }
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_capability_selection_is_explicit() {
        assert_eq!(
            global_capability_for_backend(true),
            GlobalCapability::Unavailable
        );
        assert_eq!(
            global_capability_for_backend(false),
            GlobalCapability::Available
        );
    }

    #[test]
    fn x11_release_requires_a_press_owned_by_the_same_run() {
        let drag_n = BridgeGeneration {
            world: 9,
            session: None,
        };
        let drag_n_plus_one = BridgeGeneration {
            world: 10,
            session: None,
        };
        assert_eq!(
            x11_release_action(None, drag_n, false),
            X11ReleaseAction::Wait
        );
        assert_eq!(
            x11_release_action(None, drag_n, true),
            X11ReleaseAction::ObservePressed
        );
        assert_eq!(
            x11_release_action(Some(drag_n), drag_n, true),
            X11ReleaseAction::ObservePressed
        );
        assert_eq!(
            x11_release_action(Some(drag_n), drag_n, false),
            X11ReleaseAction::Release
        );
        assert_eq!(
            x11_release_action(Some(drag_n), drag_n_plus_one, false),
            X11ReleaseAction::Wait
        );
    }
}
