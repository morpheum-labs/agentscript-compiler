//! HIR → WebAssembly for the supported lowering subset (stack machine, `aether` host imports).

use std::collections::HashMap;

use wasm_encoder::{
    CodeSection, ConstExpr, DataSection, EntityType, ExportKind, ExportSection, Function,
    ImportSection, MemorySection, MemoryType, Module, TypeSection, ValType,
};

use crate::frontend::ast::BinOp;
use crate::frontend::ast::PrimitiveType;
use crate::frontend::ast::Type as AstType;
use crate::hir::builtin::BuiltinKind;
use crate::hir::expr::HirExpr;
use crate::hir::ids::{HirId, SymbolId};
use crate::hir::literal::HirLiteral;
use crate::hir::script::HirScript;
use crate::hir::stmt::HirStmt;
use crate::hir::ty::HirType;

/// Host import indices (stable ABI v0; must match Aether / MWVM stubs).
pub const IMPORT_SERIES_CLOSE: u32 = 0;
pub const IMPORT_INPUT_INT: u32 = 1;
pub const IMPORT_TA_SMA: u32 = 2;
pub const IMPORT_REQUEST_SECURITY: u32 = 3;
pub const IMPORT_PLOT: u32 = 4;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HirWasmError {
    #[error("HIR wasm: {0}")]
    Msg(String),
}

impl HirWasmError {
    fn unsupported(msg: impl Into<String>) -> Self {
        Self::Msg(msg.into())
    }
}

struct StringPool {
    bytes: Vec<u8>,
    /// UTF-8 string -> (offset, len)
    map: HashMap<String, (i32, i32)>,
}

impl StringPool {
    fn new() -> Self {
        Self {
            bytes: Vec::new(),
            map: HashMap::new(),
        }
    }

    fn intern(&mut self, s: &str) -> (i32, i32) {
        if let Some(&pair) = self.map.get(s) {
            return pair;
        }
        let off = self.bytes.len() as i32;
        self.bytes.extend_from_slice(s.as_bytes());
        let len = s.len() as i32;
        self.map.insert(s.to_string(), (off, len));
        (off, len)
    }
}

fn hir_ty_to_val(ty: &HirType) -> Result<ValType, HirWasmError> {
    match ty {
        HirType::Simple(AstType::Primitive(PrimitiveType::Int)) => Ok(ValType::I32),
        HirType::Simple(AstType::Primitive(PrimitiveType::Float))
        | HirType::Series(AstType::Primitive(PrimitiveType::Float)) => Ok(ValType::F64),
        _ => Err(HirWasmError::unsupported(
            "only i32 and f64 HIR types are supported in wasm codegen",
        )),
    }
}

fn collect_strings(hir: &HirScript, pool: &mut StringPool) -> Result<(), HirWasmError> {
    for e in &hir.exprs {
        match e {
            HirExpr::Literal(HirLiteral::String(s), _) => {
                pool.intern(s);
            }
            HirExpr::Security(sec) => {
                let sym = require_string_literal(hir, sec.symbol)?;
                let tf = require_string_literal(hir, sec.timeframe)?;
                pool.intern(&sym);
                pool.intern(&tf);
            }
            _ => {}
        }
    }
    Ok(())
}

fn require_string_literal(hir: &HirScript, id: HirId) -> Result<String, HirWasmError> {
    let ex = hir
        .exprs
        .get(id.0 as usize)
        .ok_or_else(|| HirWasmError::unsupported("bad HirId"))?;
    match ex {
        HirExpr::Literal(HirLiteral::String(s), _) => Ok(s.clone()),
        _ => Err(HirWasmError::unsupported(
            "request.security symbol/timeframe must be string literals for wasm codegen",
        )),
    }
}

fn collect_lets(body: &[HirStmt], out: &mut Vec<(SymbolId, HirId)>) -> Result<(), HirWasmError> {
    for s in body {
        match s {
            HirStmt::Let { symbol, value } => out.push((*symbol, *value)),
            HirStmt::Block(inner) => collect_lets(inner, out)?,
            HirStmt::Plot { .. } => {}
        }
    }
    Ok(())
}

fn local_type_for_let(hir: &HirScript, value: HirId) -> Result<ValType, HirWasmError> {
    let ex = hir
        .exprs
        .get(value.0 as usize)
        .ok_or_else(|| HirWasmError::unsupported("bad HirId"))?;
    hir_ty_to_val(match ex {
        HirExpr::Literal(_, t) => t,
        HirExpr::Variable(_, t) => t,
        HirExpr::Binary { ty, .. }
        | HirExpr::BuiltinCall { ty, .. }
        | HirExpr::SeriesAccess { ty, .. }
        | HirExpr::Security(sec) => &sec.ty,
        HirExpr::Plot { .. } => {
            return Err(HirWasmError::unsupported(
                "plot expression shape not supported as let value",
            ));
        }
    })
}

