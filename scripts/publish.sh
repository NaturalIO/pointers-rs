#!/bin/bash

cargo clippy --all-features -- -D warnings || exit 1
cargo install cargo-rdme
cargo rdme --check || exit 1
cargo publish
