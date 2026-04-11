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

  # The flake is a thin wrapper around ./default.nix so classic (non-flake)
  # Nix consumers share the exact same packaging code. The flake's job is to
  # pin a rust toolchain via rust-overlay, expose devShells, and run the VM
  # check.
  outputs = { self, nixpkgs, flake-utils, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };

        # Delegate all package building to ./default.nix so there's a single
        # source of truth. rust-overlay is already applied to `pkgs`, so
        # default.nix's rust-bin detection picks it up without needing to
        # appendOverlays again.
        tux-rs = import ./. { inherit pkgs; };

        rustToolchain = pkgs.rust-bin.stable.latest.default;

        testing = import (nixpkgs + "/nixos/lib/testing-python.nix") {
          inherit system pkgs;
        };
      in
      {
        packages = {
          inherit (tux-rs) tux-daemon tux-tui tux-kmod;
          default = tux-rs.tux-daemon;
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
      # The flake's overlay references `self.packages.${system}.*` (not
      # `final.callPackage ./nix/*.nix`) so flake consumers importing
      # `inputs.tux-rs.nixosModules.default` get rust-overlay-pinned binaries
      # without needing to pass `rust-overlay` through themselves. Non-flake
      # consumers get the generic `nix/overlay.nix` via `./default.nix`.
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
