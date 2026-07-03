mod theme;

use std::cell::RefCell;
use std::io::Read;
use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use slint_keyos_platform::app_ui;
use slint_keyos_platform::fs::{self, Location, OpenFlags};
use slint_keyos_platform::qrcode;
use slint_keyos_platform::slint::{Color, ComponentHandle, Image, ModelRc, Timer, VecModel};
use wallet_core::backup::GiftRecord;
use wallet_core::{bill, derive, keys, qr, Variant};

app_ui!("prime-paper-wallet");
security::use_api!();

/// Gift metadata (no private keys) on Internal (User) storage; exported
/// bills + full backup JSONs go to the same-named directory on Airlock.
const GIFTS_DIR: &str = "/paper-wallets";

type Fs = fs::FileSystem<fs_permissions::FileSystemPermissions>;

/// Mutable app state shared across the UI callbacks.
struct State {
    /// Freshly generated, not-yet-discarded wallet shown on the preview
    /// screen, with its created-at timestamp pair (year, full timestamp).
    current: Option<(wallet_core::Wallet, String, String)>,
    /// Record backing the open detail screen.
    open_gift: Option<GiftRecord>,
}

fn app_main(cx: AppContext, ui: AppWindow) {
    log_server::init_wait(env!("CARGO_CRATE_NAME")).unwrap();
    log::set_max_level(log::LevelFilter::Info);

    theme::init(&ui);

    let fs = cx.fs.clone();
    let ui_weak = ui.as_weak();
    let state = Rc::new(RefCell::new(State { current: None, open_gift: None }));

    if let Err(e) = fs.create_dir(GIFTS_DIR, Location::User) {
        if !matches!(e, fs::Error::FileAlreadyExists) {
            log::warn!("could not create {GIFTS_DIR}: {e:?}");
        }
    }

    // Re-scan /paper-wallets metadata and push rows into the Gifts global.
    let refresh_gifts: Rc<dyn Fn()> = {
        let fs = fs.clone();
        let ui_weak = ui_weak.clone();
        Rc::new(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut records = load_records(&fs);
            records.sort_by(|a, b| b.1.created_at.cmp(&a.1.created_at));
            let rows: Vec<GiftRow> = records
                .iter()
                .map(|(filename, r)| GiftRow {
                    filename: filename.as_str().into(),
                    address: r.address.as_str().into(),
                    subtitle: format!(
                        "{} · {}",
                        r.type_,
                        r.created_at.split(' ').next().unwrap_or("")
                    )
                    .into(),
                    has_backup: r.has_backup_key,
                })
                .collect();
            log::info!("cb: refresh-gifts n={}", rows.len());
            let gifts = ui.global::<Gifts>();
            gifts.set_status(if rows.is_empty() {
                "No saved gifts yet — generate one and save its bill".into()
            } else {
                "".into()
            });
            gifts.set_gifts(ModelRc::new(VecModel::from(rows)));
            ui.global::<Ui>().set_gift_count(records.len() as i32);
        })
    };

    refresh_gifts();

    // Generate a wallet of the selected variant (variant 2 needs the PIN-gated
    // app seed for the deterministic backup key).
    {
        let fs = fs.clone();
        let ui_weak = ui_weak.clone();
        let state = state.clone();
        ui.global::<Callbacks>().on_generate(move |variant_idx| {
            let Some(u) = ui_weak.upgrade() else { return };
            u.global::<Ui>().set_error("".into());
            u.global::<Ui>().set_busy(true);

            let fs = fs.clone();
            let ui_weak = ui_weak.clone();
            let state = state.clone();
            // Let the busy overlay paint one frame before the blocking work.
            Timer::single_shot(Duration::from_millis(150), move || {
                let Some(ui) = ui_weak.upgrade() else { return };
                let variant = match variant_idx {
                    0 => Variant::Segwit,
                    1 => Variant::Taproot,
                    _ => Variant::TaprootBackup,
                };
                let result = match variant {
                    Variant::TaprootBackup => Security::default()
                        .app_seed()
                        .map_err(|_| "Device locked or seed unavailable".to_string())
                        .and_then(|app_seed| {
                            let index = next_backup_index(&fs);
                            wallet_core::generate(variant, Some((&app_seed, index)))
                                .map_err(|e| e.to_string())
                        }),
                    _ => wallet_core::generate(variant, None).map_err(|e| e.to_string()),
                };
                ui.global::<Ui>().set_busy(false);
                match result {
                    Ok(wallet) => {
                        let (year, ts) = bill::format_utc(now_epoch());
                        let backup_index =
                            wallet.backup.as_ref().map(|b| b.index as i32).unwrap_or(-1);
                        log::info!(
                            "cb: generate variant={} ok addr={} tweaked={} backup-index={}",
                            variant_name_log(variant),
                            wallet.address,
                            wallet.is_tweaked,
                            wallet
                                .backup
                                .as_ref()
                                .map(|b| b.index.to_string())
                                .unwrap_or_else(|| "none".to_string()),
                        );
                        let p = ui.global::<Preview>();
                        p.set_variant_name(variant_name(variant).into());
                        p.set_address(wallet.address.as_str().into());
                        p.set_address_qr(qr_image(&qr::address_payload(&wallet.address)));
                        p.set_sweep_qr(qr_image(&qr::sweep_url(&wallet.bill_wif, variant)));
                        p.set_bill_wif(wallet.bill_wif.as_str().into());
                        p.set_is_tweaked(wallet.is_tweaked);
                        p.set_has_backup(wallet.backup.is_some());
                        p.set_backup_index(backup_index);
                        p.set_saved(false);
                        p.set_saved_path("".into());
                        state.borrow_mut().current = Some((wallet, year, ts));
                        ui.global::<Ui>().set_screen(1);
                    }
                    Err(e) => {
                        log::info!(
                            "cb: generate variant={} err={e}",
                            variant_name_log(variant)
                        );
                        ui.global::<Ui>().set_error(e.into());
                    }
                }
            });
        });
    }

    // Compose the bill PNG + full backup JSON to Airlock, plus the
    // private-key-free metadata record to Internal storage.
    {
        let fs = fs.clone();
        let ui_weak = ui_weak.clone();
        let state = state.clone();
        let refresh_gifts = refresh_gifts.clone();
        ui.global::<Callbacks>().on_save_bill(move || {
            let Some(u) = ui_weak.upgrade() else { return };
            u.global::<Ui>().set_error("".into());
            u.global::<Ui>().set_busy(true);

            let fs = fs.clone();
            let ui_weak = ui_weak.clone();
            let state = state.clone();
            let refresh_gifts = refresh_gifts.clone();
            Timer::single_shot(Duration::from_millis(150), move || {
                let Some(ui) = ui_weak.upgrade() else { return };
                let result = state
                    .borrow()
                    .current
                    .as_ref()
                    .ok_or_else(|| "Nothing to save".to_string())
                    .and_then(|(wallet, year, ts)| save_gift(&fs, wallet, year, ts));
                ui.global::<Ui>().set_busy(false);
                match result {
                    Ok((png_path, json_path)) => {
                        log::info!("cb: save-bill ok png={png_path} json={json_path}");
                        let p = ui.global::<Preview>();
                        p.set_saved(true);
                        p.set_saved_path(png_path.as_str().into());
                        refresh_gifts();
                    }
                    Err(e) => {
                        log::info!("cb: save-bill err={e}");
                        ui.global::<Ui>().set_error(e.into());
                    }
                }
            });
        });
    }

    // Drop the previewed wallet (also the "Done" action after saving).
    {
        let ui_weak = ui_weak.clone();
        let state = state.clone();
        ui.global::<Callbacks>().on_discard(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            state.borrow_mut().current = None;
            ui.global::<Ui>().set_error("".into());
            ui.global::<Ui>().set_screen(0);
        });
    }

    {
        let refresh_gifts = refresh_gifts.clone();
        ui.global::<Callbacks>().on_refresh_gifts(move || refresh_gifts());
    }

    // Open a saved gift's detail view from its metadata record.
    {
        let fs = fs.clone();
        let ui_weak = ui_weak.clone();
        let state = state.clone();
        ui.global::<Callbacks>().on_open_gift(move |filename| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let filename = filename.to_string();
            match load_record(&fs, &filename) {
                Ok(record) => {
                    log::info!(
                        "cb: open-gift {filename} addr={} backup={}",
                        record.address,
                        record.has_backup_key
                    );
                    let d = ui.global::<Detail>();
                    d.set_filename(filename.as_str().into());
                    d.set_address(record.address.as_str().into());
                    d.set_type_name(record.type_.as_str().into());
                    d.set_created_at(record.created_at.as_str().into());
                    d.set_internal_pubkey(record.internal_pubkey_hex.as_str().into());
                    d.set_has_backup(record.has_backup_key);
                    d.set_backup_index(record.backup_key_index.map(|i| i as i32).unwrap_or(-1));
                    d.set_show_backup(false);
                    d.set_backup_wif("".into());
                    ui.global::<Ui>().set_error("".into());
                    state.borrow_mut().open_gift = Some(record);
                    ui.global::<Ui>().set_screen(3);
                }
                Err(e) => {
                    log::info!("cb: open-gift {filename} err={e}");
                    ui.global::<Ui>().set_error(e.into());
                }
            }
        });
    }

    // Re-derive the seed-derived backup key and show it (WIF text + QR).
    {
        let ui_weak = ui_weak.clone();
        let state = state.clone();
        ui.global::<Callbacks>().on_backup_view(move || {
            let Some(u) = ui_weak.upgrade() else { return };
            u.global::<Ui>().set_error("".into());
            u.global::<Ui>().set_busy(true);

            let ui_weak = ui_weak.clone();
            let state = state.clone();
            Timer::single_shot(Duration::from_millis(150), move || {
                let Some(ui) = ui_weak.upgrade() else { return };
                let result = state
                    .borrow()
                    .open_gift
                    .as_ref()
                    .and_then(|r| r.backup_key_index)
                    .ok_or_else(|| "No backup key for this gift".to_string())
                    .and_then(|index| {
                        Security::default()
                            .app_seed()
                            .map_err(|_| "Device locked or seed unavailable".to_string())
                            .map(|app_seed| {
                                (index, keys::wif_encode(&derive::derive_backup_key(&app_seed, index)))
                            })
                    });
                ui.global::<Ui>().set_busy(false);
                match result {
                    Ok((index, wif)) => {
                        log::info!("cb: backup-view index={index} ok");
                        let d = ui.global::<Detail>();
                        d.set_backup_wif(wif.as_str().into());
                        d.set_backup_qr(qr_image(&wif));
                        d.set_show_backup(true);
                    }
                    Err(e) => {
                        log::info!("cb: backup-view err={e}");
                        ui.global::<Ui>().set_error(e.into());
                    }
                }
            });
        });
    }

    ui.run().expect("UI running");
}

