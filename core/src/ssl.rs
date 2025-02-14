use der::asn1::{Ia5String, UtcTime};
use der::pem::LineEnding;
use der::{DateTime, EncodePem};
use log::{error, info};
use rand::Rng;
use rsa::pkcs1::{DecodeRsaPrivateKey, EncodeRsaPrivateKey};
use rsa::pkcs1v15::SigningKey;
use rsa::pkcs8::EncodePublicKey;
use sha2::Sha256;
use std::collections::HashMap;
use std::fs;
use std::fs::{create_dir_all, OpenOptions};
use std::io::{Error, ErrorKind, Write};
use std::ops::{Add, Sub};
use std::path::Path;
use std::str::FromStr;
use std::time::{Duration, SystemTime};
use x509_cert::builder::{Builder, CertificateBuilder, Profile};
use x509_cert::der::DecodePem;
use x509_cert::ext::pkix::name::GeneralName;
use x509_cert::ext::pkix::{BasicConstraints, SubjectAltName};
use x509_cert::name::Name;
use x509_cert::serial_number::SerialNumber;
use x509_cert::spki::SubjectPublicKeyInfo;
use x509_cert::time::{Time, Validity};
use x509_cert::Certificate;

pub const CHIA_CA_CRT: &str = r"-----BEGIN CERTIFICATE-----
MIIDKTCCAhGgAwIBAgIUXIpxI5MoZQ65/vhc7DK/d5ymoMUwDQYJKoZIhvcNAQEL
BQAwRDENMAsGA1UECgwEQ2hpYTEQMA4GA1UEAwwHQ2hpYSBDQTEhMB8GA1UECwwY
T3JnYW5pYyBGYXJtaW5nIERpdmlzaW9uMB4XDTIxMDEyMzA4NTEwNloXDTMxMDEy
MTA4NTEwNlowRDENMAsGA1UECgwEQ2hpYTEQMA4GA1UEAwwHQ2hpYSBDQTEhMB8G
A1UECwwYT3JnYW5pYyBGYXJtaW5nIERpdmlzaW9uMIIBIjANBgkqhkiG9w0BAQEF
AAOCAQ8AMIIBCgKCAQEAzz/L219Zjb5CIKnUkpd2julGC+j3E97KUiuOalCH9wdq
gpJi9nBqLccwPCSFXFew6CNBIBM+CW2jT3UVwgzjdXJ7pgtu8gWj0NQ6NqSLiXV2
WbpZovfrVh3x7Z4bjPgI3ouWjyehUfmK1GPIld4BfUSQtPlUJ53+XT32GRizUy+b
0CcJ84jp1XvyZAMajYnclFRNNJSw9WXtTlMUu+Z1M4K7c4ZPwEqgEnCgRc0TCaXj
180vo7mCHJQoDiNSCRATwfH+kWxOOK/nePkq2t4mPSFaX8xAS4yILISIOWYn7sNg
dy9D6gGNFo2SZ0FR3x9hjUjYEV3cPqg3BmNE3DDynQIDAQABoxMwETAPBgNVHRMB
Af8EBTADAQH/MA0GCSqGSIb3DQEBCwUAA4IBAQAEugnFQjzHhS0eeCqUwOHmP3ww
/rXPkKF+bJ6uiQgXZl+B5W3m3zaKimJeyatmuN+5ST1gUET+boMhbA/7grXAsRsk
SFTHG0T9CWfPiuimVmGCzoxLGpWDMJcHZncpQZ72dcy3h7mjWS+U59uyRVHeiprE
hvSyoNSYmfvh7vplRKS1wYeA119LL5fRXvOQNW6pSsts17auu38HWQGagSIAd1UP
5zEvDS1HgvaU1E09hlHzlpdSdNkAx7si0DMzxKHUg9oXeRZedt6kcfyEmryd52Mj
1r1R9mf4iMIUv1zc2sHVc1omxnCw9+7U4GMWLtL5OgyJyfNyoxk3tC+D3KNU
-----END CERTIFICATE-----";

