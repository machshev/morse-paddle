# SPDX-FileCopyrightText: 2025 David James McCorrie <djmccorrie@gmail.com>
#
# SPDX-License-Identifier: Apache-2.0
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay/stable";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs:
    with inputs;
      flake-utils.lib.eachDefaultSystem (
        system: let
          overlays = [(import rust-overlay)];
          pkgs = import nixpkgs {
            inherit system overlays;
          };
        in {
          devShells = {
            default = with pkgs;
              mkShell {
                buildInputs = [
                  (rust-bin.stable.latest.default.override {
                    extensions = [
                      "llvm-tools"
                      "rust-src"
                    ];
                    targets = [
                      "thumbv6m-none-eabi"
                      "thumbv7m-none-eabi"
                    ];
                  })
                  cargo-nextest
                  cargo-binutils
                  cargo-udeps
                  cargo-vet
                  cargo-about
                  cargo-release
                  cargo-machete

                  rust-analyzer
                  rustfmt

                  reuse
                  adrs
                  typos

                  openocd
                  gcc-arm-embedded

                  flip-link
                  probe-rs-tools
                  # If the dependencies need system libs, you usually need pkg-config + the lib
                ];
              };
          };

          formatter = nixpkgs.legacyPackages.${system}.alejandra;
        }
      );
}
