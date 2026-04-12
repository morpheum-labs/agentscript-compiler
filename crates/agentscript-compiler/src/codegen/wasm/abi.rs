//! Aether guest strategy module: stable **`aether` import** indices and **export** names/types.
//!
//! **Import table:** `GUEST_ABI_V0_IMPORTS` keeps the historical v0 label for **index stability**
//! in [`crate::codegen::hir_wasm`]; the list is the current required import set.
//!
//! **Exports (guest ABI v1):** `init` / `aether_strategy_init` are **`() -> i32`**; `on_bar` /
//! `aether_strategy_step` are **`(i32 bar_index) -> i32`**. Validated by [`validate_guest_abi_v1`].

use std::fmt;

use wasmparser::{ExternalKind, FuncType, Parser, Payload, TypeRef, ValType};

/// Host import indices (stable; must match Aether / MWVM stubs).
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
/// `(f64) -> f64` — natural logarithm (`math.log`).
pub const IMPORT_MATH_LOG: u32 = 19;
/// `(f64) -> f64` — `math.exp`.
pub const IMPORT_MATH_EXP: u32 = 20;
/// `(f64, f64) -> f64` — `math.pow`.
pub const IMPORT_MATH_POW: u32 = 21;
/// `(i32×10) -> f64` — `sym`/`id`/`period`/`currency` string slices in guest memory; `gaps` `0`/`1` (`gaps_off`/`gaps_on`); `ignore` `0`/`1`; `currency` `-1`,`0` = default.
pub const IMPORT_REQUEST_FINANCIAL: u32 = 22;
/// `(i32 kind, i32 dst_off, i32 max_len) -> i32` — host writes UTF-8 series string for current bar; returns byte length (truncates to `max_len`; **`-1`** = na / empty).
pub const IMPORT_SERIES_STRING_UTF8: u32 = 23;

/// First function index defined in the guest module (after all `aether` imports).
pub const GUEST_FUNC_BASE: u32 = IMPORT_SERIES_STRING_UTF8 + 1;

/// [`IMPORT_SERIES_STRING_UTF8`] `kind` argument: `syminfo.ticker`.
pub const SERIES_STRING_KIND_SYMINFO_TICKER: i32 = 0;
/// [`IMPORT_SERIES_STRING_UTF8`] `kind` argument: `syminfo.prefix`.
pub const SERIES_STRING_KIND_SYMINFO_PREFIX: i32 = 1;

/// Max bytes host may write per `request.security` symbol or timeframe scratch slot ([`emit_hir_guest_wasm`] pads linear memory).
pub const SERIES_STRING_SCRATCH_SLOT_MAX: i32 = 512;

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

/// Specification of the required `aether` imports (presence; order matches emission).
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
    ("aether", "math_log"),
    ("aether", "math_exp"),
    ("aether", "math_pow"),
    ("aether", "request_financial"),
    ("aether", "series_string_utf8"),
];

/// Required export names for a full guest strategy module from [`crate::codegen::emit_hir_guest_wasm`].
pub static GUEST_ABI_V0_EXPORTS: &[&str] = &[
    "memory",
    GUEST_EXPORT_INIT_LEGACY,
    GUEST_EXPORT_STEP_LEGACY,
    GUEST_EXPORT_INIT_ABI,
    GUEST_EXPORT_STEP_ABI,
];

/// Marker linking docs to [`GUEST_ABI_V0_IMPORTS`] / [`GUEST_ABI_V0_EXPORTS`].
#[derive(Debug, Clone, Copy)]
pub struct GuestAbiV1;

/// Back-compat alias (same contract as [`GuestAbiV1`]).
pub type GuestAbiV0 = GuestAbiV1;

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

fn func_type_for_index(
    func_idx: u32,
    import_func_type_idx: &[u32],
    local_func_type_idx: &[u32],
    types: &[FuncType],
) -> Result<FuncType, AbiValidationError> {
    let n_imp = import_func_type_idx.len() as u32;
    let ty_idx = if func_idx < n_imp {
        import_func_type_idx
            .get(func_idx as usize)
            .copied()
            .ok_or_else(|| abi_err("import func index out of range"))?
    } else {
        let li = (func_idx - n_imp) as usize;
        *local_func_type_idx
            .get(li)
            .ok_or_else(|| abi_err("local func index out of range"))?
    };
    types
        .get(ty_idx as usize)
        .cloned()
        .ok_or_else(|| abi_err(format!("type index {ty_idx} out of range")))
}

