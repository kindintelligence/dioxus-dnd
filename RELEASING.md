# Releasing dioxus-dnd

The release process has two human decisions: approving the release PR and
approving the protected crates.io environment. Everything after the version
tag is validated and automated.

## Compatibility policy

The `3.1.x` line supports Dioxus `0.7` and is locked and tested against
`0.7.9`. This preserves one coherent Dioxus type graph for existing users.

The renderer-neutral library is checked on Rust 1.85. The optional desktop
dependency graph is checked on current stable Rust because its Dioxus Desktop
transitives currently require Rust 1.88 for a security-patched `time` release.

CI also rewrites a clean checkout to published Dioxus `0.8.0-alpha.0` and runs
the full Rust suite. That is a source-compatibility signal, not a published
dependency promise. Cargo cannot cleanly express a union of stable `0.7.9` and
prerelease `0.8.0-alpha.0`. When Dioxus 0.8 stabilizes, review its final public
types and choose an intentional compatibility range or a new dioxus-dnd major
version.

## One-time repository setup

The GitHub environment is named `crates-io`. It permits only `v*` tags and
requires a maintainer review before the publish job can request credentials.

Configure the crate's Trusted Publisher on crates.io with these values:

| Field | Value |
|---|---|
| Provider | GitHub Actions |
| Repository owner | `kindintelligence` |
| Repository | `dioxus-dnd` |
| Workflow | `release.yml` |
| Environment | `crates-io` |

No long-lived crates.io token belongs in GitHub. The release job requests a
short-lived OIDC token only after the protected environment is approved.

Both `main` and `development` should reject force pushes and deletions. Changes
land through pull requests with the required CI checks. GitHub Actions should
require every external action to be pinned to a full commit SHA.

## Prepare the release PR

1. Start from an up-to-date `development` branch.
2. Set the version in `Cargo.toml`.
3. Refresh `Cargo.lock` and the lockfiles under both standalone desktop
   examples.
4. Move the accumulated changelog entries from `Unreleased` under a dated
   release heading. Leave a new empty `Unreleased` heading at the top.
5. Update the README compatibility table and any version-specific docs.
6. Run the local release checks that are practical for the current platform.
7. Open a pull request from `development` to `main` and wait for every required
   check.

Useful local commands:

```console
cargo fmt --all -- --check
cargo test --locked --all-features
cargo clippy --locked --all-features --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --locked --all-features --no-deps
cargo publish --dry-run --locked
npm ci
npm run test:web
```

The standalone desktop packages have separate workspaces and lockfiles:

```console
cargo test --locked --manifest-path examples/desktop-multiwindow/Cargo.toml
cargo test --locked --manifest-path examples/desktop-showcase/Cargo.toml
```

## Tag the approved main commit

After the release PR is merged and the `main` push CI is green, tag the current
remote `main` head. The workflow rejects a tag that points anywhere else.

```console
git fetch origin main
git tag -a v3.1.0 origin/main -m "dioxus-dnd 3.1.0"
git push origin v3.1.0
```

The tag starts `.github/workflows/release.yml`. It performs these steps:

1. Match the tag to the Cargo version and dated changelog heading.
2. Confirm both standalone lockfiles are current.
3. Confirm the tag points at the current `main` head.
4. Run the complete reusable CI workflow again against the tag.
5. Build the exact `.crate` payload.
6. Pause for approval on the `crates-io` environment.
7. Publish through crates.io Trusted Publishing.
8. Create the GitHub Release from the curated changelog section.

The gallery deploys only after successful CI on `main`, using the exact commit
SHA that passed. The release tag therefore refers to the same source as the
deployed gallery.

## Retries and failures

Do not move a release tag after crates.io accepts the version. Published crate
versions cannot be overwritten.

The publish job is safe to rerun. If the version already exists, it packages
the tag again and compares the local SHA-256 checksum with crates.io. It skips
the upload only when those checksums match. The GitHub Release step can also
create a missing release or refresh an existing one.

If validation fails before publication, fix the release PR, delete the failed
remote tag, and create the tag again at the corrected `main` head. If a harmful
crate has already been published, yank that version on crates.io and prepare a
new patch version. Keep the tag and GitHub Release as the immutable record of
what was published.

## Dependency advisories

`cargo-deny` checks the root and both desktop graphs. Exceptions in `deny.toml`
must state why the dependency cannot be upgraded independently. The current
GTK3, glib, and related exceptions come through Dioxus Desktop 0.7. Revisit
them during the Dioxus 0.8 migration and whenever Dependabot reports a newly
patched path.
