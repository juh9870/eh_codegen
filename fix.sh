#!/usr/bin/env bash 

cargo fmt --all
cargo fix --allow-dirty --allow-staged -q --all-features
cargo clippy --fix --allow-dirty --allow-staged --all-features
cargo sort -w
cargo-machete --fix --skip-target-dir