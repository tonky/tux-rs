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
        description = "Whether to include and auto-load the TUXEDO kernel modules.";
      };

      package = mkOption {
        type = types.package;
        default = pkgs.tux-kmod;
        defaultText = literalExpression "pkgs.tux-kmod";
        description = "The tux-kmod package to use.";
      };
    };
  };

  config = mkIf cfg.enable {
    environment.systemPackages = [ cfg.package cfg.tui.package ];

    services.dbus.packages = [ cfg.package ];

    boot.extraModulePackages = mkIf cfg.kernelModules.enable [ cfg.kernelModules.package ];
    boot.kernelModules = mkIf cfg.kernelModules.enable [
      "tuxedo_ec"
      "tuxedo_uniwill"
      "tuxedo_clevo"
      "tuxedo_tuxi"
      "tuxedo_nb04"
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
