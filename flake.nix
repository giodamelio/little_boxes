{
  description = "Adds boxes around stdin. Optionally adds a title";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };
    rust-toolchains = {
      url = "github:giodamelio/rust-toolchains";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs @ {
    flake-parts,
    rust-toolchains,
    ...
  }:
    flake-parts.lib.mkFlake {inherit inputs;} {
      imports = [rust-toolchains.flakeModules.default];

      systems = ["x86_64-linux" "aarch64-linux" "aarch64-darwin"];

      perSystem = {
        config,
        lib,
        pkgs,
        ...
      }: let
        manifest = (lib.importTOML ./Cargo.toml).package;
        inherit (config.rust-toolchains) toolchain;

        # Every hook also runs in the pre-commit stage, so that `prek run -a`, which is what
        # `jj push` runs, checks everything a push would.
        bothStages = lib.mkForce ["pre-commit" "pre-push"];
      in {
        rust-toolchains.channel = "stable";

        packages.default = config.rust-toolchains.rustPlatform.buildRustPackage {
          pname = manifest.name;
          inherit (manifest) version;
          src = lib.cleanSource ./.;
          # The nixpkgs package builds from vendor/ too, so building from it here keeps CI
          # testing the crates that package will use.
          cargoVendorDir = "vendor";

          nativeBuildInputs = [pkgs.installShellFiles];

          postInstall = ''
            # build.rs writes the man page and completions into its OUT_DIR, whose path differs
            # between cargo versions and profiles, so locate the directory by the man page in it.
            outDir=$(dirname "$(find target/ -name little_boxes.1 -path '*/build/little_boxes*' | head -n1)")
            installManPage "$outDir/little_boxes.1"
            installShellCompletion --bash "$outDir/little_boxes.bash"
            installShellCompletion --zsh "$outDir/_little_boxes"
            installShellCompletion --fish "$outDir/little_boxes.fish"
            installShellCompletion --nushell "$outDir/little_boxes.nu"
          '';

          meta = {
            inherit (manifest) description;
            homepage = manifest.repository;
            license = lib.licenses.mit;
            maintainers = [lib.maintainers.giodamelio];
            mainProgram = "little_boxes";
          };
        };

        # The checks module copies its package's cargoDeps, which a vendored build leaves null, so
        # it gets one assembled from vendor/ and the lockfile cargoSetupHook checks it against.
        rust-toolchains.checks.package = config.packages.default.overrideAttrs {
          cargoDeps = pkgs.runCommand "${manifest.name}-vendor" {} ''
            cp -r ${./vendor} $out
            chmod u+w $out
            cp ${./Cargo.lock} $out/Cargo.lock
          '';
        };

        rust-toolchains.cargoDeny.settings.advisories = {
          ignore = [
            {
              id = "RUSTSEC-2026-0542";
              reason = "humantime is unmaintained with no replacement yet, and only trycmd uses it, for the tests";
            }
          ];
          # .cargo/config.toml replaces the crates.io index with vendor/, so cargo-deny cannot
          # look crates up there and would warn about every one of them
          yanked = "allow";
        };

        rust-toolchains.prek.hooks = {
          rustfmt.stages = bothStages;
          clippy.stages = bothStages;
          test.stages = bothStages;
          unused-deps.stages = bothStages;
          unused-dev-deps.stages = bothStages;
          cargo-deny.stages = bothStages;

          actionlint = {
            name = "actionlint";
            entry = lib.getExe pkgs.actionlint;
            language = "system";
            files = "^\\.github/workflows/";
            pass_filenames = false;
            stages = ["pre-commit" "pre-push"];
            priority = 0;
          };
          committed = {
            name = "committed";
            # git push passes the pushed commits as PRE_COMMIT_FROM_REF..TO_REF. `prek run`
            # passes neither, so it checks the same range as the Commits workflow: everything
            # after the 1.12.1 release, which predates the style.
            entry = toString (pkgs.writeShellScript "committed-range" ''
              from=''${PRE_COMMIT_FROM_REF:-924086627518ef0c96a38fb901b83d32d5db43d2}
              to=''${PRE_COMMIT_TO_REF:-HEAD}
              exec ${lib.getExe pkgs.committed} "$from..$to"
            '');
            language = "system";
            pass_filenames = false;
            always_run = true;
            stages = ["pre-commit" "pre-push"];
            priority = 0;
          };
        };

        rust-toolchains.devShell.packages = [
          pkgs.git-cliff
          pkgs.committed
          pkgs.cargo-edit
          pkgs.actionlint
          config.formatter

          # cargo upgrade needs the registry, which .cargo/config.toml replaces with vendor/
          (pkgs.writeShellApplication {
            name = "cargo-upgrade-vendored";
            runtimeInputs = [toolchain pkgs.cargo-edit];
            text = ''
              trap 'mv .cargo/config.toml.bak .cargo/config.toml 2>/dev/null || true' EXIT
              mv .cargo/config.toml .cargo/config.toml.bak
              cargo upgrade "$@"
              mv .cargo/config.toml.bak .cargo/config.toml
              trap - EXIT
              cargo vendor --locked vendor >/dev/null
            '';
          })
        ];

        rust-toolchains.banner.commands = [
          {
            command = "git-cliff --unreleased";
            description = "Preview the changelog from unreleased commits";
          }
          {
            command = "cargo-upgrade-vendored";
            description = "Upgrade dependencies and re-vendor them";
          }
          {
            command = "nix fmt";
            description = "Format Nix, Rust and YAML";
          }
        ];

        # Built from pkgs.formats.toml, the way rust-toolchains builds its own, rather than
        # with treefmt-nix. The template's formatter is alejandra alone; this adds the YAML
        # the workflows need.
        formatter = let
          treefmtConfig = (pkgs.formats.toml {}).generate "treefmt.toml" {
            excludes = ["vendor/**" "flake.lock" "Cargo.lock" "prek.toml" "result"];
            formatter = {
              alejandra = {
                command = lib.getExe pkgs.alejandra;
                includes = ["*.nix"];
              };
              # treefmt hands rustfmt bare files, so the edition Cargo.toml would supply has to
              # be passed explicitly.
              rustfmt = {
                command = lib.getExe' toolchain "rustfmt";
                options = ["--edition" manifest.edition];
                includes = ["*.rs"];
              };
              yamlfmt = {
                command = lib.getExe pkgs.yamlfmt;
                # Blank lines separate the sections of .goreleaser.yml and the workflows
                options = ["-formatter" "retain_line_breaks=true"];
                includes = ["*.yml" "*.yaml"];
              };
              yamllint = {
                command = lib.getExe pkgs.yamllint;
                options = ["--strict"];
                includes = ["*.yml" "*.yaml"];
              };
            };
          };
        in
          pkgs.writeShellApplication {
            name = "treefmt";
            text = ''exec ${lib.getExe pkgs.treefmt} --config-file ${treefmtConfig} --tree-root-file flake.nix "$@"'';
          };
      };
    };
}
