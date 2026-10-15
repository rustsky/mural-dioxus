# tao, vendored

This is [tao](https://github.com/tauri-apps/tao) 0.37.1, under its Apache-2.0 licence (`LICENSE`). The
source is unmodified; only `Cargo.toml` is changed:

- the version is `0.34.99` instead of `0.37.1`, so that it can stand in for the tao 0.34 that
  Dioxus 0.7 asks for (`[patch.crates-io]` in Mural's `Cargo.toml`);
- a `[lints.rust]` table allows warnings, which Cargo would otherwise show for a local crate.

**Why:** apps built with the iOS 27 SDK must adopt the UIScene life cycle or UIKit refuses to
launch them. tao added scene support after 0.34; with the scene manifest in `Dioxus.toml`, this
version opens Mural's window in a scene.

Remove this folder and the patch once Dioxus depends on a tao with scene support.