struct Ctx<'a> {
    hir: &'a HirScript,
    /// Let-bound locals: symbol -> wasm local index (excluding params; on_bar has none).
    sym_to_local: HashMap<SymbolId, u32>,
    pool: &'a HashMap<String, (i32, i32)>,
}

impl<'a> Ctx<'a> {
    fn symbol_name(&self, id: SymbolId) -> Option<&str> {
        self.hir.symbols.name(id)
    }

    fn input_index(&self, name: &str) -> Option<i32> {
        self.hir
            .inputs
            .iter()
            .position(|i| i.name == name)
            .map(|p| p as i32)
    }

    fn emit_expr(&self, func: &mut Function, id: HirId) -> Result<(), HirWasmError> {
        let ex = self
            .hir
            .exprs
            .get(id.0 as usize)
            .ok_or_else(|| HirWasmError::unsupported("bad HirId"))?;
        match ex {
            HirExpr::Literal(lit, ty) => {
                match (lit, ty) {
                    (HirLiteral::Int(n), HirType::Simple(AstType::Primitive(PrimitiveType::Int))) => {
                        func.instructions().i32_const(i32::try_from(*n).map_err(|_| {
                            HirWasmError::unsupported("int literal out of i32 range")
                        })?);
                    }
                    (HirLiteral::Float(x), _) => {
                        func.instructions().f64_const((*x).into());
                    }
                    (HirLiteral::Bool(b), _) => {
                        func.instructions().i32_const(if *b { 1 } else { 0 });
                    }
                    _ => {
                        return Err(HirWasmError::unsupported(
                            "literal type not supported in wasm codegen",
                        ));
                    }
                }
            }
            HirExpr::Variable(sym, _) => {
                let name = self
                    .symbol_name(*sym)
                    .ok_or_else(|| HirWasmError::unsupported("unknown symbol"))?;
                if name == "close" {
                    func.instructions().call(IMPORT_SERIES_CLOSE);
                } else if let Some(idx) = self.input_index(name) {
                    func.instructions().i32_const(idx).call(IMPORT_INPUT_INT);
                } else if let Some(&li) = self.sym_to_local.get(sym) {
                    func.instructions().local_get(li);
                } else {
                    return Err(HirWasmError::unsupported(format!(
                        "unresolved variable `{name}`"
                    )));
                }
            }
            HirExpr::Binary {
                op,
                lhs,
                rhs,
                ty,
            } => {
                if hir_ty_to_val(ty)? != ValType::F64 {
                    return Err(HirWasmError::unsupported("only f64 binary ops for wasm"));
                }
                self.emit_expr(func, *lhs)?;
                self.emit_expr(func, *rhs)?;
                let ins = func.instructions();
                match op {
                    BinOp::Add => ins.f64_add(),
                    BinOp::Sub => ins.f64_sub(),
                    BinOp::Mul => ins.f64_mul(),
                    BinOp::Div => ins.f64_div(),
                    _ => {
                        return Err(HirWasmError::unsupported(
                            "binary operator not supported in wasm codegen",
                        ));
                    }
                };
            }
            HirExpr::BuiltinCall {
                kind,
                args,
                ty: _,
            } => match kind {
                BuiltinKind::InputInt => {
                    if args.len() != 1 {
                        return Err(HirWasmError::unsupported("input.int arity"));
                    }
                    self.emit_expr(func, args[0])?;
                    // stack: i32 default — for surface `input.int(14)` expr
                }
                BuiltinKind::TaSma => {
                    if args.len() != 2 {
                        return Err(HirWasmError::unsupported("ta.sma arity"));
                    }
                    // Host uses primary series; pass period only (second arg).
                    self.emit_expr(func, args[1])?;
                    // period may be i32 variable
                    func.instructions().call(IMPORT_TA_SMA);
                }
            },
            HirExpr::SeriesAccess { .. } => {
                return Err(HirWasmError::unsupported(
                    "series history access not supported in wasm codegen",
                ));
            }
            HirExpr::Security(sec) => {
                let (so, sl) = {
                    let s = require_string_literal(self.hir, sec.symbol)?;
                    self.pool
                        .get(&s)
                        .copied()
                        .ok_or_else(|| HirWasmError::unsupported("missing string pool entry"))?
                };
                let (to, tl) = {
                    let s = require_string_literal(self.hir, sec.timeframe)?;
                    self.pool
                        .get(&s)
                        .copied()
                        .ok_or_else(|| HirWasmError::unsupported("missing string pool entry"))?
                };
                func
                    .instructions()
                    .i32_const(so)
                    .i32_const(sl)
                    .i32_const(to)
                    .i32_const(tl);
                self.emit_expr(func, sec.expression)?;
                func.instructions().call(IMPORT_REQUEST_SECURITY);
            }
            HirExpr::Plot { .. } => {
                return Err(HirWasmError::unsupported(
                    "nested plot expression not supported",
                ));
            }
        }
        Ok(())
    }

