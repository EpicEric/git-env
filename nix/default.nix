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
      nativeBuildInputs = commonArgs.nativeBuildInputs ++ [ pkgs.installShellFiles ];
      meta.mainProgram = "git-env";
      postInstall = ''
        installShellCompletion --cmd git-env \
          --bash <($out/bin/git-env completions bash) \
          --fish <($out/bin/git-env completions fish) \
          --zsh <($out/bin/git-env completions zsh)
      '';
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
