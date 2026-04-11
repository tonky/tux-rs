# Stage 2: Packaging Kernel Modules

## Objective
Define a derivation for `tux-kmod` building the out-of-tree kernel modules.

## Plan
1. Add `tux-kmod` package to `flake.nix` using `linux.moduleBuild`.
2. Configure it to build all subdirectories (as per `tux-kmod/Makefile`).
3. Build against a default kernel version (e.g. `linuxPackages_latest`).
4. Test the build using `nix build .#tux-kmod`.

## References
- `tux-kmod/Makefile`