    fn emit_stmt(&self, func: &mut Function, stmt: &HirStmt) -> Result<(), HirWasmError> {
        match stmt {
            HirStmt::Let { symbol, value } => {
                self.emit_expr(func, *value)?;
                let li = *self
                    .sym_to_local
                    .get(symbol)
                    .ok_or_else(|| HirWasmError::unsupported("let without local slot"))?;
                func.instructions().local_set(li);
            }
            HirStmt::Plot { expr, .. } => {
                self.emit_expr(func, *expr)?;
                func.instructions().call(IMPORT_PLOT);
            }
            HirStmt::Block(stmts) => {
                for s in stmts {
                    self.emit_stmt(func, s)?;
                }
            }
        }
        Ok(())
    }
}

/// Emit a `wasm32` module: imports from module **`aether`**, exported **`memory`**, **`init`**, **`on_bar`**.
///
/// # Host ABI (v0)
///
/// | Import | Signature | Role |
/// |--------|-----------|------|
/// | `series_close` | `() -> f64` | Current bar close |
/// | `input_int` | `(i32 idx) -> i32` | `idx` = index in [`HirScript::inputs`] |
/// | `ta_sma` | `(i32 period) -> f64` | SMA of host close series |
/// | `request_security` | `(i32 sym_off, i32 sym_len, i32 tf_off, i32 tf_len, f64 inner) -> f64` | Strings in guest memory |
/// | `plot` | `(f64) -> ()` | Plot side effect |
#[must_use]
pub fn emit_hir_guest_wasm(hir: &HirScript) -> Result<Vec<u8>, HirWasmError> {
    let mut pool = StringPool::new();
    collect_strings(hir, &mut pool)?;

    let mut let_pairs: Vec<(SymbolId, HirId)> = Vec::new();
    collect_lets(&hir.body, &mut let_pairs)?;

    let mut local_defs: Vec<(u32, ValType)> = Vec::new();
    let mut sym_to_local: HashMap<SymbolId, u32> = HashMap::new();
    let mut next_local: u32 = 0;
    for (sym, val) in &let_pairs {
        let vt = local_type_for_let(hir, *val)?;
        local_defs.push((1, vt));
        sym_to_local.insert(*sym, next_local);
        next_local += 1;
    }

    let mut types = TypeSection::new();
    let t_close = types.len();
    types.ty().function([], [ValType::F64]);
    let t_in = types.len();
    types.ty().function([ValType::I32], [ValType::I32]);
    let t_sma = types.len();
    types.ty().function([ValType::I32], [ValType::F64]);
    let t_sec = types.len();
    types.ty().function(
        [ValType::I32, ValType::I32, ValType::I32, ValType::I32, ValType::F64],
        [ValType::F64],
    );
    let t_plot = types.len();
    types.ty().function([ValType::F64], []);
    let t_void = types.len();
    types.ty().function([], []);

    let mut imports = ImportSection::new();
    imports.import(
        "aether",
        "series_close",
        EntityType::Function(t_close),
    );
    imports.import("aether", "input_int", EntityType::Function(t_in));
    imports.import("aether", "ta_sma", EntityType::Function(t_sma));
    imports.import(
        "aether",
        "request_security",
        EntityType::Function(t_sec),
    );
    imports.import("aether", "plot", EntityType::Function(t_plot));

    let mut functions = FunctionSection::new();
    functions.function(t_void);
    functions.function(t_void);

    let mut memory = MemorySection::new();
    memory.memory(MemoryType {
        minimum: 1,
        maximum: None,
        memory64: false,
        shared: false,
        page_size_log2: None,
    });

    let fn_init = IMPORT_PLOT + 1;
    let fn_on_bar = fn_init + 1;

    let mut exports = ExportSection::new();
    exports.export("memory", ExportKind::Memory, 0);
    exports.export("init", ExportKind::Func, fn_init);
    exports.export("on_bar", ExportKind::Func, fn_on_bar);

    let mut code = CodeSection::new();
    // init
    {
        let mut f = Function::new([]);
        f.instructions().end();
        code.function(&f);
    }
    // on_bar
    {
        let ctx = Ctx {
            hir,
            sym_to_local,
            pool: &pool.map,
        };
        let mut f = Function::new(local_defs);
        for stmt in &hir.body {
            ctx.emit_stmt(&mut f, stmt)?;
        }
        f.instructions().end();
        code.function(&f);
    }

    let mut data = DataSection::new();
    if !pool.bytes.is_empty() {
        data.active(0, &ConstExpr::i32_const(0), pool.bytes.iter().copied());
    }

    let mut module = Module::new();
    module.section(&types);
    module.section(&imports);
    module.section(&functions);
    module.section(&memory);
    module.section(&exports);
    module.section(&code);
    if !data_bytes.is_empty() {
        module.section(&data);
    }

    Ok(module.finish())
}
