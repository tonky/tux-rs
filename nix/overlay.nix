final: prev: {
  tux-daemon = final.callPackage ./tux-daemon.nix { };
  tux-tui = final.callPackage ./tux-tui.nix { };
}
