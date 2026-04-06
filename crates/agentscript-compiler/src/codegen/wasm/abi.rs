//! Aether guest ABI v0: stable import indices, export names, and module validation.
//!
//! **SRP:** constants and [`validate_guest_abi_v0`] live here; emission uses the same indices in
//! [`crate::codegen::hir_wasm`].

use std::fmt;

use wasmparser::{Parser, Payload};

/// Host import indices (stable ABI v0; must match Aether / MWVM stubs).
pub const IMPORT_SERIES_CLOSE: u32 = 0;
pub const IMPORT_INPUT_INT: u32 = 1;
/// `(i32 src_kind, i32 period) -> f64` — `src_kind` 0 = close, 1 = true range (`ta.tr`).
pub const IMPORT_TA_SMA: u32 = 2;
pub const IMPORT_REQUEST_SECURITY: u32 = 3;
pub const IMPORT_PLOT: u32 = 4;
/// `close[offset]` — same as [`IMPORT_SERIES_HIST_AT`] with kind `0` (legacy import).
pub const IMPORT_SERIES_HIST: u32 = 5;
/// Same signature as [`IMPORT_TA_SMA`]: `(i32 src_kind, i32 period) -> f64`.
pub const IMPORT_TA_EMA: u32 = 6;
pub const IMPORT_INPUT_FLOAT: u32 = 7;
/// Stateful host: compares `(a,b)` to previous bar; returns bool as f64 (`0`/`1`).
pub const IMPORT_TA_CROSSOVER: u32 = 8;
pub const IMPORT_TA_CROSSUNDER: u32 = 9;
pub const IMPORT_SERIES_OPEN: u32 = 10;
pub const IMPORT_SERIES_HIGH: u32 = 11;
pub const IMPORT_SERIES_LOW: u32 = 12;
pub const IMPORT_SERIES_VOLUME: u32 = 13;
pub const IMPORT_SERIES_TIME: u32 = 14;
/// `(i32 series_kind, i32 offset) -> f64` — see [`series_kind_for_hist`].
pub const IMPORT_SERIES_HIST_AT: u32 = 15;
/// `() -> f64` — current bar true range (matches Pine `ta.tr` series).
pub const IMPORT_TA_TR: u32 = 16;
/// `(i32 period) -> f64` — ATR on host stream.
pub const IMPORT_TA_ATR: u32 = 17;
/// `(f64 a, f64 y) -> f64` — Pine `nz`-style replacement when `a` is na.
pub const IMPORT_NZ: u32 = 18;

/// First function index defined in the guest module (after all `aether` imports).
pub const GUEST_FUNC_BASE: u32 = IMPORT_NZ + 1;

/// `ta_sma` / `ta_ema` first argument: host moving-average source stream.
pub const MA_SRC_CLOSE: i32 = 0;
pub const MA_SRC_TRUE_RANGE: i32 = 1;

/// `series_hist_at(series_kind, offset)` — must match guest emission order.
#[must_use]
pub fn series_hist_kind(name: &str) -> Option<i32> {
    match name {
        "close" => Some(0),
        "open" => Some(1),
        "high" => Some(2),
        "low" => Some(3),
        "volume" => Some(4),
        "time" => Some(5),
        _ => None,
    }
}

/// Legacy / CLI-friendly export names (same function indices as [`GUEST_EXPORT_INIT_ABI`] / [`GUEST_EXPORT_STEP_ABI`]).
pub const GUEST_EXPORT_INIT_LEGACY: &str = "init";
pub const GUEST_EXPORT_STEP_LEGACY: &str = "on_bar";

/// Names aligned with `aether_common::guest_abi` (dual-exported with legacy names).
pub const GUEST_EXPORT_INIT_ABI: &str = "aether_strategy_init";
pub const GUEST_EXPORT_STEP_ABI: &str = "aether_strategy_step";

/// Specification of the v0 guest contract: required `aether` imports (presence; order matches emission).
pub static GUEST_ABI_V0_IMPORTS: &[(&str, &str)] = &[
    ("aether", "series_close"),
    ("aether", "input_int"),
    ("aether", "ta_sma"),
    ("aether", "request_security"),
    ("aether", "plot"),
    ("aether", "series_hist"),
    ("aether", "ta_ema"),
    ("aether", "input_float"),
    ("aether", "ta_crossover"),
    ("aether", "ta_crossunder"),
    ("aether", "series_open"),
    ("aether", "series_high"),
    ("aether", "series_low"),
    ("aether", "series_volume"),
    ("aether", "series_time"),
    ("aether", "series_hist_at"),
    ("aether", "ta_tr"),
    ("aether", "ta_atr"),
    ("aether", "nz"),
];

/// Required export names for a full guest strategy module from [`crate::codegen::emit_hir_guest_wasm`].
pub static GUEST_ABI_V0_EXPORTS: &[&str] = &[
    "memory",
    GUEST_EXPORT_INIT_LEGACY,
    GUEST_EXPORT_STEP_LEGACY,
    GUEST_EXPORT_INIT_ABI,
    GUEST_EXPORT_STEP_ABI,
];

/// Marker type linking docs to [`GUEST_ABI_V0_IMPORTS`] / [`GUEST_ABI_V0_EXPORTS`].
#[derive(Debug, Clone, Copy)]
pub struct GuestAbiV0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbiValidationError {
    pub message: String,
}

impl fmt::Display for AbiValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for AbiValidationError {}

fn abi_err(msg: impl Into<String>) -> AbiValidationError {
    AbiValidationError {
        message: msg.into(),
    }
}

/// Validate WASM bytes and check required v0 imports/exports (presence, any order within the section).
pub fn validate_guest_abi_v0(wasm: &[u8]) -> Result<(), AbiValidationError> {
    wasmparser::validate(wasm).map_err(|e| abi_err(format!("wasm validate: {e}")))?;

    let mut imports: Vec<(String, String)> = Vec::new();
    let mut exports: Vec<String> = Vec::new();
    for payload in Parser::new(0).parse_all(wasm) {
        let Ok(p) = payload else { continue };
        match p {
            Payload::ImportSection(reader) => {
                for imp in reader {
                    let Ok(i) = imp else { continue };
                    imports.push((i.module.to_string(), i.name.to_string()));
                }
            }
            Payload::ExportSection(reader) => {
                for exp in reader {
                    let Ok(e) = exp else { continue };
                    exports.push(e.name.to_string());
                }
            }
            _ => {}
        }
    }

    for &(module, name) in GUEST_ABI_V0_IMPORTS {
        if !imports
            .iter()
            .any(|(m, n)| m == module && n == name)
        {
            return Err(abi_err(format!(
                "missing import `{module}::{name}`, have {imports:?}"
            )));
        }
    }

    for &name in GUEST_ABI_V0_EXPORTS {
        if !exports.iter().any(|e| e == name) {
            return Err(abi_err(format!(
                "missing export `{name}`, have {exports:?}"
            )));
        }
    }

    Ok(())
}
