{
  description = "Adds boxes around stdin. Optionally adds a title";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    git-hooks = {
      url = "github:cachix/git-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
  outputs = inputs @ {
    flake-parts,
    git-hooks,
    ...
  }:
    flake-parts.lib.mkFlake {inherit inputs;} {
      systems = ["x86_64-linux" "aarch64-linux" "aarch64-darwin" "x86_64-darwin"];
      imports = [
        inputs.treefmt-nix.flakeModule
      ];
      flake = {};
      perSystem = {
        pkgs,
        config,
        system,
        ...
      }: {
        packages.default = let
          manifest = (pkgs.lib.importTOML ./Cargo.toml).package;
        in
          pkgs.rustPlatform.buildRustPackage {
            pname = manifest.name;
            version = manifest.version;
            src = pkgs.lib.cleanSource ./.;
            cargoVendorDir = "vendor";

            nativeBuildInputs = [pkgs.installShellFiles];

            postInstall = ''
              # Find the OUT_DIR where build.rs wrote manpages and completions
              outDir=$(find target/ -type d -name out -path '*/build/little_boxes-*' | head -n1)
              installManPage "$outDir/little_boxes.1"
              installShellCompletion --bash "$outDir/little_boxes.bash"
              installShellCompletion --zsh "$outDir/_little_boxes"
              installShellCompletion --fish "$outDir/little_boxes.fish"
              installShellCompletion --nushell "$outDir/little_boxes.nu"
            '';

            meta = with pkgs.lib; {
              description = manifest.description;
              homepage = manifest.homepage;
              license = licenses.mit;
              maintainers = [maintainers.giodamelio];
            };
          };

        devShells.default = let
          hooks = git-hooks.lib.${system}.run {
            src = ./.;
            package = pkgs.prek;
            # prek runs hooks with the same priority in parallel, lowest first. rustfmt rewrites
            # files, so it finishes before anything compiles them.
            hooks = {
              cargo-check = {
                enable = true;
                priority = 1;
              };
              rustfmt = {
                enable = true;
                priority = 0;
              };
              clippy = {
                enable = true;
                entry = "cargo clippy -- -D warnings";
                pass_filenames = false;
                priority = 1;
              };
              cargo-test = {
                enable = true;
                name = "cargo-test";
                entry = "cargo test";
                pass_filenames = false;
                priority = 1;
              };
              actionlint = {
                enable = true;
                priority = 0;
                name = "actionlint";
                entry = "${pkgs.actionlint}/bin/actionlint";
                files = "^\\.github/workflows/";
                types = ["yaml"];
                pass_filenames = false;
              };
              committed = {
                enable = true;
                name = "committed";
                # git push passes the pushed commits as PRE_COMMIT_FROM_REF..TO_REF. `prek run`
                # passes neither, so it checks the same range as the Commits workflow: everything
                # after the 1.12.1 release, which predates the style.
                entry = toString (pkgs.writeShellScript "committed-range" ''
                  from=''${PRE_COMMIT_FROM_REF:-924086627518ef0c96a38fb901b83d32d5db43d2}
                  to=''${PRE_COMMIT_TO_REF:-HEAD}
                  exec ${pkgs.committed}/bin/committed "$from..$to"
                '');
                # pre-commit too, so that `prek run -a` includes it alongside the other hooks
                stages = ["pre-commit" "pre-push"];
                pass_filenames = false;
                always_run = true;
                priority = 0;
              };
            };
          };
        in
          pkgs.mkShell {
            nativeBuildInputs = with pkgs;
              [
                # Include the `treefmt` command and the wrapped programs
                config.treefmt.build.wrapper

                # Rust toolchain from nixpkgs
                rustc
                cargo
                clippy
                rust-analyzer
                rustfmt

                cargo-edit
                prek
                git-cliff
                committed

                # Temporarily disable vendored sources to run `cargo upgrade`, then re-vendor.
                (pkgs.writeShellApplication {
                  name = "cargo-upgrade-vendored";
                  runtimeInputs = with pkgs; [cargo cargo-edit];
                  text = ''
                    set -euo pipefail
                    trap 'mv .cargo/config.toml.bak .cargo/config.toml 2>/dev/null || true' EXIT
                    mv .cargo/config.toml .cargo/config.toml.bak
                    cargo upgrade "$@"
                    mv .cargo/config.toml.bak .cargo/config.toml
                    trap - EXIT
                    cargo vendor --locked vendor >/dev/null
                  '';
                })
              ]
              # Include all the wrapped programs from the Treefmt and Git hooks configs
              ++ hooks.enabledPackages
              ++ (lib.attrValues config.treefmt.build.programs);

            shellHook = ''
              ${hooks.shellHook}
            '';
          };

        treefmt = {
          projectRootFile = "flake.nix";
          # Don't mess with our vendored dependencies
          settings = {
            excludes = ["vendor/**"];
            formatter.yamllint.options = pkgs.lib.mkAfter ["--strict"];
          };
          programs = {
            # Rust
            rustfmt.enable = true;

            # Nix
            alejandra.enable = true;

            # Yaml
            yamlfmt.enable = true;
            yamllint.enable = true;
          };
        };
      };
    };
}
