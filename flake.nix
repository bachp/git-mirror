{
  description = "A flake for building the git-mirror project";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs {
        inherit system;
      };
      nativeBuildInputs = with pkgs; [
          cargo
          gcc
          pkg-config
          perl
      ];
      buildInputs = with pkgs; [
          openssl.dev
          openssl
      ];
      cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
    in
    {
      devShells.${system}.default = pkgs.mkShell {
        inherit buildInputs;
        inherit nativeBuildInputs;

        shellHook = ''
          echo "Welcome to the git-mirror development environment!"
        '';
      };

      packages.${system}.default = pkgs.rustPlatform.buildRustPackage {
        inherit buildInputs;
        inherit nativeBuildInputs;

        pname = cargoToml.package.name;
        version = cargoToml.package.version;
        src = ./.;
        cargoLock = {
          lockFile = ./Cargo.lock;
        };
      };
    };
}
