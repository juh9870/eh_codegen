#!/usr/bin/env bash 

cargo fix --allow-dirty --allow-staged -q --all-features
cargo clippy --fix --allow-dirty --allow-staged --all-features
cargo fmt --all
cargo sort -w
cargo-machete --fix --skip-target-dir