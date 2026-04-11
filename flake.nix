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

  outputs = { self, nixpkgs, flake-utils, rust-overlay, ... }:
    let
      overlays = [
        (import rust-overlay)
        self.overlays.default
      ];
    in
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default;

        rustPlatform = pkgs.makeRustPlatform {
          rustc = rustToolchain;
          cargo = rustToolchain;
        };

        commonArgs = {
          src = ./.;
          cargoLock = {
            lockFile = ./Cargo.lock;
          };
          nativeBuildInputs = [
            pkgs.pkg-config
            pkgs.dbus
          ];
          buildInputs = [
            pkgs.dbus
          ];
          doCheck = false;
        };

        # Testing framework
        testing = import (nixpkgs + "/nixos/lib/testing-python.nix") {
          inherit system pkgs;
        };

      in
      {
        packages = {
          tux-daemon = rustPlatform.buildRustPackage (commonArgs // {
            pname = "tux-daemon";
            version = "0.1.0";
            cargoBuildFlags = [ "-p" "tux-daemon" ];

            postInstall = ''
              install -Dm444 dist/com.tuxedocomputers.tccd.conf -t $out/share/dbus-1/system.d/
            '';
          });

          tux-tui = rustPlatform.buildRustPackage (commonArgs // {
            pname = "tux-tui";
            version = "0.1.0";
            cargoBuildFlags = [ "-p" "tux-tui" ];
          });

          tux-kmod = (kernel: pkgs.stdenv.mkDerivation {
            pname = "tux-kmod";
            version = "0.1.0";
            src = ./tux-kmod;

            nativeBuildInputs = kernel.moduleBuildDependencies;

            makeFlags = [
              "KDIR=${kernel.dev}/lib/modules/${kernel.modDirVersion}/build"
            ];

            installPhase = ''
              mkdir -p $out/lib/modules/${kernel.modDirVersion}/extra
              find . -name "*.ko" -exec cp {} $out/lib/modules/${kernel.modDirVersion}/extra/ \;
            '';
          });

          tux-kmod-latest = self.packages.${system}.tux-kmod pkgs.linuxPackages_latest.kernel;

          default = self.packages.${system}.tux-daemon;
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
              systemd.services.tux-daemon.serviceConfig.ExecStart = pkgs.lib.mkForce "${pkgs.tux-daemon}/bin/tux-daemon --mock";
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
      overlays.default = final: prev: {
        tux-daemon = self.packages.${final.system}.tux-daemon;
        tux-tui = self.packages.${final.system}.tux-tui;
        tux-kmod = self.packages.${final.system}.tux-kmod-latest;
      };

      nixosModules.default = { pkgs, ... }: {
        nixpkgs.overlays = [ self.overlays.default ];
        imports = [ ./nixos/default.nix ];
      };
    };
}