// ---------------------------------------------------------------------------
// Persistence
// ---------------------------------------------------------------------------

/// Write bill PNG + full backup JSON to Airlock and the metadata record to
/// Internal storage. Returns (png_path, json_path).
fn save_gift(
    fs: &Fs,
    wallet: &wallet_core::Wallet,
    year: &str,
    ts: &str,
) -> Result<(String, String), String> {
    let png = bill::compose_bill(wallet, year, ts).map_err(|e| e.to_string())?;
    let addr12: String = wallet.address.chars().take(12).collect();

    ensure_airlock_mounted(fs)?;

    for loc in [Location::Airlock, Location::User] {
        if let Err(e) = fs.create_dir(GIFTS_DIR, loc) {
            if !matches!(e, fs::Error::FileAlreadyExists) {
                return Err(err_msg(&e));
            }
        }
    }

    let png_path = format!("{GIFTS_DIR}/bitcoin_bill_{addr12}.png");
    let json_path = format!("{GIFTS_DIR}/bitcoin_bill_{addr12}.json");
    write_bytes(fs, &png_path, Location::Airlock, &png)?;
    write_bytes(
        fs,
        &json_path,
        Location::Airlock,
        wallet_core::backup::airlock_json(wallet, ts).as_bytes(),
    )?;

    let record = wallet_core::backup::gift_record(wallet, ts);
    let record_json =
        serde_json_string(&record).map_err(|_| "Could not serialize record".to_string())?;
    write_bytes(
        fs,
        &format!("{GIFTS_DIR}/{addr12}.json"),
        Location::User,
        record_json.as_bytes(),
    )?;

    // Unmount Airlock after the export (the next save remounts): a fatfs
    // unmount is the full-flush path the USB mass-storage flow also uses, so
    // the exported files are durable even across an unclean sim shutdown.
    // Then flush the User volume, which carries both the airlock image file
    // and the metadata record.
    let mut flush_fs = fs.clone();
    if let Err(e) = flush_fs.unmount_airlock() {
        log::warn!("airlock unmount after export failed: {e:?}");
    }
    if let Err(e) = flush_fs.flush(Location::User) {
        log::warn!("flush User failed: {e:?}");
    }

    Ok((png_path, json_path))
}

