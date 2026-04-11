{ lib
, stdenv
, kernel
}:

stdenv.mkDerivation {
  pname = "tux-kmod";
  version = "0.1.0";

  src = ../tux-kmod;

  nativeBuildInputs = kernel.moduleBuildDependencies;

  makeFlags = [
    "KDIR=${kernel.dev}/lib/modules/${kernel.modDirVersion}/build"
  ];

  installPhase = ''
    runHook preInstall
    mkdir -p $out/lib/modules/${kernel.modDirVersion}/extra
    find . -name "*.ko" -exec cp {} $out/lib/modules/${kernel.modDirVersion}/extra/ \;
    runHook postInstall
  '';

  meta = with lib; {
    description = "TUXEDO kernel modules (Rust port's C shims)";
    homepage = "https://github.com/tonky/tux-rs";
    license = licenses.gpl2Only;
    platforms = platforms.linux;
  };
}