pub const CHIA_CA_KEY: &str = r"-----BEGIN RSA PRIVATE KEY-----
MIIEowIBAAKCAQEAzz/L219Zjb5CIKnUkpd2julGC+j3E97KUiuOalCH9wdqgpJi
9nBqLccwPCSFXFew6CNBIBM+CW2jT3UVwgzjdXJ7pgtu8gWj0NQ6NqSLiXV2WbpZ
ovfrVh3x7Z4bjPgI3ouWjyehUfmK1GPIld4BfUSQtPlUJ53+XT32GRizUy+b0CcJ
84jp1XvyZAMajYnclFRNNJSw9WXtTlMUu+Z1M4K7c4ZPwEqgEnCgRc0TCaXj180v
o7mCHJQoDiNSCRATwfH+kWxOOK/nePkq2t4mPSFaX8xAS4yILISIOWYn7sNgdy9D
6gGNFo2SZ0FR3x9hjUjYEV3cPqg3BmNE3DDynQIDAQABAoIBAGupS4BJdx8gEAAh
2VDRqAAzhHTZb8j9uoKXJ+NotEkKrDTqUMiOu0nOqOsFWdYPo9HjxoggFuEU+Hpl
a4kj4uF3OG6Yj+jgLypjpV4PeoFM6M9R9BCp07In2i7DLLK9gvYA85SoVLBd/tW4
hFH+Qy3M+ZNZ1nLCK4pKjtaYs0dpi5zLoVvpEcEem2O+aRpUPCZqkNwU0umATCfg
ZGfFzgXI/XPJr8Uy+LVZOFp3PXXHfnZZD9T5AjO/ViBeqbMFuWQ8BpVOqapNPKj8
xDY3ovw3uiAYPC7eLib3u/WoFelMc2OMX0QljLp5Y+FScFHAMxoco3AQdWSYvSQw
b5xZmg0CgYEA6zKASfrw3EtPthkLR5NBmesI4RbbY6iFVhS5loLbzTtStvsus8EI
6RQgLgAFF14H21YSHxb6dB1Mbo45BN83gmDpUvKPREslqD3YPMKFo5GXMmv+JhNo
5Y9fhiOEnxzLJGtBB1HeGmg5NXp9mr2Ch9u8w/slfuCHckbA9AYvdxMCgYEA4ZR5
zg73+UA1a6Pm93bLYZGj+hf7OaB/6Hiw9YxCBgDfWM9dJ48iz382nojT5ui0rClV
5YAo8UCLh01Np9AbBZHuBdYm9IziuKNzTeK31UW+Tvbz+dEx7+PlYQffNOhcIgd+
9SXjoZorQksImKdMGZld1lEReHuBawq92JQvtY8CgYEAtNwUws7xQLW5CjKf9d5K
5+1Q2qYU9sG0JsmxHQhrtZoUtRjahOe/zlvnkvf48ksgh43cSYQF/Bw7lhhPyGtN
6DhVs69KdB3FS2ajTbXXxjxCpEdfHDB4zW4+6ouNhD1ECTFgxBw0SuIye+lBhSiN
o6NZuOr7nmFSRpIZ9ox7G3kCgYA4pvxMNtAqJekEpn4cChab42LGLX2nhFp7PMxc
bqQqM8/j0vg3Nihs6isCd6SYKjstvZfX8m7V3/rquQxWp9oRdQvNJXJVGojaDBqq
JdU7V6+qzzSIufQLpjV2P+7br7trxGwrDx/y9vAETynShLmE+FJrv6Jems3u3xy8
psKwmwKBgG5uLzCyMvMB2KwI+f3np2LYVGG0Pl1jq6yNXSaBosAiF0y+IgUjtWY5
EejO8oPWcb9AbqgPtrWaiJi17KiKv4Oyba5+y36IEtyjolWt0AB6F3oDK0X+Etw8
j/xlvBNuzDL6gRJHQg1+d4dO8Lz54NDUbKW8jGl+N/7afGVpGmX9
-----END RSA PRIVATE KEY-----";

