//! Conservative policy for desktop targets without a reconciled host leg.
//! Local webview dragging remains available; global geometry and cursor
//! bridging stay inert until that platform gains an explicit strategy.

use dioxus::prelude::*;

use super::GlobalCapability;

pub(super) fn use_global_capability() -> Signal<GlobalCapability> {
    use_signal(|| GlobalCapability::Unavailable)
}
