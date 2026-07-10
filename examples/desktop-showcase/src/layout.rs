//! The demo filming layout: one `D` keypress snaps every window to its
//! slot, so re-recording the hero GIF is reproducible forever.
//!
//! Each window snaps ITSELF: the keypress bumps a shared epoch signal and
//! every window watches it, because only a window's own runtime may touch
//! its tao handle. Coordinates are physical pixels tuned for the 1920x1200
//! @1.5x filming machine and clamped to the primary monitor elsewhere.

use dioxus::desktop::tao::dpi::{PhysicalPosition, PhysicalSize};
use dioxus::desktop::window;

#[derive(Clone, Copy, PartialEq)]
pub enum WindowRole {
    MissionControl,
    Satellite(u32),
}

pub fn snap(role: WindowRole) {
    let desktop = window();
    let monitor = desktop.current_monitor();
    let bounds = monitor
        .map(|m| (m.size().width as i32, m.size().height as i32))
        .unwrap_or((1920, 1200));

    let ((x, y), (w, h)) = match role {
        WindowRole::MissionControl => ((60, 60), (960, 700)),
        WindowRole::Satellite(n) => {
            let slot = n.saturating_sub(1) as i32;
            ((1080, 60 + slot * 530), (560, 500))
        }
    };
    let x = x.min(bounds.0 - w - 10).max(0);
    let y = y.min(bounds.1 - h - 60).max(0);
    desktop.set_inner_size(PhysicalSize::new(w as u32, h as u32));
    desktop.set_outer_position(PhysicalPosition::new(x, y));
}
