# Stage 5: Remove tux-kmod & update packaging

## Context
Remove the legacy `tux-kmod` entirely now that the daemon is safely mapped to `tuxedo-drivers`. We must also adjust Nix packaging, `Justfile`, and DKMS configs to prune out the old C driver pipeline.

## Files to modify
- **[DELETE] `tux-kmod/`**: Entirely remove the legacy `tux-kmod` module.
- **`Justfile`**: Remove the recipes like `kmod-build`, `kmod-install`, `kmod-swap`.
- **`flake.nix`**: Remove `tux-kmod` dependencies from default Nix shells and derivations. Update NixOS module service to depend explicitly on `tuxedo-drivers` package rather than local modules.
- **`README.md`**: Wipe all references of building with local `tux-kmod`.
