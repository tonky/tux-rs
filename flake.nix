{
  description = "TUXEDO laptop hardware control daemon and TUI";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  # The flake is a thin wrapper around ./default.nix so classic (non-flake)
  # Nix consumers share the exact same packaging code.
  outputs = { self, nixpkgs, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };

        # Delegate all package building to ./default.nix so there's a single
        # source of truth.
        tux-rs = import ./. { inherit pkgs; };

        testing = import (nixpkgs + "/nixos/lib/testing-python.nix") {
          inherit system pkgs;
        };
      in
      {
        packages = {
          inherit (tux-rs) tux-daemon tux-tui;
          default = tux-rs.tux-daemon;
        };

        devShells.default = pkgs.mkShell {
          buildInputs = [
            pkgs.rustc
            pkgs.cargo
            pkgs.pkg-config
            pkgs.dbus
            pkgs.just
          ];
        };

        checks = {
          vmTest = testing.makeTest {
            name = "tux-daemon-test";
            nodes.machine = { pkgs, ... }: {
              imports = [ self.nixosModule ];
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
      overlay = final: prev: {
        tux-daemon = self.packages.${final.system}.tux-daemon;
        tux-tui = self.packages.${final.system}.tux-tui;
      };

      nixosModule = { pkgs, ... }: {
        nixpkgs.overlays = [ self.overlay ];
        imports = [ ./nix/nixos.nix ];
      };

      # Compatibility aliases for older configs.
      overlays.default = self.overlay;
      nixosModules.default = self.nixosModule;
    };
}
