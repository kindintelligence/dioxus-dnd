#![doc = include_str!("../README.md")]
#![allow(non_snake_case)]

pub mod core;

pub mod a11y;
pub mod animate;
pub mod autoscroll;
pub mod board;
pub mod canvas;
pub mod dragout;
pub mod external;
pub mod files;
pub mod grid;
pub mod multiselect;
pub mod pointer;
pub mod sortable;
pub mod tree;

/// One-stop import: `use dioxus_dnd::prelude::*;`
pub mod prelude {
    pub use crate::a11y::{LiveRegion, ReorderButtons};
    pub use crate::animate::FlipItem;
    pub use crate::autoscroll::{edge_delta, AutoScroll, ScrollAxis};
    pub use crate::board::{
        apply_move, BoardColumn, BoardItem, BoardPayload, BoardSlot, ContainerId, MoveEvent,
    };
    pub use crate::canvas::{Bounds, CanvasDrop, CanvasDropZone, SnapGrid};
    pub use crate::core::{
        apply_clone_or_move, apply_list_clone_or_move, apply_modifiers, client_point,
        effective_effect, element_point, transition, use_dnd, use_dnd_provider, use_zone_id,
        use_zone_registry, DndContext, DndProvider, DragId, DragInputMode, DragMode, DragModifier,
        DragOverlay, DragState, Draggable, DropEffect, DropOutcome, DropZone, GestureEffect,
        GestureEvent, GesturePhase, ModifierCtx, Point, Rect, ZoneId, ZoneRecord, ZoneRegistry,
    };
    pub use crate::dragout::{ExternalDragSource, OutboundContent};
    pub use crate::external::{classify, ExternalDrop, ExternalDropZone, ExternalPayload};
    pub use crate::files::{FileDrop, FileDropZone, FileFilter, FileRejection};
    pub use crate::grid::{cell_of, index_of, SortableGrid};
    pub use crate::multiselect::{use_selection, SelectableDraggable, Selection, SelectionCount};
    pub use crate::pointer::PointerDraggable;
    pub use crate::sortable::{
        apply_sort, apply_swap, displacement, Axis, ReorderMode, SortEvent, SortableList,
    };
    pub use crate::tree::{
        intent_from_offset, would_create_cycle, DropIntent, NodeId, TreeDropEvent, TreeNodeTarget,
    };
}
