//! The bridge's platform legs. Each leg is a hook the bridge installs
//! unconditionally; the platform decides at runtime whether it ever
//! fires (raw input only exists on Windows, the portable legs go quiet
//! wherever their events or the global cursor do not).
//!
//! Layout policy: a leg lives in a per-OS module only when its MECHANICS
//! are OS-specific (`windows`: WM_INPUT raw input). Legs that are plain
//! tao API and merely HAPPEN to matter on some platforms stay in
//! `fallback` until their behavior diverges - premature per-OS copies of
//! identical code would drift apart silently.

pub(super) mod fallback;
pub(super) mod windows;
