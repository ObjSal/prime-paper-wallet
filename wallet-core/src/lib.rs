//! wallet-core — offline bitcoin gift paper-wallet library.
//!
//! Byte-for-byte port of the generation path of the bitcoin-gift-wallet web
//! app (`js/bitcoin_crypto.js`; Python twin `server/bitcoin_crypto.py`),
//! pinned by twin fixtures in `tests/vectors.rs`. Mainnet only.

pub mod address;
pub mod backup;
pub mod bill;
pub mod derive;
pub mod keys;
pub mod qr;
pub mod taproot;
pub mod template;
pub mod text;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    /// P2WPKH, bill prints the plain WIF.
    Segwit,
    /// P2TR without script tree, bill prints the UNTWEAKED internal WIF
    /// (BIP86-compatible: importable into standard taproot wallets).
    Taproot,
    /// P2TR committing to a `<backup_pk> OP_CHECKSIG` leaf; bill prints the
    /// TWEAKED private key ("(tweaked)" on the bill), because standard
    /// wallets cannot derive the script-tree tweak from a bare WIF.
    TaprootBackup,
}

impl Variant {
    /// The `type` parameter of the sweep URL / web app.
    pub fn type_str(&self) -> &'static str {
        match self {
            Variant::Segwit => "segwit",
            Variant::Taproot => "taproot",
            Variant::TaprootBackup => "taproot_tweaked",
        }
    }
}

/// Giver-side backup spend path of a [`Variant::TaprootBackup`] wallet.
#[derive(Debug, Clone)]
pub struct BackupInfo {
    /// Derivation index under the app seed (see [`derive::derive_backup_key`]).
    pub index: u32,
    pub wif: String,
    pub pubkey_hex: String,
    pub script_tree_hash_hex: String,
}

/// One generated gift wallet: everything the bill, the backup JSON and the
/// on-device metadata need.
#[derive(Debug, Clone)]
pub struct Wallet {
    pub variant: Variant,
    pub address: String,
    /// WIF of the gift (internal) private key.
    pub private_key_wif: String,
    pub private_key_hex: String,
    /// Compressed pubkey (segwit) or x-only internal pubkey (taproot).
    pub internal_pubkey_hex: String,
    /// Tweaked output x-only key (taproot variants only).
    pub output_pubkey_hex: Option<String>,
    pub output_parity: Option<u8>,
    /// Tweaked private key hex (taproot variants only).
    pub tweaked_private_key_hex: Option<String>,
    /// What the bill's private-key strip and sweep QR carry.
    pub bill_wif: String,
    /// True when `bill_wif` is the tweaked key (TaprootBackup).
    pub is_tweaked: bool,
    pub backup: Option<BackupInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Entropy,
    InvalidPrivateKey,
    InvalidPublicKey,
    InvalidWif,
    TweakOutOfRange,
    PointAtInfinity,
    QrTooLong,
    Render,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            Error::Entropy => "entropy source failure",
            Error::InvalidPrivateKey => "invalid private key",
            Error::InvalidPublicKey => "invalid public key",
            Error::InvalidWif => "invalid WIF",
            Error::TweakOutOfRange => "taproot tweak out of range",
            Error::PointAtInfinity => "resulting point is at infinity",
            Error::QrTooLong => "QR payload too long",
            Error::Render => "bill rendering failed",
        };
        f.write_str(msg)
    }
}

impl std::error::Error for Error {}

/// Generate a wallet from a fresh TRNG gift key. For [`Variant::TaprootBackup`]
/// pass the app seed and the backup index to derive the giver's key.
pub fn generate(variant: Variant, backup: Option<(&[u8; 32], u32)>) -> Result<Wallet, Error> {
    let gift_privkey = keys::generate_private_key()?;
    let backup_privkey = match (variant, backup) {
        (Variant::TaprootBackup, Some((app_seed, index))) => {
            Some((derive::derive_backup_key(app_seed, index), index))
        }
        (Variant::TaprootBackup, None) => return Err(Error::InvalidPrivateKey),
        _ => None,
    };
    from_privkeys(variant, &gift_privkey, backup_privkey)
}

/// Deterministic core of [`generate`], also the entry point for the twin
/// fixtures: build the full wallet from explicit private keys.
pub fn from_privkeys(
    variant: Variant,
    gift_privkey: &[u8; 32],
    backup: Option<([u8; 32], u32)>,
) -> Result<Wallet, Error> {
    let private_key_wif = keys::wif_encode(gift_privkey);
    let private_key_hex = hex_str(gift_privkey);

    match variant {
        Variant::Segwit => {
            let pubkey = keys::compressed_pubkey(gift_privkey)?;
            let address = address::segwit_address(&keys::hash160(&pubkey));
            Ok(Wallet {
                variant,
                address,
                bill_wif: private_key_wif.clone(),
                private_key_wif,
                private_key_hex,
                internal_pubkey_hex: hex_str(&pubkey),
                output_pubkey_hex: None,
                output_parity: None,
                tweaked_private_key_hex: None,
                is_tweaked: false,
                backup: None,
            })
        }
        Variant::Taproot | Variant::TaprootBackup => {
            let (internal_x, _negated) = keys::xonly_pubkey(gift_privkey)?;
            let (script_tree_hash, backup_info) = match (variant, backup) {
                (Variant::TaprootBackup, Some((backup_privkey, index))) => {
                    let (backup_x, _) = keys::xonly_pubkey(&backup_privkey)?;
                    let sth = taproot::backup_leaf_hash(&backup_x);
                    let info = BackupInfo {
                        index,
                        wif: keys::wif_encode(&backup_privkey),
                        pubkey_hex: hex_str(&backup_x),
                        script_tree_hash_hex: hex_str(&sth),
                    };
                    (Some(sth), Some(info))
                }
                (Variant::TaprootBackup, None) => return Err(Error::InvalidPrivateKey),
                _ => (None, None),
            };
            let sth_ref = script_tree_hash.as_ref();
            let (output_x, parity) = taproot::taproot_tweak_pubkey(&internal_x, sth_ref)?;
            let tweaked_privkey = taproot::taproot_tweak_seckey(gift_privkey, sth_ref)?;
            let address = address::taproot_address(&output_x);
            let is_tweaked = variant == Variant::TaprootBackup;
            let bill_wif = if is_tweaked {
                keys::wif_encode(&tweaked_privkey)
            } else {
                private_key_wif.clone()
            };
            Ok(Wallet {
                variant,
                address,
                bill_wif,
                private_key_wif,
                private_key_hex,
                internal_pubkey_hex: hex_str(&internal_x),
                output_pubkey_hex: Some(hex_str(&output_x)),
                output_parity: Some(parity),
                tweaked_private_key_hex: Some(hex_str(&tweaked_privkey)),
                is_tweaked,
                backup: backup_info,
            })
        }
    }
}

pub(crate) fn hex_str(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}
