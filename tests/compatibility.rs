use std::collections::HashMap;

use dioxus::prelude::Callback;
use dioxus_dnd::core::{
    apply_clone_or_move, apply_list_clone_or_move, DragMode, DragState, DropEffect, DropOutcome,
    Point, PointerKind, Rect, ZoneId, ZoneRecord,
};

fn outcome(payload: u32) -> DropOutcome<u32> {
    DropOutcome {
        payload,
        from: Some(ZoneId(1)),
        to: ZoneId(2),
        effect: DropEffect::Move,
        mode: DragMode::Pointer,
        client: Point::default(),
        element: Point::default(),
        grab: Point::default(),
        edge: None,
    }
}

fn construct_version_three_public_records(on_drop: Callback<DropOutcome<u32>>) {
    let _zone = ZoneRecord::<u32> {
        id: ZoneId(1),
        parent: None,
        label: Some("target".to_string()),
        on_drop,
        accepts: None,
        mounted: None,
        rect: Some(Rect::new(0.0, 0.0, 10.0, 10.0)),
    };

    let _drag = DragState::<u32> {
        payload: Some(1),
        source: Some(ZoneId(1)),
        over: None,
        pointer: Point::default(),
        grab: Point::default(),
        effect: DropEffect::Move,
        mode: DragMode::Pointer,
        pointer_kind: PointerKind::Mouse,
        source_rect: None,
        refocus: None,
        settle: None,
    };
}

#[test]
fn version_three_public_records_remain_constructible() {
    let _contract: fn(Callback<DropOutcome<u32>>) = construct_version_three_public_records;
}

#[test]
fn version_three_model_helpers_still_return_unit() {
    let mut zones = HashMap::from([(ZoneId(1), vec![1]), (ZoneId(2), Vec::new())]);
    let _: () = apply_clone_or_move(&mut zones, outcome(1), |value| *value, |value| value);

    let mut source = vec![2];
    let mut target = Vec::new();
    let _: () = apply_list_clone_or_move(
        Some(&mut source),
        &mut target,
        outcome(2),
        |value| *value,
        |value| value,
    );
}
