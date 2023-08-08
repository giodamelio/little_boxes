{
  description = "Adds boxes around stdin. Optionally adds a title";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nixago = {
      url = "github:nix-community/nixago";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
  outputs = inputs @ {
    flake-parts,
    nixago,
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
            cargoLock.lockFile = ./Cargo.lock;
            src = pkgs.lib.cleanSource ./.;

            # Install the manpage
            postBuild = ''
              export manfilepath=$(find target/ -type f -wholename "*out/little_boxes.1" | head -n 1)
              mv $manfilepath .
            '';
            postInstall = ''
              mkdir -p $out/share/man/man1
              cp little_boxes.1 $out/share/man/man1/
            '';

            meta = with pkgs.lib; {
              description = manifest.description;
              homepage = manifest.homepage;
              license = licenses.mit;
              maintainers = [maintainers.giodamelio];
              manpages = ["man/man1/little_boxes.1"];
            };
          };

        devShells.default = let
          lefthookConfig = nixago.lib.${system}.make {
            data = {
              pre-commit = {
                commands = {
                  format = {
                    run = "treefmt --fail-on-change";
                  };
                  check = {
                    run = "cargo check";
                  };
                  clippy = {
                    run = "cargo clippy -- -D warnings";
                  };
                };
              };
              pre-push = {
                commands = {
                  test = {
                    run = "cargo test";
                  };
                  check-flake = {
                    run = "nix flake check";
                  };
                };
              };
            };
            output = "lefthook.yaml";
            format = "yaml";
          };
        in
          pkgs.mkShell {
            nativeBuildInputs = with pkgs; [
              # Include the `treefmt` command
              config.treefmt.build.wrapper

              rustc
              cargo
              cargo-edit
              rust-analyzer # LSP Server
              clippy # Linter
              lefthook # Git hook manager
            ];

            shellHook = ''
              ${lefthookConfig.shellHook}
              ${pkgs.lefthook}/bin/lefthook install
            '';
          };

        checks = {
          #testing = "ls";
        };

        treefmt = {
          projectRootFile = ./flake.nix;
          # Don't mess with our vendored dependencies
          settings = {
            global.excludes = ["./vendor/**"];
          };
          programs = {
            # Rust
            rustfmt = {
              enable = true;
              edition = "2021";
            };

            # Nix
            alejandra.enable = true;
          };
        };
      };
    };
}
