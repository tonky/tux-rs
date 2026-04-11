{ lib
, rustPlatform
, pkg-config
, dbus
}:

rustPlatform.buildRustPackage {
  pname = "tux-daemon";
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

  cargoBuildFlags = [ "-p" "tux-daemon" ];

  # E2E tests require D-Bus in the Nix sandbox; tracked in follow_up.toml.
  doCheck = false;

  postInstall = ''
    install -Dm444 dist/com.tuxedocomputers.tccd.conf \
      -t $out/share/dbus-1/system.d/
  '';

  meta = with lib; {
    description = "TUXEDO laptop hardware control daemon";
    homepage = "https://github.com/tonky/tux-rs";
    license = licenses.gpl3Plus;
    platforms = platforms.linux;
    mainProgram = "tux-daemon";
  };
}
