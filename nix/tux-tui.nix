{ lib
, rustPlatform
, pkg-config
, dbus
}:

rustPlatform.buildRustPackage {
  pname = "tux-tui";
  version = "0.1.0";

  src = ../.;
  cargoLock = {
    lockFile = ../Cargo.lock;
  };

  nativeBuildInputs = [
    pkg-config
    dbus
  ];

  buildInputs = [
    dbus
  ];

  cargoBuildFlags = [ "-p" "tux-tui" ];

  # E2E tests require D-Bus in the Nix sandbox; tracked in follow_up.toml.
  doCheck = false;

  meta = with lib; {
    description = "Terminal UI for the TUXEDO hardware control daemon";
    homepage = "https://github.com/tonky/tux-rs";
    license = licenses.gpl3Plus;
    platforms = platforms.linux;
    mainProgram = "tux-tui";
  };
}
