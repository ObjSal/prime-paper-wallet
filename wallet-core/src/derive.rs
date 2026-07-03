//! Deterministic backup-key derivation from the app seed.
//!
//! The giver's taproot backup key is derived from GetAppSeed
//! (HMAC-SHA256(app-id, master_seed), PIN-gated on hardware) so that a
//! seed-phrase restore re-derives every backup key by index — losing the
//! exported backup JSON does not burn the recovery path.
//!
//! CONSENSUS-CRITICAL FOR RE-DERIVATION: the salt and info strings below and
//! the attempt-loop layout are baked into every bill already gifted. NEVER
//! change them (same rule as prime-pgp-keychain's DERIVED_KEY_CREATED_AT).

use hkdf::Hkdf;
use sha2::Sha256;

use crate::keys::scalar_from_bytes;

const SALT: &[u8] = b"prime-paper-wallet/derive/v1";

/// Derive the backup private key for `index`. Expands
/// HKDF-SHA256(salt, app_seed, "backup-key/" || index_le || attempt_le)
/// and bumps `attempt` until the 32 bytes are a valid scalar (first try in
/// all but ~1 in 2^128 cases).
pub fn derive_backup_key(app_seed: &[u8; 32], index: u32) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(Some(SALT), app_seed);
    let mut attempt: u32 = 0;
    loop {
        let mut info = Vec::with_capacity(11 + 8);
        info.extend_from_slice(b"backup-key/");
        info.extend_from_slice(&index.to_le_bytes());
        info.extend_from_slice(&attempt.to_le_bytes());
        let mut okm = [0u8; 32];
        hk.expand(&info, &mut okm).expect("32 bytes is a valid HKDF length");
        if scalar_from_bytes(&okm).is_some() {
            return okm;
        }
        attempt += 1;
    }
}
