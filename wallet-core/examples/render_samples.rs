//! Render one sample bill per variant (fixed keys) for visual inspection:
//! `cargo run -p wallet-core --example render_samples -- <out-dir>`

use wallet_core::{bill, Variant};

fn main() {
    let out_dir = std::env::args().nth(1).unwrap_or_else(|| ".".to_string());
    let mut k1 = [0u8; 32];
    k1[31] = 1;
    let kb = [0x11u8; 32];
    for (name, variant, backup) in [
        ("sample_segwit.png", Variant::Segwit, None),
        ("sample_taproot.png", Variant::Taproot, None),
        ("sample_taproot_tweaked.png", Variant::TaprootBackup, Some((kb, 0))),
    ] {
        let wallet = wallet_core::from_privkeys(variant, &k1, backup).unwrap();
        let png = bill::compose_bill(&wallet, "2026", "2026-07-02 12:00:00 UTC").unwrap();
        let path = format!("{out_dir}/{name}");
        std::fs::write(&path, png).unwrap();
        println!("{path}  addr={}", wallet.address);
    }
}