/// Validate WASM bytes: **imports** (names), **exports** (names), and **guest ABI v1** export signatures.
pub fn validate_guest_abi_v1(wasm: &[u8]) -> Result<(), AbiValidationError> {
    wasmparser::validate(wasm).map_err(|e| abi_err(format!("wasm validate: {e}")))?;

    let mut func_types: Vec<FuncType> = Vec::new();
    let mut import_func_type_idx: Vec<u32> = Vec::new();
    let mut local_func_type_idx: Vec<u32> = Vec::new();
    /// Full import section order (module, name, kind). Function imports populate
    /// [`import_func_type_idx`] in WASM function-index order.
    let mut import_entries: Vec<(String, String, TypeRef)> = Vec::new();
    let mut export_funcs: Vec<(String, u32)> = Vec::new();
    let mut export_memory_names: Vec<String> = Vec::new();

    for payload in Parser::new(0).parse_all(wasm) {
        let Ok(p) = payload else { continue };
        match p {
            Payload::TypeSection(reader) => {
                for ft in reader.into_iter_err_on_gc_types() {
                    let ft = ft.map_err(|e| abi_err(format!("type section: {e}")))?;
                    func_types.push(ft);
                }
            }
            Payload::ImportSection(reader) => {
                for imp in reader {
                    let imp = imp.map_err(|e| abi_err(format!("import section: {e}")))?;
                    let ty = imp.ty;
                    import_entries.push((imp.module.to_string(), imp.name.to_string(), ty));
                    if let TypeRef::Func(ti) = ty {
                        import_func_type_idx.push(ti);
                    }
                }
            }
            Payload::FunctionSection(reader) => {
                for ty in reader {
                    let ty = ty.map_err(|e| abi_err(format!("function section: {e}")))?;
                    local_func_type_idx.push(ty);
                }
            }
            Payload::ExportSection(reader) => {
                for exp in reader {
                    let exp = exp.map_err(|e| abi_err(format!("export section: {e}")))?;
                    match exp.kind {
                        ExternalKind::Func => {
                            export_funcs.push((exp.name.to_string(), exp.index));
                        }
                        ExternalKind::Memory => {
                            export_memory_names.push(exp.name.to_string());
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    // Aether links by stable function import index: the first N imports must match exactly.
    if import_entries.len() < GUEST_ABI_V0_IMPORTS.len() {
        return Err(abi_err(format!(
            "expected at least {} imports in guest ABI order, got {}",
            GUEST_ABI_V0_IMPORTS.len(),
            import_entries.len()
        )));
    }
    for (i, &(module, name)) in GUEST_ABI_V0_IMPORTS.iter().enumerate() {
        let (m, n, ty) = &import_entries[i];
        if m != module || n != name {
            return Err(abi_err(format!(
                "import index {i}: expected `{module}::{name}`, got `{m}::{n}` (stable ABI order)"
            )));
        }
        if !matches!(ty, TypeRef::Func(_)) {
            return Err(abi_err(format!(
                "import index {i} (`{module}::{name}`) must be a function import"
            )));
        }
    }

    let mut export_names: Vec<String> = export_funcs.iter().map(|(n, _)| n.clone()).collect();
    export_names.extend(export_memory_names.iter().cloned());

    for &name in GUEST_ABI_V0_EXPORTS {
        if name == "memory" {
            if !export_memory_names.iter().any(|e| e == name) {
                return Err(abi_err(format!(
                    "missing export `{name}`, have func exports {:?} and memory exports {:?}",
                    export_funcs.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(),
                    export_memory_names
                )));
            }
            continue;
        }
        let func_idx = export_funcs
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, i)| *i)
            .ok_or_else(|| {
                abi_err(format!("missing export `{name}`, have {export_names:?}"))
            })?;
        let ft = func_type_for_index(
            func_idx,
            &import_func_type_idx,
            &local_func_type_idx,
            &func_types,
        )?;
        let is_init = name == GUEST_EXPORT_INIT_LEGACY || name == GUEST_EXPORT_INIT_ABI;
        let is_step = name == GUEST_EXPORT_STEP_LEGACY || name == GUEST_EXPORT_STEP_ABI;
        if is_init {
            if !ft.params().is_empty() || ft.results() != [ValType::I32] {
                return Err(abi_err(format!(
                    "export `{name}` must be () -> i32, got {}",
                    FuncTypeDisplay(&ft)
                )));
            }
        } else if is_step {
            if ft.params() != [ValType::I32] || ft.results() != [ValType::I32] {
                return Err(abi_err(format!(
                    "export `{name}` must be (i32) -> i32, got {}",
                    FuncTypeDisplay(&ft)
                )));
            }
        }
    }

    let export_idx = |name: &str| -> Option<u32> {
        export_funcs
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, i)| *i)
    };
    if export_idx(GUEST_EXPORT_INIT_LEGACY) != export_idx(GUEST_EXPORT_INIT_ABI) {
        return Err(abi_err(
            "`init` and `aether_strategy_init` must export the same function index",
        ));
    }
    if export_idx(GUEST_EXPORT_STEP_LEGACY) != export_idx(GUEST_EXPORT_STEP_ABI) {
        return Err(abi_err(
            "`on_bar` and `aether_strategy_step` must export the same function index",
        ));
    }

    Ok(())
}

struct FuncTypeDisplay<'a>(&'a FuncType);

impl fmt::Display for FuncTypeDisplay<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Back-compat: same as [`validate_guest_abi_v1`].
pub fn validate_guest_abi_v0(wasm: &[u8]) -> Result<(), AbiValidationError> {
    validate_guest_abi_v1(wasm)
}
