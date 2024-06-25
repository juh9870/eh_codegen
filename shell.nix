let
  moz_overlay = import (builtins.fetchTarball
    "https://github.com/mozilla/nixpkgs-mozilla/archive/9b11a87c0cc54e308fa83aac5b4ee1816d5418a2.tar.gz");
  nixpkgs = import <nixpkgs> { overlays = [ moz_overlay ]; };
in with nixpkgs;
let dlopen-libs = with xorg; [ ];
in mkShell.override {
  stdenv = pkgs.stdenvAdapters.useMoldLinker pkgs.stdenv;
} {
  nativeBuildInputs = with pkgs; [
    (rustChannels.stable.rust.override {
      extensions = [ "rust-src" "rust-analysis" ];
      targets = [ "x86_64-unknown-linux-gnu" ];
    })
    pkg-config
    pkgs.cargo-bloat
    pkgs.cargo-unused-features
    pkgs.cargo-watch
    pkgs.cargo-sort
    pkgs.cargo-machete
    pkgs.cargo-depgraph
    pkgs.cargo-limit
    pkgs.cargo-flamegraph
    pkgs.cargo-insta
    pkgs.cargo-audit
    pkgs.cargo-expand
    pkgs.simple-http-server
    pkgs.pre-commit
  ];
  shellHook = ''
    export RUST_BACKTRACE=1
    export LD_LIBRARY_PATH="$LD_LIBRARY_PATH:${lib.makeLibraryPath dlopen-libs}"
    export RUST_LOG=trace
    pre-commit install
  '';
}
