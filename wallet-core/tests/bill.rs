//! Bill composition tests: deterministic compose for pinned wallets, PNG
//! sanity, and rqrr round-trip of both QR codes back to the exact payloads.

use wallet_core::{bill, qr, Variant};

const K1: [u8; 32] = {
    let mut k = [0u8; 32];
    k[31] = 1;
    k
};
const KBACKUP: [u8; 32] = [0x11; 32];

fn decode_qrs(png: &[u8]) -> Vec<String> {
    let img = image::load_from_memory(png).unwrap().to_luma8();
    let mut prepared = rqrr::PreparedImage::prepare(img);
    prepared
        .detect_grids()
        .into_iter()
        .filter_map(|g| g.decode().ok().map(|(_, content)| content))
        .collect()
}

fn check_variant(variant: Variant, backup: Option<([u8; 32], u32)>) {
    let wallet = wallet_core::from_privkeys(variant, &K1, backup).unwrap();
    let png = bill::compose_bill(&wallet, "2026", "2026-07-02 12:00:00 UTC").unwrap();

    let decoded = image::load_from_memory(&png).unwrap();
    assert_eq!(decoded.width(), bill::BILL_WIDTH);
    assert_eq!(decoded.height(), bill::BILL_HEIGHT);

    let contents = decode_qrs(&png);
    assert_eq!(contents.len(), 2, "{variant:?}: expected both QRs to scan");
    let addr_payload = qr::address_payload(&wallet.address);
    let sweep = qr::sweep_url(&wallet.bill_wif, variant);
    assert!(
        contents.contains(&addr_payload),
        "{variant:?}: address QR must decode to {addr_payload}, got {contents:?}"
    );
    assert!(
        contents.contains(&sweep),
        "{variant:?}: privkey QR must decode to the sweep URL, got {contents:?}"
    );
}

#[test]
fn bill_segwit() {
    check_variant(Variant::Segwit, None);
}

#[test]
fn bill_taproot() {
    check_variant(Variant::Taproot, None);
}

#[test]
fn bill_taproot_backup() {
    check_variant(Variant::TaprootBackup, Some((KBACKUP, 0)));
}

#[test]
fn taproot_address_renders_readably() {
    let wallet = wallet_core::from_privkeys(Variant::Taproot, &K1, None).unwrap();
    // 62-char bc1p address must fit the 800px band at a readable size.
    let (size, width) =
        wallet_core::text::fit_to_box(wallet_core::text::BillFont::Condensed, &wallet.address, 800.0, 57.0, 36);
    assert!(size >= 14.0, "taproot address font too small: {size}px");
    assert!(width <= 800.0);
}

#[test]
fn format_utc_known_values() {
    assert_eq!(
        bill::format_utc(0),
        ("1970".to_string(), "1970-01-01 00:00:00 UTC".to_string())
    );
    // 2026-07-02 12:00:00 UTC
    assert_eq!(
        bill::format_utc(1_782_993_600),
        ("2026".to_string(), "2026-07-02 12:00:00 UTC".to_string())
    );
}
