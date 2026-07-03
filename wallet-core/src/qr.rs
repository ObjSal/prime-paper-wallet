//! QR payloads and matrices for the bill (and the on-screen preview reuses
//! the same payload strings through the SDK's own renderer).

use qrcode::QrCode;

use crate::{Error, Variant};

/// Base of the hosted sweep page the private-key QR points at (same as the
/// web app's bills — the recipient scans it and lands on the guided sweep
/// flow with everything pre-filled).
pub const SWEEP_BASE: &str = "https://ObjSal.github.io/bitcoin-gift-paper-wallet/sweep.html";

/// Private-key QR payload. Mixed case → byte-mode QR, like the web bills.
pub fn sweep_url(bill_wif: &str, variant: Variant) -> String {
    format!(
        "{SWEEP_BASE}?wif={bill_wif}&network=mainnet&type={}",
        variant.type_str()
    )
}

/// Address QR payload: uppercased, so the all-alphanumeric bech32 string
/// encodes in the denser alphanumeric mode (exactly what the web app does).
pub fn address_payload(address: &str) -> String {
    address.to_uppercase()
}

/// A rendered QR matrix; `modules[y * width + x]`, true = dark.
pub struct QrMatrix {
    pub width: usize,
    pub modules: Vec<bool>,
}

/// Encode with the qrcode crate's defaults: error-correction level M and
/// automatic (per-payload) mode selection — matching the web generator.
pub fn qr_matrix(data: &str) -> Result<QrMatrix, Error> {
    let code = QrCode::new(data.as_bytes()).map_err(|_| Error::QrTooLong)?;
    let width = code.width();
    let modules = code
        .to_colors()
        .into_iter()
        .map(|c| c == qrcode::Color::Dark)
        .collect();
    Ok(QrMatrix { width, modules })
}
