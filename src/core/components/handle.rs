//! Explicit drag activators and interactive-child escape hatches.

use dioxus::prelude::*;

#[derive(Clone, Copy)]
pub(crate) struct ActivatorContext {
    pub pointer: Signal<Option<i32>>,
    pub keyboard: Signal<bool>,
}

/// An accessible button that activates the nearest handle-only
/// [`super::Draggable`].
#[component]
pub fn DragHandle(
    #[props(default = "Drag".to_string())] label: String,
    #[props(default)] disabled: bool,
    #[props(extends = button, extends = GlobalAttributes)] mut attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    let activator = try_use_context::<ActivatorContext>();
    super::protect_attributes(
        &mut attributes,
        &[
            "type",
            "disabled",
            "aria-label",
            "data-dnd-handle",
            "onpointerdown",
            "onkeydown",
        ],
    );
    let style = super::merge_style_invariant_last(
        &mut attributes,
        "touch-action: none; user-select: none;",
        &["touch-action", "user-select"],
    );
    rsx! {
        button {
            r#type: "button",
            disabled,
            aria_label: label,
            "data-dnd-handle": "true",
            style,
            onpointerdown: move |event: PointerEvent| {
                if !super::primary_press(&event) {
                    return;
                }
                if let Some(context) = activator.filter(|_| !disabled) {
                    let mut pointer = context.pointer;
                    pointer.set(Some(event.pointer_id()));
                }
            },
            onkeydown: move |_| {
                if let Some(context) = activator.filter(|_| !disabled) {
                    let mut keyboard = context.keyboard;
                    keyboard.set(true);
                }
            },
            ..attributes,
            {children}
        }
    }
}

/// Stop pointer and keyboard events in an interactive subtree from
/// activating a surface-driven draggable.
#[component]
pub fn NoDrag(
    #[props(extends = span, extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    let mut attributes = attributes;
    super::protect_attributes(
        &mut attributes,
        &["data-no-drag", "onpointerdown", "onkeydown"],
    );
    rsx! {
        span {
            "data-no-drag": "true",
            onpointerdown: move |event: PointerEvent| event.stop_propagation(),
            onkeydown: move |event: KeyboardEvent| event.stop_propagation(),
            ..attributes,
            {children}
        }
    }
}
