{
  description = "Muse Rust development environment";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { nixpkgs, ... }:
    let
      systems = [ "aarch64-darwin" "aarch64-linux" "x86_64-darwin" "x86_64-linux" ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      devShells = forAllSystems (system:
        let pkgs = nixpkgs.legacyPackages.${system};
        in {
          # This shell only supplies tools; it has no application build input.
          default = pkgs.mkShell {
            packages = with pkgs; [
              cargo
              clippy
              git
              rustc
              rustfmt
            ];
          };
        });
    };
}
