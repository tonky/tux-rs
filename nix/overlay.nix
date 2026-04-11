final: prev: {
  tux-daemon = final.callPackage ./tux-daemon.nix { };
  tux-tui = final.callPackage ./tux-tui.nix { };
  tux-kmod = final.callPackage ./tux-kmod.nix {
    kernel = final.linuxPackages_latest.kernel;
  };
}