pub fn generate_ca_signed_cert(
    cert_path: &Path,
    cert_data: &str,
    key_path: &Path,
    key_data: &str,
) -> Result<(String, String), Error> {
    let (cert_data, key_data) = generate_ca_signed_cert_data(cert_data, key_data)
        .map_err(|e| Error::new(ErrorKind::Other, format!("OpenSSL Errors: {:?}", e)))?;
    write_ssl_cert_and_key(cert_path, &cert_data, key_path, &key_data, true)?;
    Ok((cert_data, key_data))
}

fn write_ssl_cert_and_key(
    cert_path: &Path,
    cert_data: &str,
    key_path: &Path,
    key_data: &str,
    overwrite: bool,
) -> Result<(), Error> {
    if cert_path.exists() && overwrite {
        fs::remove_file(cert_path)?;
    }
    let mut crt = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(cert_path)?;
    crt.write_all(cert_data.as_bytes())?;
    crt.flush()?;
    if key_path.exists() && overwrite {
        fs::remove_file(key_path)?;
    }
    let mut key = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(key_path)?;
    key.write_all(key_data.as_bytes())?;
    key.flush()
}

pub fn generate_ca_signed_cert_data(
    cert_data: &str,
    key_data: &str,
) -> Result<(String, String), Error> {
    let root_cert = Certificate::from_pem(cert_data.as_bytes())
        .map_err(|e| Error::new(ErrorKind::Other, format!("{e:?}")))?;
    let root_key = rsa::RsaPrivateKey::from_pkcs1_pem(key_data)
        .map_err(|e| Error::new(ErrorKind::Other, format!("{e:?}")))?;
    let mut rng = rand::thread_rng();
    let cert_key = rsa::RsaPrivateKey::new(&mut rng, 2048)
        .map_err(|e| Error::new(ErrorKind::Other, format!("{e:?}")))?;
    let pub_key = cert_key.to_public_key();
    let signing_key: SigningKey<Sha256> = SigningKey::new(root_key);
    let subject_pub_key = SubjectPublicKeyInfo::from_pem(
        pub_key
            .to_public_key_pem(LineEnding::LF)
            .map_err(|e| Error::new(ErrorKind::Other, format!("{e:?}")))?
            .as_bytes(),
    )
    .map_err(|e| Error::new(ErrorKind::Other, format!("{e:?}")))?;
    let mut cert = CertificateBuilder::new(
        Profile::Leaf {
            issuer: root_cert.tbs_certificate.issuer,
            enable_key_agreement: false,
            enable_key_encipherment: false,
        },
        SerialNumber::from(rng.gen::<u32>()),
        Validity {
            not_before: Time::UtcTime(
                UtcTime::from_system_time(SystemTime::now().sub(Duration::from_secs(60 * 60 * 24)))
                    .map_err(|e| Error::new(ErrorKind::Other, format!("{e:?}")))?,
            ),
            not_after: Time::UtcTime(
                UtcTime::from_date_time(
                    DateTime::new(2049, 8, 2, 0, 0, 0)
                        .map_err(|e| Error::new(ErrorKind::Other, format!("{e:?}")))?,
                )
                .map_err(|e| Error::new(ErrorKind::Other, format!("{e:?}")))?,
            ),
        },
        Name::from_str("CN=Chia,O=Chia,OU=Organic Farming Division")
            .map_err(|e| Error::new(ErrorKind::Other, format!("{e:?}")))?,
        subject_pub_key,
        &signing_key,
    )
    .map_err(|e| Error::new(ErrorKind::Other, format!("{e:?}")))?;
    cert.add_extension(&SubjectAltName(vec![GeneralName::DnsName(
        Ia5String::new("chia.net").map_err(|e| Error::new(ErrorKind::Other, format!("{e:?}")))?,
    )]))
    .map_err(|e| Error::new(ErrorKind::Other, format!("{e:?}")))?;
    let cert = cert
        .build()
        .map_err(|e| Error::new(ErrorKind::Other, format!("{e:?}")))?;
    Ok((
        cert.to_pem(LineEnding::LF)
            .map_err(|e| Error::new(ErrorKind::Other, format!("{e:?}")))?,
        cert_key
            .to_pkcs1_pem(LineEnding::LF)
            .map_err(|e| Error::new(ErrorKind::Other, format!("{e:?}")))?
            .to_string(),
    ))
}

