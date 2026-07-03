//! Twin fixtures: every constant here was produced by the web app's Python
//! reference (`server/bitcoin_crypto.py`, itself cross-validated against the
//! JS) via scratchpad/gen_fixtures.py, plus published BIP vectors. Rust must
//! be byte-identical.

use wallet_core::{address, derive, keys, qr, taproot, Variant};

const K1: [u8; 32] = hex32("0000000000000000000000000000000000000000000000000000000000000001");
const K2: [u8; 32] = hex32("0000000000000000000000000000000000000000000000000000000000000002");
const KDEAD: [u8; 32] = hex32("deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef");
/// Fixed backup key used for all backup-variant fixtures.
const KBACKUP: [u8; 32] = hex32("1111111111111111111111111111111111111111111111111111111111111111");

const fn hex_nibble(b: u8) -> u8 {
    match b {
        b'0'..=b'9' => b - b'0',
        b'a'..=b'f' => b - b'a' + 10,
        _ => panic!("bad hex"),
    }
}

const fn hex32(s: &str) -> [u8; 32] {
    let bytes = s.as_bytes();
    assert!(bytes.len() == 64);
    let mut out = [0u8; 32];
    let mut i = 0;
    while i < 32 {
        out[i] = (hex_nibble(bytes[2 * i]) << 4) | hex_nibble(bytes[2 * i + 1]);
        i += 1;
    }
    out
}

// ---- RIPEMD-160 vectors (from the web repo's tests/test_bitcoin.py) ----

#[test]
fn hash160_building_blocks() {
    use ripemd::{Digest, Ripemd160};
    let vectors: &[(&[u8], &str)] = &[
        (b"", "9c1185a5c5e9fc54612808977ee8f548b2258d31"),
        (b"a", "0bdc9d2d256b3ee9daae347be6f4dc835a467ffe"),
        (b"abc", "8eb208f7e05d987a9b044a8e98c6b087f15a0bfc"),
        (b"message digest", "5d0689ef49d2fae572b881b123a85ffa21595f36"),
        (
            b"abcdefghijklmnopqrstuvwxyz",
            "f71c27109c692c1b56bbdceb5b9d2865b3708dbc",
        ),
    ];
    for (input, expect) in vectors {
        assert_eq!(hex::encode(Ripemd160::digest(input)), *expect);
    }
    // hash160(pubkey(k=1)) — BIP173's canonical program
    let pubkey = keys::compressed_pubkey(&K1).unwrap();
    assert_eq!(
        hex::encode(keys::hash160(&pubkey)),
        "751e76e8199196d454941c45d1b3a323f1433bd6"
    );
}

// ---- tagged hash ----

#[test]
fn tagged_hash_taptweak_empty() {
    assert_eq!(
        hex::encode(taproot::tagged_hash("TapTweak", b"")),
        "8aa4229474ab0100b2d6f0687f031d1fc9d8eef92a042ad97d279bff456b15e4"
    );
}

// ---- pubkeys / WIF ----

#[test]
fn k1_pubkeys_and_wif() {
    assert_eq!(
        hex::encode(keys::compressed_pubkey(&K1).unwrap()),
        "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
    );
    let (x, negated) = keys::xonly_pubkey(&K1).unwrap();
    assert_eq!(
        hex::encode(x),
        "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
    );
    assert!(!negated);
    assert_eq!(
        keys::wif_encode(&K1),
        "KwDiBf89QgGbjEhKnhXJuH7LrciVrZi3qYjgd9M7rFU73sVHnoWn"
    );
    assert_eq!(keys::wif_decode(&keys::wif_encode(&K1)).unwrap(), K1);
    assert_eq!(
        keys::wif_encode(&KBACKUP),
        "KwntMbt59tTsj8xqpqYqRRWufyjGunvhSyeMo3NTYpFYzZbXJ5Hp"
    );
}

#[test]
fn wif_round_trips() {
    for k in [K1, K2, KDEAD, KBACKUP] {
        assert_eq!(keys::wif_decode(&keys::wif_encode(&k)).unwrap(), k);
    }
    assert!(keys::wif_decode("KwDiBf89QgGbjEhKnhXJuH7LrciVrZi3qYjgd9M7rFU73sVHnoWm").is_err());
}

// ---- addresses: BIP173 / BIP350 published vectors ----

#[test]
fn bip173_bip350_addresses() {
    let program = {
        let pubkey = keys::compressed_pubkey(&K1).unwrap();
        keys::hash160(&pubkey)
    };
    assert_eq!(
        address::segwit_address(&program),
        "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4"
    );
    let (x, _) = keys::xonly_pubkey(&K1).unwrap();
    assert_eq!(
        address::taproot_address(&x),
        "bc1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vqzk5jj0"
    );
}

// ---- full twin fixtures: segwit / taproot / taproot+backup, all 3 keys ----

