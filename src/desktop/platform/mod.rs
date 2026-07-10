//! The bridge's sealed platform policy and mechanics.
//!
//! Layout policy: a leg lives in a per-OS module only when its MECHANICS
//! are OS-specific (`windows`: WM_INPUT raw input). Legs that are plain
//! tao API and merely HAPPEN to matter on some platforms stay in
//! `fallback` so their implementations cannot drift apart. Linux has two
//! OS-specific responsibilities while its shared legs remain
//! portable: `linux` detects the live Tao backend and, on X11, observes the
//! root pointer's held-button mask so dead-space releases are visible. It
//! disables every global leg on Wayland by policy. `macos` owns the decision
//! to keep using only the portable legs, but labels that strategy
//! runtime-unverified until it is exercised on AppKit/WKWebView.

use dioxus::prelude::*;

use crate::core::{DndContext, JoinedWindow};

// Like `windows` below, `fallback` stays compiled on every desktop target
// so each toolchain type-checks the shared legs; only the Linux and macOS
// policies install them, so elsewhere the module is intentionally uncalled.
#[cfg_attr(not(any(target_os = "linux", target_os = "macos")), allow(dead_code))]
mod fallback;
mod windows;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
mod unsupported;

/// Whether the host exposes the global window geometry and cursor needed
/// by cross-window desktop legs. Linux starts unknown because only Tao's
/// event-loop target can identify the backend actually in use.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum GlobalCapability {
    #[default]
    Unknown,
    Available,
    // Only the Linux (Wayland) and unsupported-target policies report
    // Unavailable; Windows and macOS never construct it.
    #[cfg_attr(any(target_os = "windows", target_os = "macos"), allow(dead_code))]
    Unavailable,
}

impl GlobalCapability {
    pub(super) fn available(self) -> bool {
        self == Self::Available
    }
}

#[cfg(target_os = "linux")]
pub(super) fn use_global_capability() -> Signal<GlobalCapability> {
    linux::use_global_capability()
}
#[cfg(target_os = "macos")]
pub(super) fn use_global_capability() -> Signal<GlobalCapability> {
    macos::use_global_capability()
}
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub(super) fn use_global_capability() -> Signal<GlobalCapability> {
    unsupported::use_global_capability()
}

#[cfg(target_os = "windows")]
pub(super) fn use_global_capability() -> Signal<GlobalCapability> {
    use_signal(|| GlobalCapability::Available)
}

/// Install this target's host-side pointer legs without leaking OS branches
/// into the shared bridge component. The Windows hook is kept compiled on
/// every desktop target so WSL gates still type-check its Tao integration;
/// its handler returns before claiming raw input anywhere but Windows.
pub(super) fn use_pointer_legs<T: Clone + PartialEq + 'static>(
    joined: JoinedWindow<T>,
    ctx: DndContext<T>,
    capability: Signal<GlobalCapability>,
) {
    windows::use_raw_input_leg(joined, ctx);
    let _ = capability;

    #[cfg(target_os = "linux")]
    linux::use_portable_legs(joined, ctx, capability);
    #[cfg(target_os = "macos")]
    macos::use_portable_legs(joined, ctx, capability);
}
