//! Offline one-time purchase license verification using Ed25519 (zero network).
//!
//! Format: `TRSA1-<payload_base64url_nopad>-<sig_base64url_nopad>`
//! Signed bytes: `b"TRSA1." + payload_base64url.as_bytes()`.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

/// Embedded Ed25519 public key used by the Trasea desktop app to verify offline license keys.
/// NOTE FOR COMMERCIAL RELEASE: Generate your own private keypair via `trasea-keygen init`,
/// replace this 32-byte public key, keep the private key strictly outside the git repo, and rebuild.
pub const EMBEDDED_PUBLIC_KEY: [u8; 32] = [
    0xab, 0xa9, 0x17, 0xf5, 0xc8, 0x89, 0x5d, 0x77,
    0xbe, 0x81, 0x4f, 0xc6, 0x56, 0x3b, 0x08, 0xbe,
    0xef, 0xc3, 0x8c, 0x42, 0x42, 0x97, 0x05, 0xab,
    0x65, 0x51, 0x92, 0x2b, 0x56, 0xe1, 0x40, 0x7e,
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LicenseInfo {
    pub v: u32,
    pub id: String,
    pub name: String,
    pub tier: String,
    pub issued: String,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LicenseError {
    #[error("Format kunci tidak valid (harus diawali TRSA1-).")]
    BadFormat,
    #[error("Payload kunci tidak dapat dibaca.")]
    BadPayload,
    #[error("Tanda tangan kunci rusak atau panjangnya salah.")]
    BadSignatureBytes,
    #[error("Tanda tangan lisensi tidak sah.")]
    InvalidSignature,
    #[error("Versi format lisensi tidak didukung ({0}).")]
    UnsupportedVersion(u32),
    #[error("Public key tidak valid.")]
    BadPublicKey,
}

pub fn generate_keypair() -> Result<([u8; 32], [u8; 32]), String> {
    let mut secret = [0u8; 32];
    getrandom::fill(&mut secret).map_err(|e| format!("gagal membaca entropi OS: {e}"))?;
    let signing = SigningKey::from_bytes(&secret);
    let pubkey = signing.verifying_key().to_bytes();
    Ok((secret, pubkey))
}

pub fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        use std::fmt::Write;
        let _ = write!(&mut s, "{b:02x}");
    }
    s
}

pub fn from_hex_32(hex_str: &str) -> Result<[u8; 32], String> {
    let h = hex_str.trim();
    if h.len() != 64 {
        return Err(format!("harus tepat 64 karakter hex (ditemukan {})", h.len()));
    }
    let mut out = [0u8; 32];
    for i in 0..32 {
        out[i] = u8::from_str_radix(&h[i * 2..i * 2 + 2], 16)
            .map_err(|_| format!("karakter hex tidak valid di posisi {}", i * 2))?;
    }
    Ok(out)
}

/// Sign a `LicenseInfo` payload into a `TRSA1-...` offline license key (used by `trasea-keygen` and tests).
pub fn sign_license(info: &LicenseInfo, secret_key: &[u8; 32]) -> Result<String, LicenseError> {
    let json = serde_json::to_vec(info).map_err(|_| LicenseError::BadPayload)?;
    let payload_b64 = URL_SAFE_NO_PAD.encode(&json);
    let mut msg = Vec::with_capacity(6 + payload_b64.len());
    msg.extend_from_slice(b"TRSA1.");
    msg.extend_from_slice(payload_b64.as_bytes());
    let signing = SigningKey::from_bytes(secret_key);
    let sig = signing.sign(&msg);
    let sig_b64 = URL_SAFE_NO_PAD.encode(sig.to_bytes());
    Ok(format!("TRSA1-{payload_b64}-{sig_b64}"))
}

/// Verify an offline license key against an explicit 32-byte Ed25519 public key.
pub fn verify_license(key: &str, pubkey: &[u8; 32]) -> Result<LicenseInfo, LicenseError> {
    let trimmed = key.trim();
    let rest = trimmed.strip_prefix("TRSA1-").ok_or(LicenseError::BadFormat)?;
    // Ed25519 signature is always 64 bytes = 86 base64url (unpadded) chars; base64url itself may contain '-'.
    const SIG_B64_LEN: usize = 86;
    if rest.len() <= SIG_B64_LEN + 1 {
        return Err(LicenseError::BadFormat);
    }
    let sep = rest.len() - SIG_B64_LEN - 1;
    if !rest.is_char_boundary(sep) || rest.as_bytes()[sep] != b'-' {
        return Err(LicenseError::BadFormat);
    }
    let payload_b64 = &rest[..sep];
    let sig_b64 = &rest[sep + 1..];
    if payload_b64.is_empty() {
        return Err(LicenseError::BadFormat);
    }

    let sig_bytes = URL_SAFE_NO_PAD
        .decode(sig_b64.as_bytes())
        .map_err(|_| LicenseError::BadSignatureBytes)?;
    let sig_arr: [u8; 64] = sig_bytes
        .try_into()
        .map_err(|_| LicenseError::BadSignatureBytes)?;
    let sig = Signature::from_bytes(&sig_arr);

    let verifying = VerifyingKey::from_bytes(pubkey).map_err(|_| LicenseError::BadPublicKey)?;
    let mut msg = Vec::with_capacity(6 + payload_b64.len());
    msg.extend_from_slice(b"TRSA1.");
    msg.extend_from_slice(payload_b64.as_bytes());
    verifying.verify(&msg, &sig).map_err(|_| LicenseError::InvalidSignature)?;

    let payload_bytes = URL_SAFE_NO_PAD
        .decode(payload_b64.as_bytes())
        .map_err(|_| LicenseError::BadPayload)?;
    let info: LicenseInfo = serde_json::from_slice(&payload_bytes).map_err(|_| LicenseError::BadPayload)?;
    if info.v != 1 {
        return Err(LicenseError::UnsupportedVersion(info.v));
    }
    Ok(info)
}

/// Verify a license key against the embedded production/dev public key.
pub fn verify_embedded(key: &str) -> Result<LicenseInfo, LicenseError> {
    verify_license(key, &EMBEDDED_PUBLIC_KEY)
}
