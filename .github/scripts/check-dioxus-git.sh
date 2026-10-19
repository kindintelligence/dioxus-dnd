#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "$0")/../.." && pwd)
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT

# This must be a consumer, outside the workspace: the library's dev-dependency
# features must not hide missing features or duplicate Dioxus public types.
python3 - "$repo_root" "$scratch" <<'PY'
import pathlib
import sys
import tomllib

root, scratch = map(pathlib.Path, sys.argv[1:])
revision = "f9519967a6fb3e85bf3c1a2e2bf816a85033f752"
packages = tomllib.loads((root / "Cargo.lock").read_text())["package"]
# These packages come from the Dioxus workspace. Patch the whole family so
# core/signals/HTML types cannot split between registry and git copies.
names = sorted({p["name"] for p in packages if p["version"] == "0.8.0-alpha.1"})
manifest = f'''[package]
name = "dioxus-dnd-git-compat"
version = "0.0.0"
edition = "2021"
publish = false

[dependencies]
dioxus-dnd = {{ path = "{root}", features = ["web", "desktop", "serde"] }}
dioxus = {{ version = "=0.8.0-alpha.1", default-features = false, features = ["minimal", "document", "mounted", "web", "router", "ssr"] }}

[patch.crates-io]
'''
for name in names:
    manifest += f'{name} = {{ git = "https://github.com/DioxusLabs/dioxus", rev = "{revision}" }}\n'
(scratch / "Cargo.toml").write_text(manifest)
(scratch / "src").mkdir()
(scratch / "src/lib.rs").write_text('''use dioxus::prelude::*;
use dioxus_dnd::prelude::*;

#[component]
pub fn Consumer() -> Element {
    let model = use_dnd_model(|| Signal::new(vec![1_u32]));
    rsx! {
        DndProvider::<u32> {
            Draggable::<u32> { payload: model.read()[0], "Drag" }
            DropZone::<u32> { on_drop: move |_: DropOutcome<u32>| {}, "Drop" }
        }
    }
}

#[component]
pub fn DesktopConsumer() -> Element {
    rsx! { MultiWindowProvider::<u32> {} }
}
''')
PY

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$repo_root/target/git-compat}"
cargo check --manifest-path "$scratch/Cargo.toml"

# A successful build alone could still hide two copies if the consumer never
# exchanges one of their types. Check the resolved family as well.
cargo metadata --manifest-path "$scratch/Cargo.toml" --format-version 1 \
    > "$scratch/metadata.json"
python3 - "$scratch/metadata.json" <<'PY'
import json
import sys

packages = json.load(open(sys.argv[1]))["packages"]
family = [p for p in packages if p["version"] == "0.8.0-alpha.1"]
assert any(p["name"] == "dioxus" for p in family)
for package in family:
    source = package.get("source") or ""
    assert source.startswith("git+https://github.com/DioxusLabs/dioxus?rev=f9519967"), package["name"]
    assert source.endswith("#f9519967a6fb3e85bf3c1a2e2bf816a85033f752"), package["name"]
assert not any(p["name"] == "dioxus" and p["version"] != "0.8.0-alpha.1" for p in packages)
print("Verified one coherent Dioxus family at f9519967.")
PY
