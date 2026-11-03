# Feather 0.5.6

## Fixes
- Fix Rust E0521 in the Save All close-confirmation callback by cloning the parent window before connecting the GTK signal.
- Keep the Save As required error dialog transient to the editor window.

## Validation
Run `cargo test` and `./tests/mvp-smoke-test.sh` on the target Linux environment before installing.
