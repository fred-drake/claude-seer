{
  description = "Rust development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    nixpkgs,
    rust-overlay,
    flake-utils,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      overlays = [(import rust-overlay)];
      pkgs = import nixpkgs {inherit system overlays;};
      rustToolchain = pkgs.rust-bin.stable.latest.default.override {
        extensions = ["rust-src" "rust-analyzer" "clippy" "rustfmt"];
      };
    in {
      devShells.default = pkgs.mkShell {
        buildInputs = with pkgs;
          [
            # Rust toolchain
            rustToolchain
            cargo-edit
            cargo-watch
            cargo-expand
            cargo-nextest
            cargo-audit
            cargo-outdated
            cargo-flamegraph
            cargo-tarpaulin

            # GitHub CLI
            gh

            # Task runner
            just

            # Debugging
            lldb

            # Profiling
            samply
          ]
          ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
            pkgs.gdb
            pkgs.valgrind
            pkgs.linuxPackages_latest.perf
            pkgs.heaptrack
            pkgs.hotspot
          ];

        RUST_BACKTRACE = 1;
        RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
      };
    });
}
