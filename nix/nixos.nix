{ config, lib, pkgs, ... }:

with lib;

let
  cfg = config.services.tux-daemon;
in
{
  options.services.tux-daemon = {
    enable = mkEnableOption "TUXEDO hardware control daemon";

    package = mkOption {
      type = types.package;
      default = pkgs.tux-daemon;
      defaultText = literalExpression "pkgs.tux-daemon";
      description = "The tux-daemon package to use.";
    };

    tui.package = mkOption {
      type = types.package;
      default = pkgs.tux-tui;
      defaultText = literalExpression "pkgs.tux-tui";
      description = "The tux-tui package to use.";
    };

    kernelModules = {
      enable = mkOption {
        type = types.bool;
        default = true;
        description = "Whether to include and auto-load the tuxedo-drivers kernel modules.";
      };

      package = mkOption {
        type = types.package;
        default = pkgs.linuxPackages.tuxedo-drivers;
        defaultText = literalExpression "pkgs.linuxPackages.tuxedo-drivers";
        description = "The tuxedo-drivers kernel module package to use.";
      };
    };
  };

  config = mkIf cfg.enable {
    environment.systemPackages = [ cfg.package cfg.tui.package ];

    services.dbus.packages = [ cfg.package ];

    boot.extraModulePackages = mkIf cfg.kernelModules.enable [ cfg.kernelModules.package ];
    boot.kernelModules = mkIf cfg.kernelModules.enable [
      "tuxedo_io"
      "tuxedo_nb05_fan_control"
      "tuxedo_nb05_sensors"
      "tuxedo_nb04_sensors"
      "tuxedo_nb04_power_profiles"
      "tuxedo_fan_control"
    ];

    systemd.services.tux-daemon = {
      description = "TUXEDO Hardware Daemon";
      after = [ "dbus.service" ];
      requires = [ "dbus.service" ];
      wantedBy = [ "multi-user.target" ];

      serviceConfig = {
        Type = "simple";
        ExecStart = "${cfg.package}/bin/tux-daemon";
        Restart = "on-failure";
        RestartSec = "2s";
      };
    };
  };
}
