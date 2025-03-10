{ pkgs ? import <nixpkgs> { } }:
let
  gdb-wrapped = pkgs.writeShellScriptBin "gdb" ''
    ${pkgs.gdb}/bin/gdb -q -iex 'add-auto-load-safe-path .gdbinit' "$@"
  '';
in
pkgs.mkShell
{
  nativeBuildInputs = with pkgs; [ rustc cargo gcc rustfmt clippy nasm gdb-wrapped ];

  RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
}
