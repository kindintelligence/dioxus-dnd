//! Shared primitives: the drag context, id/geometry types, hooks, and the
//! `Draggable`/`DropZone`/`DragOverlay` components every other module builds on.

pub mod components;
pub mod hooks;
pub mod machine;
pub mod model;
pub mod modifiers;
pub(crate) mod platform;
pub mod registry;
mod session;
pub mod state;
pub mod strings;
pub mod types;
pub mod viewport;
pub mod world;

pub use components::{
    BridgeDropZone, DndProvider, DragOverlay, Draggable, DropZone, ParentZone, SettleSlot,
};
pub use hooks::{
    client_point, element_point, use_bridge_world, use_dnd, use_dnd_provider, use_rect_refresh,
    use_zone_id, use_zone_registry, BridgeGeometry, BridgeWorld,
};
pub use machine::{
    transition, transition_with, GestureEffect, GestureEvent, GesturePhase, Promotion,
};
pub use model::{apply_clone_or_move, apply_list_clone_or_move};
pub use modifiers::{apply_modifiers, DragModifier, ModifierCtx};
pub use registry::{RectRefresh, ZoneRecord, ZoneRegistration, ZoneRegistry};
pub use state::{DndContext, DragState};
pub use strings::{use_dnd_strings, DndStrings};
pub use types::{
    edge_of, effective_effect, Direction, DragId, DragMode, DragSessionId, DropEffect, DropOutcome,
    Edge, EdgeSet, Point, PointerKind, Rect, TouchSense, ZoneId,
};
pub use viewport::{
    screen_delta_to_world, screen_to_world, world_delta_to_screen, world_to_screen, CanvasViewport,
};
pub use world::{
    use_dnd_world, use_joined_window, DndWorld, JoinedWindow, WindowGeometry, WindowKey,
    WindowRecord,
};
