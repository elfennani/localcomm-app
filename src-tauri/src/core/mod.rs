pub mod device;
pub mod cert_gen;


/*
Generate these files using `./scripts/generate-root-ca.sh`
 */

pub const ROOT_CA_CERT: &[u8] = include_bytes!("../../certs/rootCA.crt");
pub const ROOT_CA_KEY: &[u8] = include_bytes!("../../certs/rootCA.key");