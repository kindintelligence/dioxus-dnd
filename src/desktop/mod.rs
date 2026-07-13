#![doc = include_str!("../../docs/api/multi-window.md")]

mod bridge;
mod feed;
mod platform;
mod provider;

pub use bridge::DragBridge;
pub use feed::use_window_geometry_feed;
pub use provider::MultiWindowProvider;
