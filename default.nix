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
(import ./nix {
  inherit
    system
    sources
    pkgs
    craneLib
    ;
}).packages.git-env
