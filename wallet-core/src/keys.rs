//! Key generation, pubkey derivation, HASH160 and WIF encoding.
//!
//! Byte-for-byte twin of the reference implementation in the web app
//! (`js/bitcoin_crypto.js` / `server/bitcoin_crypto.py`); mainnet-only,
//! compressed-only, exactly what the bills print.

use elliptic_curve::point::AffineCoordinates;
use elliptic_curve::sec1::ToEncodedPoint;
use elliptic_curve::PrimeField;
use k256::elliptic_curve;
use k256::{ProjectivePoint, Scalar};
use ripemd::Ripemd160;
use sha2::{Digest, Sha256};

use crate::Error;

/// Parse 32 bytes as a scalar, rejecting 0 and values >= the curve order.
pub(crate) fn scalar_from_bytes(bytes: &[u8; 32]) -> Option<Scalar> {
    let ct = Scalar::from_repr((*bytes).into());
    if bool::from(ct.is_some()) {
        let s = ct.unwrap();
        if bool::from(s.is_zero()) {
            None
        } else {
            Some(s)
        }
    } else {
        None
    }
}

/// Generate a private key from OS/TRNG entropy with rejection sampling
/// until 0 < k < N (same loop as the reference `generatePrivateKey`).
pub fn generate_private_key() -> Result<[u8; 32], Error> {
    for _ in 0..128 {
        let mut buf = [0u8; 32];
        getrandom::getrandom(&mut buf).map_err(|_| Error::Entropy)?;
        if scalar_from_bytes(&buf).is_some() {
            return Ok(buf);
        }
    }
    // 128 consecutive out-of-range draws means the entropy source is broken.
    Err(Error::Entropy)
}

/// 33-byte compressed SEC1 public key (0x02/0x03 prefix).
pub fn compressed_pubkey(privkey: &[u8; 32]) -> Result<[u8; 33], Error> {
    let k = scalar_from_bytes(privkey).ok_or(Error::InvalidPrivateKey)?;
    let point = (ProjectivePoint::GENERATOR * k).to_affine();
    let encoded = point.to_encoded_point(true);
    let mut out = [0u8; 33];
    out.copy_from_slice(encoded.as_bytes());
    Ok(out)
}

/// X-only public key plus `negated` flag (true when the point's y is odd,
/// i.e. the effective BIP340 secret key is N - k).
pub fn xonly_pubkey(privkey: &[u8; 32]) -> Result<([u8; 32], bool), Error> {
    let k = scalar_from_bytes(privkey).ok_or(Error::InvalidPrivateKey)?;
    let point = (ProjectivePoint::GENERATOR * k).to_affine();
    let x: [u8; 32] = point.x().into();
    Ok((x, bool::from(point.y_is_odd())))
}

/// RIPEMD160(SHA256(data)).
pub fn hash160(data: &[u8]) -> [u8; 20] {
    let sha = Sha256::digest(data);
    let mut out = [0u8; 20];
    out.copy_from_slice(&Ripemd160::digest(sha));
    out
}

pub(crate) fn double_sha256(data: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(&Sha256::digest(Sha256::digest(data)));
    out
}

/// Mainnet compressed-key WIF: base58(0x80 || key || 0x01 || checksum4).
pub fn wif_encode(privkey: &[u8; 32]) -> String {
    let mut payload = Vec::with_capacity(38);
    payload.push(0x80);
    payload.extend_from_slice(privkey);
    payload.push(0x01);
    let checksum = double_sha256(&payload);
    payload.extend_from_slice(&checksum[..4]);
    bs58::encode(payload).into_string()
}

/// Decode a mainnet compressed-key WIF (used by tests and sanity checks).
pub fn wif_decode(wif: &str) -> Result<[u8; 32], Error> {
    let raw = bs58::decode(wif).into_vec().map_err(|_| Error::InvalidWif)?;
    if raw.len() != 38 {
        return Err(Error::InvalidWif);
    }
    let (payload, checksum) = raw.split_at(34);
    if double_sha256(payload)[..4] != *checksum {
        return Err(Error::InvalidWif);
    }
    if payload[0] != 0x80 || payload[33] != 0x01 {
        return Err(Error::InvalidWif);
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&payload[1..33]);
    Ok(key)
}