pub fn make_ca_cert(cert_path: &Path, key_path: &Path) -> Result<(String, String), Error> {
    let (cert_data, key_data) = make_ca_cert_data()
        .map_err(|e| Error::new(ErrorKind::Other, format!("OpenSSL Errors: {:?}", e)))?;
    write_ssl_cert_and_key(cert_path, &cert_data, key_path, &key_data, true)?;
    Ok((cert_data, key_data))
}

fn make_ca_cert_data() -> Result<(String, String), Error> {
    let mut rng = rand::thread_rng();
    let root_key = rsa::RsaPrivateKey::new(&mut rng, 2048).expect("failed to generate a key");
    let pub_key = root_key.to_public_key();
    let signing_key: SigningKey<Sha256> = SigningKey::new(root_key.clone());
    let name = Name::from_str("CN=Chia CA,O=Chia,OU=Organic Farming Division")
        .map_err(|e| Error::new(ErrorKind::Other, format!("{e:?}")))?;
    let subject_pub_key = SubjectPublicKeyInfo::from_pem(
        pub_key
            .to_public_key_pem(LineEnding::LF)
            .map_err(|e| Error::new(ErrorKind::Other, format!("{e:?}")))?
            .as_bytes(),
    )
    .map_err(|e| Error::new(ErrorKind::Other, format!("{e:?}")))?;
    let mut cert = CertificateBuilder::new(
        Profile::SubCA {
            issuer: name.clone(),
            path_len_constraint: None,
        },
        SerialNumber::from(rng.gen::<u32>()),
        Validity {
            not_before: Time::UtcTime(
                UtcTime::from_system_time(SystemTime::UNIX_EPOCH)
                    .map_err(|e| Error::new(ErrorKind::Other, format!("{e:?}")))?,
            ),
            not_after: Time::UtcTime(
                UtcTime::from_system_time(
                    SystemTime::now().add(Duration::from_secs(60 * 60 * 24 * 3650)),
                )
                .map_err(|e| Error::new(ErrorKind::Other, format!("{e:?}")))?,
            ),
        },
        name,
        subject_pub_key,
        &signing_key,
    )
    .map_err(|e| Error::new(ErrorKind::Other, format!("{e:?}")))?;
    cert.add_extension(&BasicConstraints {
        ca: true,
        path_len_constraint: None,
    })
    .map_err(|e| Error::new(ErrorKind::Other, format!("{e:?}")))?;
    Ok((
        cert.build()
            .map_err(|e| Error::new(ErrorKind::Other, format!("{e:?}")))?
            .to_pem(LineEnding::LF)
            .map_err(|e| Error::new(ErrorKind::Other, format!("{e:?}")))?,
        root_key
            .to_pkcs1_pem(LineEnding::LF)
            .map_err(|e| Error::new(ErrorKind::Other, format!("{e:?}")))?
            .to_string(),
    ))
}

const ALL_PRIVATE_NODE_NAMES: [&str; 8] = [
    "full_node",
    "wallet",
    "farmer",
    "harvester",
    "timelord",
    "crawler",
    "data_layer",
    "daemon",
];

const ALL_PUBLIC_NODE_NAMES: [&str; 6] = [
    "full_node",
    "wallet",
    "farmer",
    "introducer",
    "timelord",
    "data_layer",
];

pub struct MemorySSL {
    pub public: HashMap<String, MemoryNodeSSL>,
    pub private: HashMap<String, MemoryNodeSSL>,
}

pub struct MemoryNodeSSL {
    pub cert: Vec<u8>,
    pub key: Vec<u8>,
}

