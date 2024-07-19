#!/usr/bin/env bash

set -e

export RUST_LOG=info
export RUST_BACKTRACE=1

cargo run -p eh_roguelite -- eh_roguelite/output

DIR=/media/juh9870/drive1t/Games/EH/Mods/roguelite

rm -r $DIR || true
mkdir -p $DIR
cp -r eh_roguelite/output/* $DIR