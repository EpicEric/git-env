{
  system ? builtins.currentSystem,
  sources ? import ./npins,
  pkgs ? import sources.nixpkgs {
    inherit system;
    overlays = [ (import sources.rust-overlay) ];
  },
  craneLib ? (import sources.crane { inherit pkgs; }).overrideToolchain (
    ps: ps.rust-bin.stable.latest.default
  ),
}:
let
  src = craneLib.cleanCargoSource ../.;

  commonArgs = {
    inherit src;
    strictDeps = true;

    nativeBuildInputs = [ pkgs.pkg-config ];
  };

  cargoArtifacts = craneLib.buildDepsOnly commonArgs;

  git-env = craneLib.buildPackage (
    commonArgs
    // {
      inherit cargoArtifacts;
      meta.mainProgram = "git-env";
    }
  );
in
{
  inherit pkgs;

  packages = {
    inherit git-env;
    default = git-env;
  };

  devShell = craneLib.devShell { };
}
