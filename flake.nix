{
  description = "routefilterd";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }: {
    overlay = (final: prev: let
      cargoFile = builtins.fromTOML (builtins.readFile ./Cargo.toml);
      pkgs = import nixpkgs {
        inherit (final.stdenv.hostPlatform) system;
      };
    in
      {
        routefilterd = pkgs.rustPackages.rustPlatform.callPackage ./package.nix { routefilterd-version = cargoFile.package.version; };
      });
  } // (flake-utils.lib.eachDefaultSystem (system:
    let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [ self.overlay ];
      };
    in
      {
        packages = {
          routefilterd = pkgs.routefilterd;
          default = pkgs.routefilterd;
        };
        devShell = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            cargo
            rustc
            rustfmt
          ];
        };
      }
  ));
}
