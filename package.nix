{
  buildRustPackage,
  routefilterd-version,
}:

buildRustPackage rec {
  pname = "routefilterd";
  version = routefilterd-version;
  
  cargoLock.lockFile = ./Cargo.lock;
  src = ./.;
}
