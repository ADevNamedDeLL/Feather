#!/usr/bin/env bash
set -euo pipefail
cargo build --quiet --release
printf '\nBuilt: %s\n' "$(pwd)/target/release/feather"
