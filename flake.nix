{
  description = "TUXEDO laptop hardware control daemon and TUI";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  # The flake is a thin wrapper around ./default.nix and ./nix/*.nix so that
  # classic (non-flake) Nix consumers can use the same packaging. The flake's
  # job is to pin a rust toolchain via rust-overlay, expose devShells, and run
  # the VM check.
  outputs = { self, nixpkgs, flake-utils, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default;

        rustPlatform = pkgs.makeRustPlatform {
          rustc = rustToolchain;
          cargo = rustToolchain;
        };

        # Build packages via the shared nix/*.nix files, but inject the
        # rust-overlay-pinned rustPlatform so flake consumers get a predictable
        # toolchain regardless of what nixpkgs currently ships.
        tux-daemon = pkgs.callPackage ./nix/tux-daemon.nix { inherit rustPlatform; };
        tux-tui = pkgs.callPackage ./nix/tux-tui.nix { inherit rustPlatform; };
        tux-kmod = pkgs.callPackage ./nix/tux-kmod.nix {
          kernel = pkgs.linuxPackages_latest.kernel;
        };

        testing = import (nixpkgs + "/nixos/lib/testing-python.nix") {
          inherit system pkgs;
        };
      in
      {
        packages = {
          inherit tux-daemon tux-tui tux-kmod;
          default = tux-daemon;
        };

        devShells.default = pkgs.mkShell {
          buildInputs = [
            rustToolchain
            pkgs.pkg-config
            pkgs.dbus
            pkgs.just
          ];
        };

        checks = {
          vmTest = testing.makeTest {
            name = "tux-daemon-test";
            nodes.machine = { pkgs, ... }: {
              imports = [ self.nixosModules.default ];
              services.tux-daemon.enable = true;
              services.tux-daemon.kernelModules.enable = false;

              # Use mock mode in VM test to avoid hardware detection failure
              systemd.services.tux-daemon.serviceConfig.ExecStart =
                pkgs.lib.mkForce "${pkgs.tux-daemon}/bin/tux-daemon --mock";
            };

            testScript = ''
              machine.wait_for_unit("tux-daemon.service")
              # The daemon might take a second to register on D-Bus
              machine.wait_until_succeeds("busctl introspect com.tuxedocomputers.tccd /com/tuxedocomputers/tccd")

              # Check if daemon is responsive on D-Bus via tux-tui
              output = machine.succeed("tux-tui --json")
              print(output)

              # Verify it contains some expected keys in the JSON
              # Rate-limited/mocked dashboard should still have basic keys
              import json
              data = json.loads(output)
              assert "dashboard" in data
              assert "fan_curve" in data
              assert "capabilities" in data
            '';
          };
        };
      }
    ) // {
      # System-independent outputs.
      #
      # The overlay here references `self.packages.${system}.*` rather than
      # `final.callPackage ./nix/*.nix`, so that flake consumers importing
      # `inputs.tux-rs.nixosModules.default` get the rust-overlay-pinned
      # binaries. Non-flake consumers get a generic overlay from
      # `./nix/overlay.nix` via `./default.nix`.
      overlays.default = final: prev: {
        tux-daemon = self.packages.${final.system}.tux-daemon;
        tux-tui = self.packages.${final.system}.tux-tui;
        tux-kmod = self.packages.${final.system}.tux-kmod;
      };

      nixosModules.default = { pkgs, ... }: {
        nixpkgs.overlays = [ self.overlays.default ];
        imports = [ ./nixos/default.nix ];
      };
    };
}
