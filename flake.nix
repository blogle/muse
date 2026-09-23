{
  description = "MUSE reproducible development environment and Rust checks";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    crane = {
      url = "github:ipetkov/crane";
    };
  };

  outputs = { self, nixpkgs, crane }:
    let
      systems = [ "aarch64-darwin" "aarch64-linux" "x86_64-linux" ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      devShells = forAllSystems (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          perf = pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux [ pkgs.perf ];
          playwrightFontConfig = pkgs.makeFontsConf {
            fontDirectories = [ pkgs.dejavu_fonts.minimal ];
            impureFontDirectories = [ pkgs.dejavu_fonts.minimal ];
            includes = [ ];
          };
        in {
          # Tooling only: entering this shell never realizes the application.
          default = pkgs.mkShell {
            packages = [
              pkgs.cargo
              pkgs.cargo-deny
              pkgs.cargo-nextest
              pkgs.clippy
              pkgs.coz
              pkgs.fontconfig
              pkgs.git
              pkgs.just
              pkgs.nodejs
              pkgs.playwright-driver
              pkgs.playwright-test
              pkgs.pnpm
              pkgs.rustc
              pkgs.rustfmt
            ] ++ perf;
            PLAYWRIGHT_BROWSERS_PATH = "${pkgs.playwright-driver.browsers}";
            PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD = "1";
            FONTCONFIG_FILE = "${playwrightFontConfig}";
          };
        });

      checks = forAllSystems (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          craneLib = crane.mkLib pkgs;
          src = craneLib.cleanCargoSource ./.;
          commonArgs = {
            inherit src;
            strictDeps = true;
          };
          cargoArtifacts = craneLib.buildDepsOnly commonArgs;
        in {
          fmt = craneLib.cargoFmt commonArgs;
          check = craneLib.cargoBuild (commonArgs // {
            inherit cargoArtifacts;
            cargoExtraArgs = "--workspace";
            cargoBuildCommand = "cargoWithProfile check";
          });
          clippy = craneLib.cargoClippy (commonArgs // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--workspace --all-targets --all-features -- -D warnings";
          });
          nextest = craneLib.cargoNextest (commonArgs // {
            inherit cargoArtifacts;
            partitions = 1;
            partitionType = "count";
          });
          release = craneLib.cargoBuild (commonArgs // {
            inherit cargoArtifacts;
            cargoExtraArgs = "--workspace";
          });
          bench = craneLib.cargoBuild (commonArgs // {
            inherit cargoArtifacts;
            cargoExtraArgs = "--workspace --benches";
          });
        });
    };
}
