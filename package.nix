{
  buildRustPackage,
  routefilterd-version,
  cacert,
  openssl,
  pkg-config,
}:

buildRustPackage rec {
  pname = "routefilterd";
  version = routefilterd-version;
  
  cargoLock.lockFile = ./Cargo.lock;
  src = ./.;

  nativeBuildInputs = [
     pkg-config
  ];
  buildInputs = [
     cacert
     openssl
  ];
}
