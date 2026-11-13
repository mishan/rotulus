# AGENTS.md — Rotulus working notes

Orientation for anyone, human or AI, working in this repository. What the
widget is and promises is in [README.md](README.md); how it works is in
[docs/design.md](docs/design.md). Git history is the record of how it got
here.

## Layout

- `crates/rotulus-layout`: the layout engine. No GTK.
- `crates/rotulus-mirc`: mIRC formatting codes into styled runs.
- `crates/rotulus`: the GTK4 widget, and with the `capi` feature the C API
  in `include/rotulus.h` that `meson.build` builds into `librotulus-1`
  with GObject introspection.

The three crates release together at one version (`Cargo.toml`'s
workspace version, its inter-crate pins and `meson.build`'s project
version). GtkHx is the first user; a change to the API lands here, is
released, and GtkHx moves to the release.

## Windows and displays: always sealed

Anything that opens a window or needs a display (the widget's tests, the
render goldens, the C smoke test, an example) runs in a sealed session,
never on the desktop you are working at. Use
[shotbox](https://github.com/mishan/shotbox): `shotbox run` gives the
command its own X display, session bus and scratch home.

```sh
shotbox run --env CARGO_HOME=$HOME/.cargo -- cargo test --workspace
shotbox run -- meson test -C _build --print-errorlogs
```

`CARGO_HOME` is passed because the scratch home has no `.cargo`; without
it cargo fetches every crate again into the scratch home. To look at or
drive the widget (a screenshot, keys, the accessible tree), use shotbox's
other commands (`shoot`, `start`/`spawn`, `key`, `a11y`); see
`shotbox --help`.

CI runs the same tests under `dbus-run-session -- xvfb-run -a`. The render
goldens (`crates/rotulus/tests/render.rs`) pin their own fonts, DPI, theme
and settings, so they draw the same either way. To accept a deliberate
change to them:

```sh
ROTULUS_UPDATE_GOLDEN=1 shotbox run --env CARGO_HOME=$HOME/.cargo -- \
    cargo test -p rotulus --test render
```

## Checks

What CI runs (`.github/workflows/ci.yml`), on the `rust-toolchain.toml`
floor and on stable:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --workspace --all-targets --features rotulus/v4_14 -- -D warnings
cargo clippy --workspace --all-targets --features rotulus/v4_16 -- -D warnings
cargo clippy --workspace --all-targets --no-default-features -- -D warnings
cargo clippy --workspace --all-targets --features rotulus/capi -- -D warnings
# each of "", --features rotulus/v4_14, --features rotulus/v4_16,
# --no-default-features, sealed as above:
cargo test --workspace
cargo publish --workspace --dry-run
tools/check-translations.sh
meson setup _build -Dwerror=true -Ddocumentation=true && meson compile -C _build
meson test -C _build --print-errorlogs   # sealed
```

`RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` is clean;
keep it so, since docs.rs publishes it.

## Conventions

- A branch per change, short kebab-case names, one commit squashed before
  the pull request; follow-up commits during review.
- Commits authored `Misha Nasledov <misha@nasledov.com>`. No AI
  attribution anywhere: no `Co-Authored-By` trailers, no "generated with",
  in commits, pull requests, code or docs.
- US spelling, in code and prose alike.
- Comments say why, not what the code does or how it changed.
- A test is watched failing before it is trusted.
- Releases: a pull request titled with the bare version bumps the three
  version places above, with release notes for the tag in its body. Once
  it merges and CI is green on main: tag `vX.Y.Z`, `cargo publish
  --workspace`, and a GitHub release from those notes. The C library's ABI
  version (`abi_version` in `meson.build`) changes only when the header
  removes or changes something.
