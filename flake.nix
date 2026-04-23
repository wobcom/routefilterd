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
      minimal-dev-pkgs = with pkgs; [
        clippy
        cargo
        rustc
        libllvm
        grcov
        lcov
      ];
      coverageScript = pkgs: pkgs.writeShellApplication {
        name = "gen-coverage";
        runtimeInputs = minimal-dev-pkgs;
        runtimeEnv = {
          LLVM_PROFILE_FILE = "grcov-%p-%m.profraw";
          RUSTFLAGS = "-Cinstrument-coverage -Copt-level=0 -Coverflow-checks=off";
        };
        # --llvm-path is needed since libllvm
        #   will be in nix store,
        #   which grcov doesn't expect.
        # --html-resources cdn is needed
        #   bc we don't have a local HTTP server
        text = ''
             cargo test
             mkdir -p target/coverage
             grcov "$(find . -name 'grcov-*.profraw' -print)" \
                --llvm-path ${pkgs.libllvm}/bin \
                --source-dir src \
                --binary-path target/debug/ \
                --output-types lcov,html \
                --html-resources cdn \
                --branch --ignore-not-existing \
                --ignore "/*" \
                -o target/coverage/
             lcov -l target/coverage/lcov
             rm ./*.profraw
             '';
      };
    in {
      packages = {
        routefilterd = pkgs.routefilterd;
        default = pkgs.routefilterd;
        gen_coverage = coverageScript pkgs;
      };
      devShell = pkgs.mkShell {
        LLVM_COV = "${pkgs.libllvm}/bin/llvm-cov"; # needed for cargo-llvm-cov
        LLVM_PROFDATA = "${pkgs.libllvm}/bin/llvm-profdata"; # same as above
        nativeBuildInputs = minimal-dev-pkgs ++ [ pkgs.cargo-llvm-cov (coverageScript pkgs) ];
      };
    }
  ));
}
