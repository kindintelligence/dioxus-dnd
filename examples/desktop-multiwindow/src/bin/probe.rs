//! Display-session probe (plan P0): shows, side by side, what the ENGINE
//! dispatches (raw JS listeners via `document::eval`, straight from
//! WebKit) and what DIOXUS handlers decode for the same input - pointer
//! type, button, buttons mask, isPrimary - plus geometry readouts.
//! Run: `cargo run --bin probe` (add `GDK_BACKEND=x11` for the X11 pass).

use dioxus::desktop::tao::event::{ElementState, Event, WindowEvent};
use dioxus::desktop::{use_wry_event_handler, window, Config, WindowBuilder};
use dioxus::prelude::*;

fn main() {
    dioxus::LaunchBuilder::new()
        .with_cfg(
            Config::new().with_window(
                WindowBuilder::new()
                    .with_title("dioxus-dnd - probe")
                    .with_inner_size(dioxus::desktop::tao::dpi::LogicalSize::new(560.0, 720.0)),
            ),
        )
        .launch(app);
}

fn app() -> Element {
    // Raw engine truth: capture-phase listeners in the page itself, fed
    // back over the eval channel (reliable on desktop).
    let mut engine = use_signal(Vec::<String>::new);
    use_hook(|| {
        spawn(async move {
            let mut eval = document::eval(
                r#"
                const fmt = (t, e) =>
                    `${t} type=${e.pointerType||"?"} btn=${e.button} btns=${e.buttons} prim=${e.isPrimary}`;
                window.addEventListener('pointerdown', e => dioxus.send(fmt('down', e)), true);
                window.addEventListener('pointerup',   e => dioxus.send(fmt('up  ', e)), true);
                let n = 0;
                window.addEventListener('pointermove', e => {
                    if ((n++ % 5) === 0) dioxus.send(fmt('move', e));
                }, true);
                "#,
            );
            while let Ok(msg) = eval.recv::<String>().await {
                let mut log = engine.write();
                log.insert(0, msg);
                log.truncate(12);
            }
        });
    });

    // What dioxus handlers decode for the same input.
    let mut decoded = use_signal(Vec::<String>::new);
    let mut log_decoded = move |tag: &str, evt: &PointerEvent| {
        let line = format!(
            "{tag} type={:?} trigger={:?} held={:?} prim={:?}",
            evt.pointer_type(),
            evt.trigger_button(),
            evt.held_buttons(),
            evt.is_primary(),
        );
        let mut log = decoded.write();
        log.insert(0, line);
        log.truncate(12);
    };
    let mut moves = use_signal(|| 0u32);

    // The tao layer's view of the same input: does CursorMoved keep
    // arriving (with out-of-bounds coordinates) while a button is held and
    // the cursor is outside the window - i.e. can a host-side bridge see
    // what the webview cannot?
    let mut tao_log = use_signal(Vec::<String>::new);
    let mut tao_moves = use_signal(|| 0u32);
    let desktop_for_tao = window();
    use_wry_event_handler(move |event, _| {
        let mut push = |line: String| {
            let mut log = tao_log.write();
            log.insert(0, line);
            log.truncate(10);
        };
        if let Event::WindowEvent { event, .. } = event {
            match event {
                WindowEvent::CursorMoved { position, .. } => {
                    let n = *tao_moves.peek() + 1;
                    tao_moves.set(n);
                    let size = desktop_for_tao.inner_size();
                    let out = position.x < 0.0
                        || position.y < 0.0
                        || position.x > size.width as f64
                        || position.y > size.height as f64;
                    if n % 5 == 0 || out {
                        let cur = desktop_for_tao
                            .cursor_position()
                            .map(|p| (p.x as i32, p.y as i32));
                        push(format!(
                            "CursorMoved ({:.0},{:.0}) out={} cursor_position={:?}",
                            position.x, position.y, out, cur
                        ));
                    }
                }
                WindowEvent::CursorLeft { .. } => push("CursorLeft".into()),
                WindowEvent::CursorEntered { .. } => push("CursorEntered".into()),
                WindowEvent::MouseInput { state, button, .. } => {
                    let s = match state {
                        ElementState::Pressed => "pressed",
                        ElementState::Released => "released",
                        _ => "?",
                    };
                    push(format!("MouseInput {s} {button:?}"));
                }
                _ => {}
            }
        }
    });

    // Geometry: window position/size/scale as tao reports them.
    let desktop = window();
    let geo = format!(
        "inner_position={:?} inner_size={:?} scale={:?} cursor={:?}",
        desktop.inner_position().map(|p| (p.x, p.y)),
        (desktop.inner_size().width, desktop.inner_size().height),
        desktop.scale_factor(),
        desktop.cursor_position().map(|p| (p.x as i32, p.y as i32)),
    );

    let open_second = move |_| {
        window().new_window(
            VirtualDom::new(app),
            Config::new().with_window(
                WindowBuilder::new()
                    .with_title("dioxus-dnd - probe2")
                    .with_inner_size(dioxus::desktop::tao::dpi::LogicalSize::new(560.0, 720.0)),
            ),
        );
    };

    rsx! {
        style {
            "body {{ font: 13px monospace; margin: 0; }} \
             section {{ padding: 8px 12px; border-bottom: 1px solid #ccc; }} \
             h3 {{ margin: 4px 0; font-size: 13px; }} \
             .pad {{ background: #eef; min-height: 130px; padding: 12px; user-select: none; touch-action: none; }}"
        }
        section {
            button { onclick: open_second, "Open second probe window" }
        }
        section { h3 { "geometry (render-time)" } div { "{geo}" } }
        section {
            div {
                class: "pad",
                onpointerdown: move |evt| log_decoded("down", &evt),
                onpointerup: move |evt| log_decoded("up  ", &evt),
                onpointermove: move |evt| {
                    let n = moves() + 1;
                    moves.set(n);
                    if n % 5 == 0 {
                        log_decoded("move", &evt);
                    }
                },
                "press / drag on this pad ({moves} moves seen)"
            }
        }
        section {
            h3 { "engine (raw JS, capture phase)" }
            for line in engine() {
                div { "{line}" }
            }
        }
        section {
            h3 { "dioxus (handler decode)" }
            for line in decoded() {
                div { "{line}" }
            }
        }
        section {
            h3 { "tao (window layer, {tao_moves} moves)" }
            for line in tao_log() {
                div { "{line}" }
            }
        }
    }
}