pub fn create_all_ssl_memory() -> Result<MemorySSL, Error> {
    info!("Generating CA Certs");
    let mut public_map = HashMap::new();
    let mut private_map = HashMap::new();
    let (ca_cert_data, ca_key_data) = make_ca_cert_data()
        .map_err(|e| Error::new(ErrorKind::Other, format!("OpenSSL Errors: {:?}", e)))?;
    info!("Generating Private Certs");
    let private_certs =
        generate_ssl_for_nodes_in_memory(&ca_cert_data, &ca_key_data, &ALL_PRIVATE_NODE_NAMES)?;
    private_map.insert(
        "ca".to_string(),
        MemoryNodeSSL {
            cert: ca_cert_data.into_bytes(),
            key: ca_key_data.into_bytes(),
        },
    );
    private_map.extend(private_certs);
    info!("Generating Public Certs");
    let public_certs =
        generate_ssl_for_nodes_in_memory(CHIA_CA_CRT, CHIA_CA_KEY, &ALL_PUBLIC_NODE_NAMES)?;
    public_map.insert(
        "ca".to_string(),
        MemoryNodeSSL {
            cert: CHIA_CA_CRT.as_bytes().to_vec(),
            key: CHIA_CA_KEY.as_bytes().to_vec(),
        },
    );
    public_map.extend(public_certs);
    Ok(MemorySSL {
        public: public_map,
        private: private_map,
    })
}

pub fn create_all_ssl(ssl_dir: &Path, overwrite: bool) -> Result<(), Error> {
    let ca_dir = ssl_dir.join(Path::new("ca"));
    create_dir_all(&ca_dir)?;
    let private_ca_key_path = ca_dir.join("private_ca.key");
    let private_ca_crt_path = ca_dir.join("private_ca.crt");
    let chia_ca_crt_path = ca_dir.join("chia_ca.crt");
    let chia_ca_key_path = ca_dir.join("chia_ca.key");
    write_ssl_cert_and_key(
        &chia_ca_crt_path,
        CHIA_CA_CRT,
        &chia_ca_key_path,
        CHIA_CA_KEY,
        true,
    )?;
    let (crt, key) = if !private_ca_crt_path.exists() || !private_ca_key_path.exists() {
        info!("Generating SSL CA Cert");
        make_ca_cert(&private_ca_crt_path, &private_ca_key_path)?
    } else {
        info!("Loading SSL CA Cert");
        (
            fs::read_to_string(private_ca_crt_path)?,
            fs::read_to_string(private_ca_key_path)?,
        )
    };
    info!("Generating Private Certs");
    generate_ssl_for_nodes(
        ssl_dir,
        &crt,
        &key,
        "private",
        &ALL_PRIVATE_NODE_NAMES,
        overwrite,
    )?;
    info!("Generating Public Certs");
    generate_ssl_for_nodes(
        ssl_dir,
        CHIA_CA_CRT,
        CHIA_CA_KEY,
        "public",
        &ALL_PUBLIC_NODE_NAMES,
        overwrite,
    )
}

fn generate_ssl_for_nodes(
    ssl_dir: &Path,
    crt: &str,
    key: &str,
    prefix: &str,
    nodes: &[&str],
    overwrite: bool,
) -> Result<(), Error> {
    for node_name in nodes {
        let node_dir = ssl_dir.join(Path::new(*node_name));
        create_dir_all(&node_dir)?;
        let crt_path = node_dir.join(Path::new(&format!("{prefix}_{node_name}.crt")));
        let key_path = node_dir.join(Path::new(&format!("{prefix}_{node_name}.key")));
        if key_path.exists() && crt_path.exists() && !overwrite {
            continue;
        }
        if let Err(e) = generate_ca_signed_cert(&crt_path, crt, &key_path, key) {
            error!("Failed to write Cert Files: {:?}", e);
        }
    }
    Ok(())
}

pub fn generate_ssl_for_nodes_in_memory(
    crt: &str,
    key: &str,
    nodes: &[&str],
) -> Result<HashMap<String, MemoryNodeSSL>, Error> {
    let mut map = HashMap::new();
    for node_name in nodes {
        let (cert, key) = generate_ca_signed_cert_data(crt, key)
            .map_err(|e| Error::new(ErrorKind::Other, format!("OpenSSL Errors: {:?}", e)))?;
        map.insert(
            node_name.to_string(),
            MemoryNodeSSL {
                cert: cert.into_bytes(),
                key: key.into_bytes(),
            },
        );
    }
    Ok(map)
}

#[test]
pub fn test_ssl() {
    use simple_logger::SimpleLogger;
    SimpleLogger::new().init().unwrap();
    create_all_ssl("./ssl".as_ref(), true).unwrap();
}
