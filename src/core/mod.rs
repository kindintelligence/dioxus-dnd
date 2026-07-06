//! Shared primitives: the drag context, id/geometry types, hooks, and the
//! `Draggable`/`DropZone`/`DragOverlay` components every other module builds on.

pub mod components;
pub mod hooks;
pub mod machine;
pub mod model;
pub mod modifiers;
pub(crate) mod platform;
pub mod registry;
pub mod state;
pub mod types;

pub use components::{DndProvider, DragOverlay, Draggable, DropZone, ParentZone};
pub use hooks::{
    client_point, element_point, use_dnd, use_dnd_provider, use_zone_id, use_zone_registry,
};
pub use machine::{transition, GestureEffect, GestureEvent, GesturePhase};
pub use model::{apply_clone_or_move, apply_list_clone_or_move};
pub use modifiers::{apply_modifiers, DragModifier, ModifierCtx};
pub use registry::{ZoneRecord, ZoneRegistry};
pub use state::{DndContext, DragState};
pub use types::{
    effective_effect, DragId, DragInputMode, DragMode, DropEffect, DropOutcome, Point, Rect, ZoneId,
};
