with import <nixpkgs> {};

{
  doxie-upload = rustPlatform.buildRustPackage {
    pname = "doxie-upload";
    version = "0.2.0-dev";

    cargoLock = {
      lockFile = ./Cargo.lock;

      outputHashes = {
        "multipart-async-0.0.2" = "sha256-C3vrrYf7zeQGfGqHtoCdosWhC+sF3Xmx9g/MEFzrXMc=";
      };
    };

    src = ./.;

    meta = with lib; {
      description = "Simple HTTP server for accepting scans from Doxie scanners";
      homepage = "https://github.com/crawford/doxie-upload";
      license = licenses.agpl3Plus;
    };
  };
}
