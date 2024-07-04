#!/usr/bin/env bash 

cargo lfix --allow-dirty --allow-staged -q --all-features
cargo lclippy --fix --allow-dirty --allow-staged --all-features
cargo fmt --all
cargo sort -w
cargo-machete --fix --skip-target-dir