{
  buildRustPackage,
  routefilterd-version,
  cacert,
}:

buildRustPackage rec {
  pname = "routefilterd";
  version = routefilterd-version;
  
  cargoLock.lockFile = ./Cargo.lock;
  src = ./.;

  buildInputs = [
     cacert
  ];
}