fn serde_json_string(record: &GiftRecord) -> Result<String, ()> {
    wallet_core::backup::to_json_pretty(record).map_err(|_| ())
}

/// Mount Airlock before exporting (idempotent server-side). Nothing mounts
/// it in the hosted simulator; on a fresh sim the volume is also unformatted,
/// so a failed mount falls back to format-then-mount — the same recovery the
/// launcher offers behind its alert, and it only runs when there was no
/// readable filesystem to lose.
fn ensure_airlock_mounted(fs: &Fs) -> Result<(), String> {
    let mut fs = fs.clone();
    if fs.mount_airlock().is_ok() {
        return Ok(());
    }
    log::warn!("airlock mount failed — formatting (no readable filesystem)");
    fs.format_airlock()
        .and_then(|_| fs.mount_airlock())
        .map_err(|e| format!("Airlock unavailable: {}", err_msg(&e)))
}

fn write_bytes(fs: &Fs, path: &str, loc: Location, bytes: &[u8]) -> Result<(), String> {
    fs.open_file(path, loc, OpenFlags::CREATE)
        .and_then(|mut f| f.overwrite(bytes))
        .map_err(|e| err_msg(&e))
}

fn load_records(fs: &Fs) -> Vec<(String, GiftRecord)> {
    let mut records = Vec::new();
    if let Ok(dir) = fs.open_dir(GIFTS_DIR, Location::User) {
        while let Ok(Some(entry)) = dir.next_entry() {
            if entry.is_file && entry.name.to_lowercase().ends_with(".json") {
                match load_record(fs, &entry.name) {
                    Ok(r) => records.push((entry.name, r)),
                    Err(e) => log::warn!("skipping {}: {e}", entry.name),
                }
            }
        }
    }
    records
}

