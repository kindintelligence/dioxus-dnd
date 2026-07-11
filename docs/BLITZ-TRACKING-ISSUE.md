# Draft tracking issue: dioxus-native (Blitz) readiness

> Draft only - not filed. File once someone has run the crate under
> dioxus-native and can replace at least one "unknown" below with an
> observation. Numbers like (1)-(13) reference the dependency inventory in
> `docs/RENDERER-CONTRACT.md`, which carries file:line citations for every
> claim here.

---

Dioxus is shipping Blitz/dioxus-native (winit-based, no webview) as a
sibling render target. This crate's core was built renderer-honest - the
in-app drag path deliberately depends on pointer events, mounted-element
measurement and CSS, never on `DataTransfer`, JS eval or browser globals -
so a Blitz port is an audit problem, not a rewrite. This issue tracks that
audit per module.

**This is not a commitment to build a Blitz backend.** The desktop module's
tao/wry glue is explicitly out of scope here (a winit host would be its own
`HostBackend`-shaped effort, owned by the desktop architecture
reconciliation).

## Blocking questions (everything else hangs on these)

1. Does Blitz deliver pointer events with `pointer_id` and `pointerType`,
   including `pointercancel`? (Contract #1)
2. Does `MountedData::get_client_rect` return real layout rects? (#3)
3. Do CSS transitions run, and does `transitionend` fire? (#4 - the one
   dependency whose absence *wedges* state rather than degrading: a settle
   glide that never hears `transitionend` sticks. If Blitz lacks it, we add
   a core-side settle timeout or auto-disable settle.)

## Module map

| Module | Verdict under Blitz | Why |
|---|---|---|
| `core::machine`, `core::modifiers`, `core::model`, `core::registry`, `core::strings`, `core::viewport` | **works as is** | pure Rust, no renderer contact |
| `core::world` + `DragSim` (`test`) | **works as is** | host-neutral by design; headless tests already prove it |
| `core::components` pointer path (`Draggable`, `DropZone`, `DndProvider`, delivery) | **works if #1 + #3 hold** | pointer events in, rect measurement for hit-testing; capture substitute already covers the no-pointer-capture case |
| `core::components::overlay` (ghost + settle) | **works if #4 + #13 hold; needs settle timeout otherwise** | `transitionend` is the settle handshake |
| `core::components::pointer` hold clock (`TouchSense::Auto` long-press) | **degrades gracefully; needs-native-impl to restore** | CSS-animation clock; documented to lose only long-press where animations don't run (#6). A core timer fallback is a small, plausible fix |
| `a11y` reorder buttons | **works as is** | plain click handlers |
| `a11y` live region / keyboard drags | **unknown** | needs `aria-live`→AccessKit mapping (#8) and `tabindex`/`keydown` (#7) |
| `animate` (`FlipItem`) | **degrades to jump-into-place** | inline CSS transitions (#4); already the documented no-`web`-feature caveat |
| `autoscroll` | **works if #3 holds** | scrolls via `MountedData::scroll`, observes via causing events - deliberately no `scroll`-event dependency |
| `sortable`, `grid`, `board`, `tree`, `multiselect`, `canvas` | **works if the pointer path works** | pure consumers of core + data-attribute styling (#9) |
| `files` (OS file drops) | **N/A under Blitz** | HTML5 `DataTransfer` (#11); needs an OS-DnD story in Blitz/winit first |
| `dragout` (drag out to other apps) | **N/A under Blitz** | same |
| `external` + `external::typed` (`serde`) | **N/A under Blitz** | same |
| virtualized-list recipe (README + gallery Archive) | **N/A until Blitz has IntersectionObserver/`onvisible`** | recipe-level only; core unaffected (#10) |
| `debug` (`DndDebugOverlay`) | **works if #13 holds** | fixed-position divs |
| `desktop` (tao/wry glue) | **N/A under Blitz, out of scope** | winit host = separate backend effort |

## Suggested first verification pass

Smallest end-to-end probe: one `DndProvider` + two `DropZone`s + one
`Draggable` + `DragOverlay` under dioxus-native, mouse only. That single
scene answers blocking questions 1-3 and settles the verdict for ~70% of
the table above.
