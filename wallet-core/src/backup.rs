//! JSON serialization: the Airlock backup file (web-app-schema compatible —
//! recover.html's import reads `backup_private_key_wif`, `internal_pubkey_hex`
//! and `network` from it) and the private-key-free on-device metadata file.

use serde::{Deserialize, Serialize};

use crate::Wallet;

/// Full backup written to Airlock alongside the bill PNG. Contains spendable
/// key material — the giver is expected to move it off-device with the bill.
#[derive(Serialize)]
pub struct AirlockBackup<'a> {
    #[serde(rename = "type")]
    pub type_: &'static str,
    pub network: &'static str,
    pub address: &'a str,
    pub private_key_wif: &'a str,
    pub private_key_hex: &'a str,
    pub internal_pubkey_hex: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_pubkey_hex: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_parity: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tweaked_private_key_hex: Option<&'a str>,
    pub bill_wif: &'a str,
    pub has_backup_key: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup_private_key_wif: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup_pubkey_hex: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script_tree_hash: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup_key_index: Option<u32>,
    pub created_at: &'a str,
}

/// On-device record kept on Location::User — NO private keys. The internal
/// pubkey is the one piece of a backup gift that cannot be re-derived from
/// the seed; together with the seed-derived backup WIF it is exactly what
/// recover.html needs.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GiftRecord {
    #[serde(rename = "type")]
    pub type_: String,
    pub network: String,
    pub address: String,
    pub internal_pubkey_hex: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub output_pubkey_hex: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub script_tree_hash: Option<String>,
    pub has_backup_key: bool,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub backup_pubkey_hex: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub backup_key_index: Option<u32>,
    pub created_at: String,
    /// Where the bill was exported, e.g. "Airlock: /gifts/bill.png" (display
    /// string; set by the app at save time).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub bill_path: Option<String>,
}

pub fn airlock_json(wallet: &Wallet, created_at: &str) -> String {
    let backup = wallet.backup.as_ref();
    let doc = AirlockBackup {
        type_: wallet.variant.type_str(),
        network: "mainnet",
        address: &wallet.address,
        private_key_wif: &wallet.private_key_wif,
        private_key_hex: &wallet.private_key_hex,
        internal_pubkey_hex: &wallet.internal_pubkey_hex,
        output_pubkey_hex: wallet.output_pubkey_hex.as_deref(),
        output_parity: wallet.output_parity,
        tweaked_private_key_hex: wallet.tweaked_private_key_hex.as_deref(),
        bill_wif: &wallet.bill_wif,
        has_backup_key: backup.is_some(),
        backup_private_key_wif: backup.map(|b| b.wif.as_str()),
        backup_pubkey_hex: backup.map(|b| b.pubkey_hex.as_str()),
        script_tree_hash: backup.map(|b| b.script_tree_hash_hex.as_str()),
        backup_key_index: backup.map(|b| b.index),
        created_at,
    };
    serde_json::to_string_pretty(&doc).expect("static schema always serializes")
}

pub fn to_json_pretty(record: &GiftRecord) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(record)
}

pub fn from_json(data: &[u8]) -> Result<GiftRecord, serde_json::Error> {
    serde_json::from_slice(data)
}

pub fn gift_record(wallet: &Wallet, created_at: &str) -> GiftRecord {
    let backup = wallet.backup.as_ref();
    GiftRecord {
        type_: wallet.variant.type_str().to_string(),
        network: "mainnet".to_string(),
        address: wallet.address.clone(),
        internal_pubkey_hex: wallet.internal_pubkey_hex.clone(),
        output_pubkey_hex: wallet.output_pubkey_hex.clone(),
        script_tree_hash: backup.map(|b| b.script_tree_hash_hex.clone()),
        has_backup_key: backup.is_some(),
        backup_pubkey_hex: backup.map(|b| b.pubkey_hex.clone()),
        backup_key_index: backup.map(|b| b.index),
        created_at: created_at.to_string(),
        bill_path: None,
    }
}