struct Fixture {
    privkey: [u8; 32],
    wif: &'static str,
    segwit_address: &'static str,
    xonly: &'static str,
    taproot_output_x: &'static str,
    taproot_output_parity: u8,
    taproot_address: &'static str,
    taproot_tweaked_seckey: &'static str,
    backup_output_x: &'static str,
    backup_output_parity: u8,
    backup_address: &'static str,
    backup_tweaked_seckey: &'static str,
    backup_tweaked_wif: &'static str,
}

const FIXTURES: &[Fixture] = &[
    Fixture {
        privkey: K1,
        wif: "KwDiBf89QgGbjEhKnhXJuH7LrciVrZi3qYjgd9M7rFU73sVHnoWn",
        segwit_address: "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4",
        xonly: "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
        taproot_output_x: "da4710964f7852695de2da025290e24af6d8c281de5a0b902b7135fd9fd74d21",
        taproot_output_parity: 1,
        taproot_address: "bc1pmfr3p9j00pfxjh0zmgp99y8zftmd3s5pmedqhyptwy6lm87hf5sspknck9",
        taproot_tweaked_seckey: "3cf5216d476a5e637bf0da674e50ddf55c403270dd36494dfcca438132fa30e8",
        backup_output_x: "70da4957e6e3d7ccf352595428ac210d024a8e0f2e0b610e68a582ae24570732",
        backup_output_parity: 0,
        backup_address: "bc1pwrdyj4lxu0tueu6jt92z3tppp5py4rs09c9kzrng5kp2ufzhqueqjus9yt",
        backup_tweaked_seckey: "8163db3770aac3e081346e81ba6983598546dac5c6845c1290a1e1d3fc1b23c2",
        backup_tweaked_wif: "L1ZEC7NqGTXmKHPPDNLX3nWAAdLdJ4tGreFKbvCaC6rfMFZ2hDbV",
    },
    // k2 and kdead values pinned from gen_fixtures.py output (fixtures.json).
    Fixture {
        privkey: K2,
        wif: "KwDiBf89QgGbjEhKnhXJuH7LrciVrZi3qYjgd9M7rFU74NMTptX4",
        segwit_address: "bc1qq6hag67dl53wl99vzg42z8eyzfz2xlkvxechjp",
        xonly: "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5",
        taproot_output_x: "cafd90c7026f0b6ab98df89490d02732881f2f4b5900856358dddff4679c2ffb",
        taproot_output_parity: 0,
        taproot_address: "bc1pet7ep3czdu9k4wvdlz2fp5p8x2yp7t6ttyqg2c6cmh0lgeuu9lasmp9hsg",
        taproot_tweaked_seckey: "04b26deba66b94db399bbf0c31029d168c5feb5664cd7a212248322171345ae0",
        backup_output_x: "ca4bb3b2c892beea93b0d5bf59d1965fd8000e28b9b29d14a41d3d2c9cd7c702",
        backup_output_parity: 1,
        backup_address: "bc1pef9m8vkgj2lw4yas6kl4n5vktlvqqr3ghxef699yr57je8xhcupqua392n",
        backup_tweaked_seckey: "6f42b297ddf4da0c8e4e58eb3fefce758a54f73a683e11610b70fbf45ac7e16f",
        backup_tweaked_wif: "KzwzBwYbUsSiUnviQ9vKjztvbp23a9BStdzvh4K6BUpgAPhwbJwd",
    },
    Fixture {
        privkey: KDEAD,
        wif: "L4gZxvfGxeHQYpUcvFwnuaXn8xaBKmvFTm1Z3advYg4xLJ7435BQ",
        segwit_address: "bc1qvmqas4maw7lg9clqu6kqu9zq9cluvlln5hw97q",
        xonly: "c6b754b20826eb925e052ee2c25285b162b51fdca732bcf67e39d647fb6830ae",
        taproot_output_x: "f7c752b8c6764495c1f6bbede0952f34db8fe1698e9f003bd749449100c4ffeb",
        taproot_output_parity: 1,
        taproot_address: "bc1p7lr49wxxwezfts0kh0k7p9f0xndclctf360sqw7hf9zfzqxyll4splzdgc",
        taproot_tweaked_seckey: "69289e7d8456db51bf20c189f8f8b29909d19744e7e664a76cd9511803ae70e2",
        backup_output_x: "8c6cd5b3553d878342415215fcce65e2e40ac497bb7704b11d14a69b9def8604",
        backup_output_parity: 1,
        backup_address: "bc1p33kdtv648krcxsjp2g2lenn9utjq43yhhdmsfvgazjnfh800sczqj3pdw3",
        backup_tweaked_seckey: "bc7a93924f389951ed69903c94475e74de0944bf3bec209442cf72f601060790",
        backup_tweaked_wif: "L3Y67sfrvEAL3UNdyrzAvq8GUSMSMFBPHF7NrK3p85s2LqvoaMZh",
    },
];

