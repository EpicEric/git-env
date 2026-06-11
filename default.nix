{
  system ? builtins.currentSystem,
  inputs ? import ./.tack,
  pkgs ? import inputs.nixpkgs {
    inherit system;
    overlays = [ (import inputs.rust-overlay) ];
  },
  craneLib ? (import inputs.crane { inherit pkgs; }).overrideToolchain (
    ps: ps.rust-bin.stable.latest.default
  ),
}:
(import ./nix {
  inherit
    system
    inputs
    pkgs
    craneLib
    ;
}).packages.git-env
