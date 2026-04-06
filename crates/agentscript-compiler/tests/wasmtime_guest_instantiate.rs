//! wasmtime smoke: link full `aether` import set, **instantiate**, call **`aether_strategy_init`** then
//! **`aether_strategy_step`** in a short loop (guest ABI **v1** export signatures).
//!
//! **Sync:** import registrations must match `aether-mwvm` `link_aether_guest_abi_v0`
//! (`aether/crates/aether-mwvm/src/aether_guest_stubs.rs`) and `GUEST_ABI_V0_IMPORTS` (names + signatures).

use agentscript_compiler::{compile_script_to_wasm_v0, parse_script, GUEST_ABI_V0_IMPORTS};
use wasmtime::{Engine, Linker, Module, Store};

/// Keep aligned with `aether-mwvm` `aether_guest_stubs.rs`.
fn link_aether_guest_abi_v0<T>(linker: &mut Linker<T>) -> wasmtime::Result<()> {
    linker.func_wrap("aether", "series_close", || -> f64 { 0.0 })?;
    linker.func_wrap("aether", "input_int", |_: i32| -> i32 { 0 })?;
    linker.func_wrap("aether", "ta_sma", |_: i32, _: i32| -> f64 { 0.0 })?;
    linker.func_wrap(
        "aether",
        "request_security",
        |_: i32, _: i32, _: i32, _: i32, inner: f64| -> f64 { inner },
    )?;
    linker.func_wrap("aether", "plot", |_: f64| {})?;
    linker.func_wrap("aether", "series_hist", |_: i32| -> f64 { 0.0 })?;
    linker.func_wrap("aether", "ta_ema", |_: i32, _: i32| -> f64 { 0.0 })?;
    linker.func_wrap("aether", "input_float", |_: i32| -> f64 { 0.0 })?;
    linker.func_wrap("aether", "ta_crossover", |_: f64, _: f64| -> f64 { 0.0 })?;
    linker.func_wrap("aether", "ta_crossunder", |_: f64, _: f64| -> f64 { 0.0 })?;
    linker.func_wrap("aether", "series_open", || -> f64 { 0.0 })?;
    linker.func_wrap("aether", "series_high", || -> f64 { 0.0 })?;
    linker.func_wrap("aether", "series_low", || -> f64 { 0.0 })?;
    linker.func_wrap("aether", "series_volume", || -> f64 { 0.0 })?;
    linker.func_wrap("aether", "series_time", || -> f64 { 0.0 })?;
    linker.func_wrap("aether", "series_hist_at", |_: i32, _: i32| -> f64 { 0.0 })?;
    linker.func_wrap("aether", "ta_tr", || -> f64 { 0.0 })?;
    linker.func_wrap("aether", "ta_atr", |_: i32| -> f64 { 0.0 })?;
    linker.func_wrap("aether", "nz", |x: f64, y: f64| -> f64 {
        if x.is_nan() {
            y
        } else {
            x
        }
    })?;
    linker.func_wrap("aether", "math_log", |x: f64| -> f64 { x.ln() })?;
    linker.func_wrap("aether", "math_exp", |x: f64| -> f64 { x.exp() })?;
    linker.func_wrap("aether", "math_pow", |b: f64, e: f64| -> f64 { b.powf(e) })?;
    linker.func_wrap(
        "aether",
        "request_financial",
        |_: i32,
         _: i32,
         _: i32,
         _: i32,
         _: i32,
         _: i32,
         _: i32,
         _: i32,
         _: i32,
         _: i32|
         -> f64 { 0.0 },
    )?;
    Ok(())
}

fn instantiate_guest_wasm(wasm: &[u8]) {
    let engine = Engine::default();
    let module = Module::new(&engine, wasm).expect("wasmtime module");
    let mut linker: Linker<()> = Linker::new(&engine);
    link_aether_guest_abi_v0(&mut linker).expect("link stubs");
    let mut store = Store::new(&engine, ());
    let instance = linker
        .instantiate(&mut store, &module)
        .expect("instantiate with aether imports");

    let init = instance
        .get_typed_func::<(), i32>(&mut store, "aether_strategy_init")
        .expect("aether_strategy_init export");
    let step = instance
        .get_typed_func::<(i32,), i32>(&mut store, "aether_strategy_step")
        .expect("aether_strategy_step export");

    assert_eq!(init.call(&mut store, ()).expect("init"), 0);
    for bar in 0..3 {
        assert_eq!(step.call(&mut store, (bar,)).expect("step"), 0);
    }
}

#[test]
fn guest_abi_v0_import_list_matches_stub_linker() {
    let mut names: Vec<&str> = GUEST_ABI_V0_IMPORTS.iter().map(|(_, n)| *n).collect();
    names.sort_unstable();
    let mut expected = vec![
        "input_float",
        "input_int",
        "math_exp",
        "math_log",
        "math_pow",
        "nz",
        "plot",
        "request_financial",
        "request_security",
        "series_close",
        "series_hist",
        "series_hist_at",
        "series_high",
        "series_low",
        "series_open",
        "series_time",
        "series_volume",
        "ta_atr",
        "ta_crossover",
        "ta_crossunder",
        "ta_ema",
        "ta_sma",
        "ta_tr",
    ];
    expected.sort_unstable();
    assert_eq!(names, expected, "update stub registrations when ABI imports change");
}

#[test]
fn wasmtime_instantiate_plot_close() {
    const SRC: &str = r#"//@version=6
indicator("t")
plot(close)
"#;
    let script = parse_script("t", SRC).expect("parse");
    let wasm = compile_script_to_wasm_v0(&script).expect("compile");
    instantiate_guest_wasm(&wasm);
}

#[test]
fn wasmtime_instantiate_request_financial() {
    const SRC: &str = r#"//@version=6
indicator("fin")
plot(request.financial("NASDAQ:MSFT", "TOTAL_REVENUE", "FY"))
"#;
    let script = parse_script("t", SRC).expect("parse");
    let wasm = compile_script_to_wasm_v0(&script).expect("compile");
    instantiate_guest_wasm(&wasm);
}

#[test]
fn wasmtime_instantiate_request_financial_gaps_currency() {
    const SRC: &str = r#"//@version=6
indicator("fin2")
plot(request.financial("NASDAQ:MSFT", "TOTAL_REVENUE", "FY", barmerge.gaps_off, false, "USDT"))
"#;
    let script = parse_script("t", SRC).expect("parse");
    let wasm = compile_script_to_wasm_v0(&script).expect("compile");
    instantiate_guest_wasm(&wasm);
}