fn load_record(fs: &Fs, filename: &str) -> Result<GiftRecord, String> {
    let data = read_bytes(fs, &format!("{GIFTS_DIR}/{filename}"), Location::User)?;
    wallet_core::backup::from_json(&data).map_err(|_| "Invalid gift record".to_string())
}

/// Next unused backup-key index: one past the highest index on record.
fn next_backup_index(fs: &Fs) -> u32 {
    load_records(fs)
        .iter()
        .filter_map(|(_, r)| r.backup_key_index)
        .max()
        .map(|i| i + 1)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Misc helpers
// ---------------------------------------------------------------------------

fn qr_image(payload: &str) -> Image {
    qrcode::render(
        payload.as_bytes(),
        Color::from_rgb_u8(0, 0, 0),
        Color::from_rgb_u8(255, 255, 255),
    )
}

fn variant_name(v: Variant) -> &'static str {
    match v {
        Variant::Segwit => "SegWit gift",
        Variant::Taproot => "Taproot gift",
        Variant::TaprootBackup => "Taproot + backup gift",
    }
}

fn variant_name_log(v: Variant) -> &'static str {
    match v {
        Variant::Segwit => "segwit",
        Variant::Taproot => "taproot",
        Variant::TaprootBackup => "taproot-backup",
    }
}

fn read_bytes(fs: &Fs, path: &str, loc: Location) -> Result<Vec<u8>, String> {
    let mut file = fs
        .open_file(path, loc, OpenFlags::READ_ONLY)
        .map_err(|e| err_msg(&e))?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).map_err(|_| "Read failed".to_string())?;
    Ok(buf)
}

fn now_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn err_msg(e: &fs::Error) -> String {
    use slint_keyos_platform::fs::Error::*;
    match e {
        NoMedia => "Not connected".to_string(),
        AccessDenied => "Access denied".to_string(),
        FileNotFound => "Not found".to_string(),
        FileAlreadyExists => "Already exists".to_string(),
        FileInUse => "File is in use".to_string(),
        InvalidPath => "Invalid name".to_string(),
        other => format!("Error: {other:?}"),
    }
}
