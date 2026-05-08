use openssl::asn1::Asn1Time;
use openssl::ec::{EcGroup, EcKey};
use openssl::hash::MessageDigest;
use openssl::nid::Nid;
use openssl::pkey::PKey;
use openssl::stack::Stack;
use openssl::x509::extension::{
    BasicConstraints, ExtendedKeyUsage, KeyUsage, SubjectAlternativeName,
};
use openssl::x509::{X509NameBuilder, X509Req, X509ReqBuilder, X509};
use std::fs;
use std::path::PathBuf;

/// Since I'm not familiar much with OpenSSL, and AI giving me conflicting
/// and confusing answers, I relied on [this Microsoft guide](https://learn.microsoft.com/en-us/azure/application-gateway/self-signed-certificates)
/// to understand what needs to be done. I provided each command separately
/// to ChatGPT to generate Rust functions using `openssl` crate, then manually
/// modified each function to my need.
///
/// NOTE:   Root certificate authority generation has been dealt with separately
///         in a script `./scripts/generate-root-ca.sh`.

pub fn gen_server_cert_key(dir: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    // Equivalent to: -name prime256v1
    let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1)?;

    // Equivalent to: -genkey
    let ec_key = EcKey::generate(&group)?;

    // Convert private key to PEM format
    let pem = ec_key.private_key_to_pem()?;

    // Equivalent to: -out fabrikam.key
    let file = dir.join("server.key");
    fs::write(file, pem)?;

    println!("EC private key generated: server.key");

    Ok(())
}

pub fn gen_csr(dir: &PathBuf, address: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    // Load EC key
    let key_pem = fs::read(dir.join("server.key"))?;
    let ec_key = EcKey::private_key_from_pem(&key_pem)?;
    let pkey = PKey::from_ec_key(ec_key)?;

    // Subject (matches req_distinguished_name)
    let mut name = X509NameBuilder::new()?;
    name.append_entry_by_text("C", "MO")?;
    name.append_entry_by_text("O", "Elfennani")?;
    name.append_entry_by_text("CN", "localhost")?;
    let name = name.build();

    let mut req = X509ReqBuilder::new()?;
    req.set_subject_name(&name)?;
    req.set_pubkey(&pkey)?;

    // Equivalent to:
    // subjectAltName=@alt_names
    let mut stack = Stack::new()?;
    stack.push(
        SubjectAlternativeName::new()
            .dns("localhost")
            .build(&req.x509v3_context(None))?,
    )?;
    stack.push(
        SubjectAlternativeName::new()
            .dns("0.0.0.0")
            .build(&req.x509v3_context(None))?,
    )?;
    if let Some(address) = address {
        stack.push(
            SubjectAlternativeName::new()
                .dns(address.as_str())
                .build(&req.x509v3_context(None))?,
        )?;
    }

    req.add_extensions(&stack)?;

    // Sign with SHA256
    req.sign(&pkey, MessageDigest::sha256())?;

    let csr = req.build();

    fs::write(dir.join("server.csr"), csr.to_pem()?)?;

    println!("CSR generated: server.csr");

    Ok(())
}

pub fn sign_csr(
    dir: &PathBuf,
    ca_cert_pem: String,
    ca_key_pem: String,
) -> Result<(), Box<dyn std::error::Error>> {
    // Inputs (as String paths or actual PEM strings depending on your app)
    let csr_pem = fs::read_to_string(dir.join("server.csr"))?;

    // Parse CA cert + key
    let ca_cert = X509::from_pem(ca_cert_pem.as_bytes())?;
    let ca_key = PKey::private_key_from_pem(ca_key_pem.as_bytes())?;

    // Parse CSR
    let csr = X509Req::from_pem(csr_pem.as_bytes())?;

    // Build certificate
    let mut builder = X509::builder()?;

    // Set subject from CSR
    builder.set_subject_name(csr.subject_name())?;

    // Set issuer from CA cert
    builder.set_issuer_name(ca_cert.subject_name())?;

    // Set public key from CSR
    let pubkey = csr.public_key()?;
    builder.set_pubkey(&pubkey)?;

    // Validity: now -> +365 days
    let not_before = Asn1Time::days_from_now(0)?;
    let not_after = Asn1Time::days_from_now(365)?;
    builder.set_not_before(&not_before)?;
    builder.set_not_after(&not_after)?;

    // Mark as end-entity (NOT CA)
    builder.append_extension(BasicConstraints::new().critical().build()?)?;

    // Required for TLS server certs
    builder.append_extension(
        KeyUsage::new()
            .digital_signature()
            .key_encipherment()
            .build()?,
    )?;

    // TLS server usage
    builder.append_extension(ExtendedKeyUsage::new().server_auth().build()?)?;

    let mut san = SubjectAlternativeName::new();

    san.dns("localhost");
    san.ip("127.0.0.1");
    san.ip("0.0.0.0");

    let san_ext = san.build(&builder.x509v3_context(Some(&ca_cert), None))?;

    builder.append_extension(san_ext)?;

    // Sign with CA key (SHA256)
    builder.sign(&ca_key, MessageDigest::sha256())?;

    let cert = builder.build();

    fs::write(dir.join("server.crt"), cert.to_pem()?)?;

    println!("Certificate generated: server.crt");

    Ok(())
}
