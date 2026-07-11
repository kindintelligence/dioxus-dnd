# Virtualized lists

A windowed (virtualized) list renders only the visible slice, so its rows -
and the drop zones on them - constantly appear and disappear. The crate
needs nothing special for this: rows are ordinary `DropZone`s, and the zone
registry is built for churn.

API reference: the registry section of [api/core.md](../api/core.md).
Live demo: the
[Archive](https://kindintelligence.github.io/dioxus-dnd/archive) page runs
the full pattern at 10,000 rows, keyboard navigation included.

## The mental model

Zones churn; the registry does not care:

- A row scrolling into view mounts a `DropZone`, which registers itself
  and **measures itself the moment it mounts**. That mount-time measurement
  is what makes the pattern work: a row appearing *mid-drag* missed the
  pickup measurement and the last scroll ping, yet it is hit-testable the
  instant it exists.
- A row scrolling out unmounts, and `use_drop` unregisters its zone.
- **Stable, index-derived ids** tie it together: give each row
  `ZoneId(BASE + index)` so a recycled row re-registers as itself, and a
  remount replaces the old registration in place. Handlers capture the row
  index, so the model update is a plain indexed write - no bookkeeping
  about which DOM node happened to host the row.

Auto-generated ids start at 2^32, so any `BASE + index` scheme that stays
in `u32` range can never collide with them. Pick a `BASE` clear of your
other explicit ids (the archive page uses 20,000).

## A worked example

The archive page's shape, trimmed. Fixed-height rows make the window
`scroll_top / row_height` plus a buffer, drawn inside a full-height canvas
with a `translateY` offset - the standard virtualization shape:

```rust,ignore
AutoScroll {
    style: "height: 420px; overflow-y: auto;",
    on_scroll: move |offset: Point| scroll_top.set(offset.y), // edge auto-scroll
    div { style: "position: relative; height: {ROWS as f64 * ROW_H}px;",
        div { style: "position: absolute; top: 0; width: 100%;
                      transform: translateY({first as f64 * ROW_H}px);",
            for ix in first..last {
                DropZone::<Tag> {
                    key: "{ix}",
                    id: ZoneId(BASE + ix as u64),   // stable per row
                    label: format!("Record {}", ix + 1),
                    on_drop: move |o: DropOutcome<Tag>| tag_row(ix, o.payload),
                    "aria-setsize": "{ROWS}",
                    "aria-posinset": "{ix + 1}",
                    div {
                        // rows double as scroll sentinels (see below)
                        onvisible: move |evt| resync_window(ix, evt),
                        Row { ix }
                    }
                }
            }
        }
    }
}
```

Ten thousand rows, a few dozen mounted zones at any moment - and every one
of them a live drop target. The library side is unremarkable on purpose:
swap in any virtual list, keep the zones.

## The windowing signal

dioxus-web 0.7 delivers no element-level scroll events - an `onscroll`
handler on the container never fires. Dioxus's documented virtual-list tool
is `onvisible`, and the rendered rows double as their own scroll sentinels:
whenever a row crosses the container's clip, its IntersectionObserver entry
fires, and the entry's rect plus the row's known canvas position
(`index * row_height`) recover the scroll offset. That covers wheel,
scrollbar and programmatic scrolls alike, idle or mid-drag. Give the window
a buffer of rows on each side so a crossing fires before blank space can
show.

## Scrolling during the drag

Wrap the container in `AutoScroll` and a drag hovering near its edges
scrolls the archive under the pointer. Two things make that safe here:

- `on_scroll` reports the container's offset after every scroll the
  component observes - its own edge-scrolling above all - so the window
  re-slices while the drag is still in flight, and new rows register as the
  old ones unregister.
- Every observed scroll also pings the rect-refresh channel, so already
  registered zones re-measure. Hover highlighting and the eventual drop
  land on the row the user actually sees, not where rows sat at pickup.

If your scroll surface is not an `AutoScroll`, grab the channel yourself
with `use_rect_refresh()` and call `refresh_all()` from your own scroll
signal. See [Auto-scroll](autoscroll.md).

## Announcing position in the full list

`DropZone` forwards arbitrary attributes, so each row can carry
`aria-setsize` (the full length) and `aria-posinset` (this row's 1-based
position). Screen readers then announce "record 5,732 of 10,000" even
though only a few dozen rows exist in the DOM. Pair with `role: "list"` on
the canvas and `role: "listitem"` on the rows, and give each zone a `label`
so keyboard hovers announce something meaningful. See
[Accessibility](accessibility.md).

## Gotchas

- **Keyboard drags reach mounted rows only.** Arrow keys walk the
  registered zones in spatial order, and only the window's rows are
  registered. Scroll first, then pick up.
- **Auto ids break recycling.** An auto-generated id is minted per mount,
  so a recycled row would register as a stranger and your handler could not
  name it. Index-derived ids are the contract.
- **Key the rows by index.** `key: "{ix}"` keeps the remount aligned with
  the registration it replaces.
- **The sentinel signal is edge-triggered.** `onvisible` fires on clip
  crossings, not continuously; the buffer rows are what turn crossings into
  a timely window. Do not shrink the buffer to zero.

## Related

- [Auto-scroll](autoscroll.md): edge-scrolling, `on_scroll`, and the
  rect-refresh channel.
- [Drag and drop](drag-and-drop.md): what a `DropZone` registers and
  measures.
- [Architecture](architecture.md): the registry that absorbs the churn.
- [Accessibility](accessibility.md): announcements and labels.
