{
  description = "Use your SSH keys to keep your sensitive data encrypted with your git repository";

  inputs = {
    crane = {
      url = "path:./nix/dummy.nix";
      flake = false;
    };
    nixpkgs = {
      url = "path:./nix/dummy.nix";
      flake = false;
    };
    rust-overlay = {
      url = "path:./nix/dummy.nix";
      flake = false;
    };
  };

  outputs =
    { self, ... }@inputs:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];

      eachSystem =
        f:
        (builtins.foldl' (
          acc: system:
          let
            fSystem = f system;
          in
          builtins.foldl' (
            acc': attr:
            acc'
            // {
              ${attr} = (acc'.${attr} or { }) // fSystem.${attr};
            }
          ) acc (builtins.attrNames fSystem)
        ) { } systems);
    in
    eachSystem (
      system:
      let
        sources = import ./npins;
        importInput =
          attr:
          if import inputs.${attr} ? __isDummyInput then import sources.${attr} else import inputs.${attr};
        pkgs = importInput "nixpkgs" {
          inherit system;
          overlays = [ (importInput "rust-overlay") ];
        };
        craneLib = (importInput "crane" { inherit pkgs; }).overrideToolchain (
          p: p.rust-bin.stable.latest.default
        );

        inherit
          (import ./nix {
            inherit
              system
              pkgs
              craneLib
              ;
          })
          packages
          devShell
          ;

        inherit (pkgs) lib;
      in
      {
        packages.${system} = packages;

        apps.${system}.default = {
          type = "app";
          program = lib.getExe self.packages.${system}.default;
          meta = {
            name = "git-env";
            description = "Use your SSH keys to keep your sensitive data encrypted with your git repository";
            homepage = "https://github.com/EpicEric/git-env";
            license = lib.licenses.mit;
            mainProgram = "git-env";
            platforms = lib.platforms.linux ++ lib.platforms.darwin;
          };
        };

        devShells.${system}.default = devShell;
      }
    );
}
