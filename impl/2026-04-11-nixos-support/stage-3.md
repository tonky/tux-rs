# Stage 3: NixOS Module and D-Bus

## Objective
Create a NixOS module that configures `tux-daemon` system-wide.

## Plan
1. Create `nix/nixos.nix`.
2. Add options to enable the daemon and load kernel modules.
3. Configure the `systemd.services.tux-daemon`.
4. Install D-Bus policy `dist/com.tuxedocomputers.tccd.conf` to `services.dbus.packages`.
5. Expose the module as `nixosModule` in `flake.nix`.

## References
- `dist/tux-daemon.service`
- `dist/com.tuxedocomputers.tccd.conf`