/// script_tree_hash for the fixed backup key — identical for every fixture.
const BACKUP_XONLY: &str = "4f355bdcb7cc0af728ef3cceb9615d90684bb5b2ca5f859ab0f0b704075871aa";
const BACKUP_STH: &str = "54461f083426f688bb72aed949de73395b4a89f2f05b438ec4401443002eeb70";

#[test]
fn backup_leaf_hash_pinned() {
    let (bx, _) = keys::xonly_pubkey(&KBACKUP).unwrap();
    assert_eq!(hex::encode(bx), BACKUP_XONLY);
    assert_eq!(hex::encode(taproot::backup_leaf_hash(&bx)), BACKUP_STH);
}

#[test]
fn twin_fixtures_all_variants() {
    for f in FIXTURES {
        let segwit = wallet_core::from_privkeys(Variant::Segwit, &f.privkey, None).unwrap();
        assert_eq!(segwit.private_key_wif, f.wif);
        assert_eq!(segwit.address, f.segwit_address);
        assert_eq!(segwit.bill_wif, f.wif);
        assert!(!segwit.is_tweaked);

        let tap = wallet_core::from_privkeys(Variant::Taproot, &f.privkey, None).unwrap();
        assert_eq!(tap.internal_pubkey_hex, f.xonly);
        assert_eq!(tap.output_pubkey_hex.as_deref(), Some(f.taproot_output_x));
        assert_eq!(tap.output_parity, Some(f.taproot_output_parity));
        assert_eq!(tap.address, f.taproot_address);
        assert_eq!(
            tap.tweaked_private_key_hex.as_deref(),
            Some(f.taproot_tweaked_seckey)
        );
        // no-backup bill prints the UNTWEAKED wif
        assert_eq!(tap.bill_wif, f.wif);
        assert!(!tap.is_tweaked);

        let bk =
            wallet_core::from_privkeys(Variant::TaprootBackup, &f.privkey, Some((KBACKUP, 7)))
                .unwrap();
        assert_eq!(bk.output_pubkey_hex.as_deref(), Some(f.backup_output_x));
        assert_eq!(bk.output_parity, Some(f.backup_output_parity));
        assert_eq!(bk.address, f.backup_address);
        assert_eq!(
            bk.tweaked_private_key_hex.as_deref(),
            Some(f.backup_tweaked_seckey)
        );
        // backup bill prints the TWEAKED wif
        assert_eq!(bk.bill_wif, f.backup_tweaked_wif);
        assert!(bk.is_tweaked);
        let backup = bk.backup.as_ref().unwrap();
        assert_eq!(backup.index, 7);
        assert_eq!(backup.pubkey_hex, BACKUP_XONLY);
        assert_eq!(backup.script_tree_hash_hex, BACKUP_STH);
        assert_eq!(
            backup.wif,
            "KwntMbt59tTsj8xqpqYqRRWufyjGunvhSyeMo3NTYpFYzZbXJ5Hp"
        );
    }
}

// ---- backup derivation ----

#[test]
fn backup_derivation_stable_and_distinct() {
    let app_seed = [0x42u8; 32];
    let k0 = derive::derive_backup_key(&app_seed, 0);
    let k0_again = derive::derive_backup_key(&app_seed, 0);
    let k1 = derive::derive_backup_key(&app_seed, 1);
    assert_eq!(k0, k0_again);
    assert_ne!(k0, k1);
    // must be valid gift-wallet backup keys end to end
    wallet_core::from_privkeys(Variant::TaprootBackup, &K1, Some((k0, 0))).unwrap();
}

// ---- keygen smoke ----

#[test]
fn keygen_produces_valid_unique_keys() {
    let mut seen = std::collections::HashSet::new();
    for _ in 0..100 {
        let k = keys::generate_private_key().unwrap();
        assert!(seen.insert(k), "duplicate key from TRNG path");
        keys::compressed_pubkey(&k).unwrap();
    }
}

// ---- QR payloads ----

#[test]
fn sweep_url_format() {
    assert_eq!(
        qr::sweep_url("L1ZEC7NqGTXmKHPPDNLX3nWAAdLdJ4tGreFKbvCaC6rfMFZ2hDbV", Variant::TaprootBackup),
        "https://ObjSal.github.io/bitcoin-gift-paper-wallet/sweep.html?wif=L1ZEC7NqGTXmKHPPDNLX3nWAAdLdJ4tGreFKbvCaC6rfMFZ2hDbV&network=mainnet&type=taproot_tweaked"
    );
    assert_eq!(qr::sweep_url("w", Variant::Segwit).contains("type=segwit"), true);
    assert_eq!(qr::sweep_url("w", Variant::Taproot).contains("type=taproot&"), false); // taproot is last param
    assert!(qr::sweep_url("w", Variant::Taproot).ends_with("type=taproot"));
    let m = qr::qr_matrix(&qr::address_payload("bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4")).unwrap();
    assert!(m.width > 0 && m.modules.len() == m.width * m.width);
}
