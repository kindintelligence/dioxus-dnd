# dioxus-dnd documentation

Two layers, paired by name:

- **`concepts/`** teaches. One guide per concept: the mental model, a worked
  example, what you get for free, the traps.
- **`api/`** specifies. One reference per concept: every component, prop,
  hook, type and function, with types and defaults verified against source.

Start with the guide, keep the API reference open while you build. Every
guide links to its reference and back.

## The map

### Start here

| Concept | Guide | API reference |
|---|---|---|
| Drag and drop | [concepts/drag-and-drop.md](concepts/drag-and-drop.md) | [api/drag-and-drop.md](api/drag-and-drop.md) |
| Architecture | [concepts/architecture.md](concepts/architecture.md) | [api/core.md](api/core.md) |

### Foundations (cross-cutting, read once)

| Concept | Guide | API reference |
|---|---|---|
| Styling | [concepts/styling.md](concepts/styling.md) | contract table in the guide |
| Accessibility | [concepts/accessibility.md](concepts/accessibility.md) | [api/accessibility.md](api/accessibility.md) |
| Localization | [concepts/localization.md](concepts/localization.md) | [api/localization.md](api/localization.md) |
| Touch and input | [concepts/touch-and-input.md](concepts/touch-and-input.md) | props in [api/drag-and-drop.md](api/drag-and-drop.md) |
| Auto-scroll | [concepts/autoscroll.md](concepts/autoscroll.md) | [api/autoscroll.md](api/autoscroll.md) |
| Drop effects | [concepts/drop-effects.md](concepts/drop-effects.md) | [api/drop-effects.md](api/drop-effects.md) |

### Patterns (jump to what you need)

| Concept | Guide | API reference |
|---|---|---|
| Sortable lists and grids | [concepts/sortable-lists.md](concepts/sortable-lists.md) | [api/sortable-lists.md](api/sortable-lists.md) |
| Boards (kanban) | [concepts/boards.md](concepts/boards.md) | [api/boards.md](api/boards.md) |
| Trees | [concepts/trees.md](concepts/trees.md) | [api/trees.md](api/trees.md) |
| Canvas | [concepts/canvas.md](concepts/canvas.md) | [api/canvas.md](api/canvas.md) |
| Multi-select | [concepts/multiselect.md](concepts/multiselect.md) | [api/multiselect.md](api/multiselect.md) |
| File drops | [concepts/file-drops.md](concepts/file-drops.md) | [api/file-drops.md](api/file-drops.md) |
| External content in | [concepts/external-content.md](concepts/external-content.md) | [api/external-content.md](api/external-content.md) |
| Dragging out | [concepts/drag-out.md](concepts/drag-out.md) | [api/drag-out.md](api/drag-out.md) |

### Composing

| Concept | Guide | API reference |
|---|---|---|
| Mixing payload types | [concepts/mixing-payload-types.md](concepts/mixing-payload-types.md) | [api/mixing-payload-types.md](api/mixing-payload-types.md) |
| Multi-window desktop drags | [concepts/multi-window.md](concepts/multi-window.md) | [api/multi-window.md](api/multi-window.md) |
| Virtualized lists | [concepts/virtualized-lists.md](concepts/virtualized-lists.md) | registry section of [api/core.md](api/core.md) |

### Workshop

| Concept | Guide | API reference |
|---|---|---|
| Animation | [concepts/animation.md](concepts/animation.md) | [api/animation.md](api/animation.md) |
| Testing | [concepts/testing.md](concepts/testing.md) | [api/testing.md](api/testing.md) |
| Debugging | [concepts/debugging.md](concepts/debugging.md) | [api/debugging.md](api/debugging.md) |

Every pattern also runs live in the
[gallery](https://kindintelligence.github.io/dioxus-dnd/); each guide names
its demo page.

Maintainers preparing a crate release should follow the checked and automated
[release process](../RELEASING.md).

## Linting the documentation

CI checks the public documentation with
[rumdl](https://github.com/rvben/rumdl). Install the same pinned version and
run it from the repository root before opening a pull request:

```console
cargo install rumdl --locked --version 0.2.30
rumdl check .
```

The repository configuration excludes `docs/superpowers/`, whose internal
plans and specifications follow their own formatting conventions.

## How the API references stay honest

Each `api/*.md` file is also the rustdoc for its module: the module's source
file starts with `#![doc = include_str!(...)]` pointing here. One file serves
GitHub and docs.rs, and `cargo doc` fails on a missing include, so the
reference cannot silently drift from the crate.

| API file | Included by |
|---|---|
| api/core.md | `src/core/mod.rs` |
| api/drag-and-drop.md | `src/core/components/mod.rs` |
| api/accessibility.md | `src/a11y.rs` |
| api/localization.md | `src/core/strings.rs` |
| api/drop-effects.md | `src/core/model.rs` |
| api/autoscroll.md | `src/autoscroll.rs` |
| api/sortable-lists.md | `src/sortable.rs` |
| api/boards.md | `src/board.rs` |
| api/trees.md | `src/tree.rs` |
| api/canvas.md | `src/canvas.rs` |
| api/multiselect.md | `src/multiselect.rs` |
| api/file-drops.md | `src/files.rs` |
| api/external-content.md | `src/external.rs` |
| api/drag-out.md | `src/dragout.rs` |
| api/multi-window.md | `src/desktop/mod.rs` |
| api/animation.md | `src/animate.rs` |
| api/testing.md | `src/test.rs` |
| api/debugging.md | `src/debug.rs` |
| api/mixing-payload-types.md | standalone (its items span three modules) |

Item-level `///` docs stay in the source; these files replace only the
module-level `//!` block.

## Conventions (for contributors)

Voice:

- Straight to the point. Every sentence teaches something or gets cut.
- No em dashes. Use ` - `, a comma, or two sentences.
- No emojis, no marketing adjectives, no "simply".
- Present tense, active voice. "The zone measures itself at mount", not
  "the zone will be measured".

Mechanics:

- API files start with an H1 title, then a one-sentence summary paragraph
  (rustdoc uses the first paragraph after the title as the module summary),
  followed by `##` sections.
- Concept files start with an H1 title.
- Code fences are `rust,ignore` (component code needs a renderer; plain
  fences would run as doctests once included in rustdoc).
- Item names in plain backticks, never intra-doc link syntax: `` [`X`] ``
  renders as literal bracketed text on GitHub, where these files are read
  most. On docs.rs the item's own docs sit on the same module page.
- Doc-to-doc navigation uses relative links. They work on GitHub and break
  on docs.rs, the same tradeoff the crate README already accepts.
- Props tables list name, type, default, and behavior. Verify every default
  against the source, never from memory.
