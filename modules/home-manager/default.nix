self:
{
  config,
  lib,
  pkgs,
  ...
}:

let
  inherit (lib)
    types
    ;
  inherit (lib.options) mkEnableOption mkOption;

  cfg = config.programs.yurf;

  tomlFormat = pkgs.formats.toml { };

  tasksConfig =
    with types;
    submodule {
      freeformType = tomlFormat.type;

      options = {
        name = mkOption {
          type = str;
          example = "light: Increase by 10";
          description = "Name of the task";
        };

        command = mkOption {
          type = str;
          example = "light -A 10";
          description = "Command to execute when selected.";
        };

        need_confirm = mkOption {
          type = bool;
          default = false;
          example = true;
          description = "need confirm to run";
        };
      };
    };
in
{
  options.programs.yurf = with lib.types; {
    enable = mkEnableOption "yurf";

    package = mkOption {
      type = package;
      default = self.packages.${pkgs.hostPlatform.system}.default;
    };

    task = mkOption {
      type = listOf tasksConfig;
      default = [ ];
      description = ''
        List of tasks to display when run `yurf task`.
      '';
      example = literalExpression ''
        [
          {
            name = "light: Increase by 10";
            command = "light -A 10";
          }
        ]
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    home.packages = [ cfg.package ];
    xdg.configFile."yurf/config.toml" = {
      source = pkgs.writers.writeTOML "yurf-config.toml" {
        task = cfg.tasks;
      };
    };
  };
}
