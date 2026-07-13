#!/usr/bin/env bash
set -euo pipefail

# WSL launches the same Mission Control charts example used on Windows. The
# shared Rust source uses use_dnd_model, DndScope, DndWorld::vdom, and
# MultiWindowProvider, so this path cannot drift back to the old manual wiring.
example_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

if [[ -r /proc/sys/kernel/osrelease ]] \
    && ! grep -qi microsoft /proc/sys/kernel/osrelease; then
    printf 'warning: this launcher is intended for WSL/WSLg; continuing on Linux\n' >&2
fi

if [[ -z "${DISPLAY:-}" && -z "${WAYLAND_DISPLAY:-}" ]]; then
    printf 'error: no WSLg display found (DISPLAY and WAYLAND_DISPLAY are unset)\n' >&2
    exit 1
fi

# X11 gives WSLg the global window/cursor coordinates needed for the complete
# Windows-like cross-window path. Set GDK_BACKEND=wayland to exercise the
# documented local-window fallback instead.
export GDK_BACKEND="${GDK_BACKEND:-x11}"
export RUST_BACKTRACE="${RUST_BACKTRACE:-1}"

case "$GDK_BACKEND" in
    x11)
        printf 'Launching Mission Control charts via WSLg/X11 (full cross-window path)\n'
        ;;
    wayland)
        printf 'Launching Mission Control charts via WSLg/Wayland (per-window fallback)\n'
        ;;
    *)
        printf 'warning: unrecognized GDK_BACKEND=%s; passing it through unchanged\n' "$GDK_BACKEND" >&2
        ;;
esac

exec cargo run \
    --locked \
    --manifest-path "$example_dir/Cargo.toml" \
    "$@"
