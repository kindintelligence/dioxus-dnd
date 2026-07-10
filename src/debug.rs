//! **Dev-only** drag-and-drop inspector.
//!
//! [`DndDebugOverlay`] draws every zone registered in a provider as a
//! tinted, labeled outline pinned over the page: acceptance state live
//! while a drag is in flight (rejecting zones dim and go dashed), the
//! hovered zone filled as the pointer or keyboard moves, and a status chip
//! with the registry's view of the world. Everything it shows *is* the
//! registry - if an outline is missing or misplaced, hit-testing sees
//! exactly the same wrong thing, which is the point.
//!
//! This is a development tool: it renders unstyled debug chrome over your
//! UI and its output is not localized. Gate it yourself and keep it out of
//! release builds:
//!
//! ```text
//! DndProvider::<Card> {
//!     if cfg!(debug_assertions) {
//!         DndDebugOverlay::<Card> {}
//!     }
//!     // ... your app ...
//! }
//! ```

use dioxus::prelude::*;

use crate::core::{use_dnd, use_joined_window, use_zone_registry};

/// Draws every registered zone of one payload world as a tinted outline
/// (color derived from the zone id, so it's stable across renders), with
/// the zone's label and id in a tag, live `data-over` highlighting, and
/// per-zone acceptance state while a drag is in flight. Render one per
/// provider, anywhere inside it. **Dev-only** - see the module docs.
///
/// Click-through by design (`pointer-events: none`), so it never changes
/// the interaction it inspects. Zones the registry hasn't measured yet
/// draw no outline; the chip counts them so absence is visible too.
#[component]
pub fn DndDebugOverlay<T: Clone + PartialEq + 'static>(
    /// Internal marker; never set this.
    #[props(default)]
    phantom: std::marker::PhantomData<T>,
) -> Element {
    let _ = phantom;
    let dnd = use_dnd::<T>();
    let joined = use_joined_window::<T>();
    let registry = use_zone_registry::<T>();

    // The core only measures rects at drag start; an inspector wants
    // outlines while idle. Re-measure whenever the zone set changes or a
    // zone's DOM handle arrives. The registry exposes a separate revision
    // for those events so the rect writes this triggers cannot loop.
    use_effect(move || {
        registry.track_mounts();
        registry.refresh_rects();
    });

    let payload = dnd.payload();
    let over = dnd.over();
    let records = registry.records();
    let unmeasured = records.iter().filter(|z| z.rect.is_none()).count();
    let status = match (dnd.dragging(), over) {
        (false, _) => format!("{} zones ({unmeasured} unmeasured) - idle", records.len()),
        (true, Some(z)) => format!("dragging - over zone {}", z.0),
        (true, None) => "dragging - over nothing".to_string(),
    };

    rsx! {
        div {
            "data-dnd-debug": "true",
            style: "position: fixed; inset: 0; pointer-events: none; z-index: 9998; \
                    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;",
            for record in records {
                {
                    let id = record.id;
                    let rect = record.cached_rect();
                    // Stable per-id tint; the multiplier scatters neighbors
                    // around the wheel.
                    let hue = (id.0.wrapping_mul(47)) % 360;
                    let accepts = payload.as_ref().map(|p| record.accepts_payload(p));
                    let is_over = match joined {
                        Some(joined) => joined.is_over(id),
                        None => over == Some(id),
                    };
                    let name = record.label.clone().unwrap_or_else(|| "zone".to_string());
                    rsx! {
                        if let Some(r) = rect {
                            div {
                                key: "{id.0}",
                                "data-debug-zone": "{id.0}",
                                "data-over": if is_over { "true" },
                                "data-accepts": accepts.map(|a| if a { "true" } else { "false" }),
                                style: format!(
                                    "position: fixed; left: {}px; top: {}px; width: {}px; height: {}px; \
                                     box-sizing: border-box; border: 2px {} hsl({hue} 70% 42%); \
                                     background: hsl({hue} 70% 42% / {}); opacity: {};",
                                    r.x, r.y, r.width, r.height,
                                    if accepts == Some(false) { "dashed" } else { "solid" },
                                    if is_over { "0.18" } else { "0.04" },
                                    if accepts == Some(false) { "0.45" } else { "1" },
                                ),
                                span {
                                    style: "position: absolute; top: 0; left: 0; transform: translateY(-100%); \
                                            background: hsl({hue} 70% 42%); color: #fff; font-size: 10px; \
                                            line-height: 1.6; padding: 0 4px; white-space: nowrap;",
                                    "{name} #{id.0}"
                                    if accepts == Some(false) { " - rejects" }
                                    if is_over { " - over" }
                                }
                            }
                        }
                    }
                }
            }
            div {
                "data-debug-status": "true",
                style: "position: fixed; right: 8px; bottom: 8px; background: #1a1a1a; color: #fff; \
                        font-size: 11px; line-height: 1; padding: 6px 8px; border-radius: 6px; \
                        opacity: 0.85;",
                "{status}"
            }
        }
    }
}
