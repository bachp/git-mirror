{
  description = "A flake for building the git-mirror project";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs";
  };

  outputs = { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs {
        inherit system;
      };
    in
    {
      devShells.${system}.default = pkgs.mkShell {
        buildInputs = with pkgs; [
          rustc
          cargo
          openssl.dev
          gcc
        ];

        shellHook = ''
          echo "Welcome to the git-mirror development environment!"
        '';
      };
    };
}