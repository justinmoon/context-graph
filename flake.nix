{
  description = "Rust project with CI setup";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
  };

  outputs = { self, nixpkgs, flake-utils, crane }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        
        craneLib = crane.mkLib pkgs;
        
        src = craneLib.cleanCargoSource (craneLib.path ./.);
        
        commonArgs = {
          inherit src;
          strictDeps = true;
          nativeBuildInputs = with pkgs; [
            pkg-config
          ];
          buildInputs = with pkgs; [
            openssl
          ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
            pkgs.libiconv
            pkgs.darwin.apple_sdk.frameworks.Security
            pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
          ];
        };
        
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;
        
        package = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
        });
      in
      {
        packages = {
          default = package;
        };
        
        devShells.default = craneLib.devShell {
          packages = with pkgs; [
            rust-analyzer
            just
            cargo-watch
          ];
        };
        
        apps.ci = {
          type = "app";
          program = "${pkgs.writeShellScript "ci" ''
            export PATH="${pkgs.lib.makeBinPath (with pkgs; [
              cargo
              rustc
              rustfmt
              clippy
              pkgs.stdenv.cc
            ])}:$PATH"
            
            exec ${./scripts/ci.sh}
          ''}";
        };
      }
    );
}
