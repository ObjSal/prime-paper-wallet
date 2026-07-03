//! Mainnet bech32/bech32m address encoding (BIP173 / BIP350).

use bech32::{hrp, segwit};

/// P2WPKH: witness v0 over HASH160(compressed pubkey), bech32, hrp "bc".
pub fn segwit_address(pubkey_hash: &[u8; 20]) -> String {
    segwit::encode_v0(hrp::BC, pubkey_hash).expect("20-byte program is always valid")
}

/// P2TR: witness v1 over the tweaked output x coordinate, bech32m.
pub fn taproot_address(output_x: &[u8; 32]) -> String {
    segwit::encode_v1(hrp::BC, output_x).expect("32-byte program is always valid")
}
