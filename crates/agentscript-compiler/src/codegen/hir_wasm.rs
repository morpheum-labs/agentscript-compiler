//! HIR → WebAssembly for the supported lowering subset (stack machine, `aether` host imports).

use std::collections::{HashMap, HashSet};

use wasm_encoder::{
    BlockType, CodeSection, ConstExpr, DataSection, EntityType, ExportKind, ExportSection, Function,
    FunctionSection, GlobalSection, GlobalType, ImportSection, MemorySection, MemoryType, Module,
    TypeSection, ValType,
};

use crate::codegen::builtin_wasm_emit::{emit_builtin_call, HirWasmEmitContext};
use crate::codegen::wasm::error::HirWasmError;

#[allow(unused_imports)] // Re-export for API / rustdoc; builtin emission uses `builtin_wasm_emit`.
pub use crate::codegen::wasm::abi::{
    GUEST_EXPORT_INIT_ABI, GUEST_EXPORT_INIT_LEGACY, GUEST_EXPORT_STEP_ABI,
    GUEST_EXPORT_STEP_LEGACY, GUEST_FUNC_BASE, IMPORT_INPUT_FLOAT, IMPORT_INPUT_INT, IMPORT_MATH_EXP,
    IMPORT_MATH_LOG, IMPORT_MATH_POW, IMPORT_NZ, IMPORT_PLOT, IMPORT_REQUEST_FINANCIAL,
    IMPORT_REQUEST_SECURITY,
    IMPORT_SERIES_CLOSE, IMPORT_SERIES_HIGH, IMPORT_SERIES_HIST, IMPORT_SERIES_HIST_AT,
    IMPORT_SERIES_LOW, IMPORT_SERIES_OPEN, IMPORT_SERIES_TIME, IMPORT_SERIES_VOLUME, IMPORT_TA_ATR,
    IMPORT_TA_CROSSOVER, IMPORT_TA_CROSSUNDER, IMPORT_TA_EMA, IMPORT_TA_SMA, IMPORT_TA_TR,
    MA_SRC_CLOSE, MA_SRC_TRUE_RANGE, series_hist_kind,
};

use crate::frontend::ast::{BinOp, PrimitiveType, Span, Type as AstType};
use crate::hir::{
    BuiltinKind, HirExpr, HirId, HirInputKind, HirLiteral, HirScript, HirStmt, HirType,
    HirUserFunction, SymbolId,
};

fn expr_span(hir: &HirScript, id: HirId) -> Span {
    let i = id.0 as usize;
    if hir.expr_spans.len() == hir.exprs.len() {
        return hir.expr_spans.get(i).copied().unwrap_or(Span::DUMMY);
    }
    // Best-effort when arena lengths diverge (should not happen from [`ast_lower`]).
    if let Some(s) = hir.expr_spans.get(i) {
        return *s;
    }
    hir.expr_spans.first().copied().unwrap_or(Span::DUMMY)
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

fn hir_expr_ty<'a>(hir: &'a HirScript, id: HirId) -> Result<&'a HirType, HirWasmError> {
    let span = expr_span(hir, id);
    let ex = hir
        .exprs
        .get(id.0 as usize)
        .ok_or_else(|| HirWasmError::at(span, "bad HirId"))?;
    Ok(match ex {
        HirExpr::Literal(_, t) => t,
        HirExpr::Variable(_, t) => t,
        HirExpr::Binary { ty, .. } => ty,
        HirExpr::BuiltinCall { ty, .. } => ty,
        HirExpr::UserCall { ty, .. } => ty,
        HirExpr::SeriesAccess { ty, .. } => ty,
        HirExpr::Select { ty, .. } => ty,
        HirExpr::Not { ty, .. } => ty,
        HirExpr::Security(sec) => &sec.ty,
        HirExpr::Financial(f) => &f.ty,
        HirExpr::Array { ty, .. } => ty,
        HirExpr::Plot { .. } => {
            return Err(HirWasmError::at(
                span,
                "internal: unexpected plot expr in ta period operand",
            ));
        }
    })
}

fn hir_ty_to_val(ty: &HirType, span: Span) -> Result<ValType, HirWasmError> {
    match ty {
        HirType::Simple(AstType::Primitive(PrimitiveType::Int)) => Ok(ValType::I32),
        HirType::Simple(AstType::Primitive(PrimitiveType::Float))
        | HirType::Series(AstType::Primitive(PrimitiveType::Float)) => Ok(ValType::F64),
        // Boolean conditions and comparisons are encoded as f64 0/1 in the guest v0 pipeline.
        HirType::Simple(AstType::Primitive(PrimitiveType::Bool))
        | HirType::Series(AstType::Primitive(PrimitiveType::Bool)) => Ok(ValType::F64),
        HirType::Array(_) | HirType::Matrix(_) => Err(HirWasmError::at(
            span,
            "array and matrix types are not supported in wasm codegen v0",
        )),
        _ => Err(HirWasmError::at(
            span,
            "only i32, f64, and bool-as-f64 HIR types are supported in wasm codegen v0",
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
            HirExpr::Financial(f) => {
                for (id, label) in [
                    (f.symbol, "symbol"),
                    (f.financial_id, "financial id"),
                    (f.period, "period"),
                ] {
                    let s = require_string_literal_for(hir, id, "request.financial", label)?;
                    pool.intern(&s);
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn require_string_literal(hir: &HirScript, id: HirId) -> Result<String, HirWasmError> {
    require_string_literal_for(hir, id, "request.security", "symbol or timeframe")
}

fn require_string_literal_for(
    hir: &HirScript,
    id: HirId,
    context: &str,
    role: &str,
) -> Result<String, HirWasmError> {
    let span = expr_span(hir, id);
    let ex = hir
        .exprs
        .get(id.0 as usize)
        .ok_or_else(|| HirWasmError::at(span, "bad HirId"))?;
    match ex {
        HirExpr::Literal(HirLiteral::String(s), _) => Ok(s.clone()),
        _ => Err(HirWasmError::at(
            span,
            format!("{context}: {role} must be a string literal for wasm codegen"),
        )),
    }
}

fn collect_lets(
    body: &[HirStmt],
    persist: &HashSet<SymbolId>,
    out: &mut Vec<(SymbolId, HirId)>,
) -> Result<(), HirWasmError> {
    for s in body {
        match s {
            HirStmt::Let { symbol, value } => {
                if !persist.contains(symbol) {
                    out.push((*symbol, *value));
                }
            }
            HirStmt::VarInit { .. } => {}
            HirStmt::Block(inner) => collect_lets(inner, persist, out)?,
            HirStmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                collect_lets(then_branch, persist, out)?;
                if let Some(e) = else_branch {
                    collect_lets(e, persist, out)?;
                }
            }
            HirStmt::Plot { .. } => {}
        }
    }
    Ok(())
}

/// One wasm local per [`SymbolId`]; use the first `Let`'s value only to infer the local type.
fn collect_lets_unique_symbols(
    body: &[HirStmt],
    persist: &HashSet<SymbolId>,
    out: &mut Vec<(SymbolId, HirId)>,
) -> Result<(), HirWasmError> {
    let mut flat: Vec<(SymbolId, HirId)> = Vec::new();
    collect_lets(body, persist, &mut flat)?;
    let mut first_val: HashMap<SymbolId, HirId> = HashMap::new();
    let mut order: Vec<SymbolId> = Vec::new();
    for (sym, val) in flat {
        use std::collections::hash_map::Entry;
        if let Entry::Vacant(e) = first_val.entry(sym) {
            e.insert(val);
            order.push(sym);
        }
    }
    for sym in order {
        let val = *first_val
            .get(&sym)
            .expect("internal: collect_lets_unique_symbols order/first_val mismatch");
        out.push((sym, val));
    }
    Ok(())
}

fn build_persist_global_pairs(hir: &HirScript) -> Vec<(SymbolId, u32, u32)> {
    let mut seen = HashSet::new();
    let mut pairs = Vec::new();
    let mut next_g = 0u32;
    let mut walk = |stmts: &[HirStmt]| {
        walk_persist_var_inits(stmts, &mut seen, &mut pairs, &mut next_g);
    };
    walk(&hir.body);
    for uf in &hir.user_functions {
        walk(&uf.body_stmts);
    }
    pairs
}

fn walk_persist_var_inits(
    stmts: &[HirStmt],
    seen: &mut HashSet<SymbolId>,
    pairs: &mut Vec<(SymbolId, u32, u32)>,
    next_g: &mut u32,
) {
    for s in stmts {
        match s {
            HirStmt::VarInit { symbol, .. } => {
                if seen.insert(*symbol) {
                    let gi = *next_g;
                    *next_g += 1;
                    let gv = *next_g;
                    *next_g += 1;
                    pairs.push((*symbol, gi, gv));
                }
            }
            HirStmt::Block(inner) => walk_persist_var_inits(inner, seen, pairs, next_g),
            HirStmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                walk_persist_var_inits(then_branch, seen, pairs, next_g);
                if let Some(e) = else_branch {
                    walk_persist_var_inits(e, seen, pairs, next_g);
                }
            }
            _ => {}
        }
    }
}

fn local_type_for_let(hir: &HirScript, value: HirId) -> Result<ValType, HirWasmError> {
    let span = expr_span(hir, value);
    let ex = hir
        .exprs
        .get(value.0 as usize)
        .ok_or_else(|| HirWasmError::at(span, "bad HirId"))?;
    let ty_ref = match ex {
        HirExpr::Literal(_, t) => t,
        HirExpr::Variable(_, t) => t,
        HirExpr::Binary { ty, .. }
        | HirExpr::BuiltinCall { ty, .. }
        | HirExpr::UserCall { ty, .. }
        | HirExpr::SeriesAccess { ty, .. }
        | HirExpr::Select { ty, .. }
        | HirExpr::Not { ty, .. } => ty,
        HirExpr::Array { ty, .. } => ty,
        HirExpr::Security(sec) => &sec.ty,
        HirExpr::Financial(f) => &f.ty,
        HirExpr::Plot { .. } => {
            return Err(HirWasmError::at(
                span,
                "plot expression shape not supported as let value",
            ));
        }
    };
    hir_ty_to_val(ty_ref, span)
}

struct Ctx<'a> {
    hir: &'a HirScript,
    /// Let-bound locals: symbol -> wasm local index (parameters use indices `0..params` in user fns).
    sym_to_local: HashMap<SymbolId, u32>,
    pool: &'a HashMap<String, (i32, i32)>,
    user_fn_indices: &'a HashMap<SymbolId, u32>,
    /// Temp f64 locals for `%` (`lhs` / `rhs`, then reuse second for `trunc(lhs/rhs)*rhs`).
    scratch_l: u32,
    scratch_r: u32,
    /// `var` / `varip`: `(inited_global, value_global)` wasm global indices.
    persist_globals: &'a HashMap<SymbolId, (u32, u32)>,
}

impl<'a> Ctx<'a> {
    fn symbol_name(&self, id: SymbolId) -> Option<&str> {
        self.hir.symbols.name(id)
    }

    fn input_import_for_name(&self, name: &str) -> Option<(i32, u32)> {
        for (i, inp) in self.hir.inputs.iter().enumerate() {
            if inp.name == name {
                let import = match inp.kind {
                    HirInputKind::Int(_) => IMPORT_INPUT_INT,
                    HirInputKind::Float(_) => IMPORT_INPUT_FLOAT,
                };
                return Some((i as i32, import));
            }
        }
        None
    }

    fn emit_expr(&self, func: &mut Function, id: HirId) -> Result<(), HirWasmError> {
        let span = expr_span(self.hir, id);
        let ex = self
            .hir
            .exprs
            .get(id.0 as usize)
            .ok_or_else(|| HirWasmError::at(span, "bad HirId"))?;
        match ex {
            HirExpr::Literal(lit, ty) => {
                match (lit, ty) {
                    (HirLiteral::Int(n), HirType::Simple(AstType::Primitive(PrimitiveType::Int))) => {
                        func.instructions().i32_const(i32::try_from(*n).map_err(|_| {
                            HirWasmError::at(span, "int literal out of i32 range")
                        })?);
                    }
                    (HirLiteral::Float(x), _) => {
                        func.instructions().f64_const((*x).into());
                    }
                    (HirLiteral::Bool(b), _) => {
                        func.instructions().i32_const(if *b { 1 } else { 0 });
                    }
                    _ => {
                        return Err(HirWasmError::at(
                            span,
                            "literal type not supported in wasm codegen",
                        ));
                    }
                }
            }
            HirExpr::Variable(sym, _) => {
                let name = self
                    .symbol_name(*sym)
                    .ok_or_else(|| HirWasmError::at(span, "unknown symbol"))?;
                if name == "close" {
                    func.instructions().call(IMPORT_SERIES_CLOSE);
                } else if name == "open" {
                    func.instructions().call(IMPORT_SERIES_OPEN);
                } else if name == "high" {
                    func.instructions().call(IMPORT_SERIES_HIGH);
                } else if name == "low" {
                    func.instructions().call(IMPORT_SERIES_LOW);
                } else if name == "volume" {
                    func.instructions().call(IMPORT_SERIES_VOLUME);
                } else if name == "time" {
                    func.instructions().call(IMPORT_SERIES_TIME);
                } else if let Some(&(_gi, gv)) = self.persist_globals.get(sym) {
                    func.instructions().global_get(gv);
                } else if let Some((idx, import_fn)) = self.input_import_for_name(name) {
                    func.instructions().i32_const(idx).call(import_fn);
                } else if let Some(&li) = self.sym_to_local.get(sym) {
                    func.instructions().local_get(li);
                } else {
                    return Err(HirWasmError::at(
                        span,
                        format!("unresolved variable `{name}`"),
                    ));
                }
            }
            HirExpr::Binary {
                op,
                lhs,
                rhs,
                ty,
            } => {
                let valty = hir_ty_to_val(ty, span)?;
                match op {
                    BinOp::And | BinOp::Or if valty == ValType::F64 => {
                        self.emit_expr(func, *lhs)?;
                        func.instructions().f64_const(0.0.into());
                        func.instructions().f64_ne();
                        self.emit_expr(func, *rhs)?;
                        func.instructions().f64_const(0.0.into());
                        func.instructions().f64_ne();
                        if *op == BinOp::And {
                            func.instructions().i32_and();
                        } else {
                            func.instructions().i32_or();
                        }
                        func.instructions().f64_convert_i32_u();
                    }
                    _ if valty == ValType::F64 => {
                        self.emit_expr(func, *lhs)?;
                        self.emit_expr(func, *rhs)?;
                        let mut ins = func.instructions();
                        match op {
                            BinOp::Add => {
                                ins.f64_add();
                            }
                            BinOp::Sub => {
                                ins.f64_sub();
                            }
                            BinOp::Mul => {
                                ins.f64_mul();
                            }
                            BinOp::Div => {
                                ins.f64_div();
                            }
                            BinOp::Mod => {
                                let sl = self.scratch_l;
                                let sr = self.scratch_r;
                                self.emit_expr(func, *lhs)?;
                                func.instructions().local_set(sl);
                                self.emit_expr(func, *rhs)?;
                                func.instructions().local_set(sr);
                                func
                                    .instructions()
                                    .local_get(sl)
                                    .local_get(sr)
                                    .f64_div()
                                    .f64_trunc()
                                    .local_get(sr)
                                    .f64_mul()
                                    .local_set(sr);
                                func
                                    .instructions()
                                    .local_get(sl)
                                    .local_get(sr)
                                    .f64_sub();
                            }
                            BinOp::Eq => {
                                ins.f64_eq();
                                ins.f64_convert_i32_u();
                            }
                            BinOp::Ne => {
                                ins.f64_ne();
                                ins.f64_convert_i32_u();
                            }
                            BinOp::Lt => {
                                ins.f64_lt();
                                ins.f64_convert_i32_u();
                            }
                            BinOp::Le => {
                                ins.f64_le();
                                ins.f64_convert_i32_u();
                            }
                            BinOp::Gt => {
                                ins.f64_gt();
                                ins.f64_convert_i32_u();
                            }
                            BinOp::Ge => {
                                ins.f64_ge();
                                ins.f64_convert_i32_u();
                            }
                            BinOp::And | BinOp::Or => {
                                return Err(HirWasmError::at(
                                    span,
                                    "internal: And/Or handled above",
                                ));
                            }
                        }
                    }
                    _ => {
                        return Err(HirWasmError::at(
                            span,
                            "binary result type not supported in wasm codegen",
                        ));
                    }
                }
            }
            HirExpr::Not { inner, ty } => {
                if hir_ty_to_val(ty, span)? != ValType::F64 {
                    return Err(HirWasmError::at(
                        span,
                        "unary `not` wasm v0 requires bool-as-f64 result type",
                    ));
                }
                let inner_ex = self
                    .hir
                    .exprs
                    .get(inner.0 as usize)
                    .ok_or_else(|| HirWasmError::at(span, "bad HirId in not operand"))?;
                if let HirExpr::Literal(HirLiteral::Bool(b), _) = inner_ex {
                    func
                        .instructions()
                        .f64_const((if *b { 0.0 } else { 1.0 }).into());
                } else {
                    func.instructions().f64_const(1.0.into());
                    self.emit_expr(func, *inner)?;
                    func.instructions().f64_sub();
                }
            }
            HirExpr::Select {
                cond,
                then_b,
                else_b,
                ty,
            } => {
                if hir_ty_to_val(ty, span)? != ValType::F64 {
                    return Err(HirWasmError::at(
                        span,
                        "select/ternary wasm v0 requires f64 result type",
                    ));
                }
                // Stack for `select`: `v1`, `v2`, `i32` (top). Result is `v1` if cond ≠ 0 else `v2`
                // (Pine `cond ? a : b` → `v1` = then, `v2` = else).
                self.emit_expr(func, *then_b)?;
                self.emit_expr(func, *else_b)?;
                self.emit_select_condition_i32(func, *cond, span)?;
                func.instructions().select();
            }
            HirExpr::BuiltinCall {
                kind,
                args,
                ty: _,
            } => emit_builtin_call(*kind, self, func, span, args)?,
            HirExpr::SeriesAccess { base, offset, ty } => {
                let base_span = expr_span(self.hir, *base);
                if hir_ty_to_val(ty, span)? != ValType::F64 {
                    return Err(HirWasmError::at(
                        span,
                        "series history wasm codegen expects f64 series element type",
                    ));
                }
                let base_ex = self
                    .hir
                    .exprs
                    .get(base.0 as usize)
                    .ok_or_else(|| HirWasmError::at(base_span, "bad HirId in SeriesAccess"))?;
                let HirExpr::Variable(sym, _) = base_ex else {
                    return Err(HirWasmError::at(
                        base_span,
                        "series history for non-variable base is not supported in wasm codegen yet",
                    ));
                };
                let name = self
                    .symbol_name(*sym)
                    .ok_or_else(|| HirWasmError::at(base_span, "unknown symbol in SeriesAccess"))?;
                let kind = series_hist_kind(name).ok_or_else(|| {
                    HirWasmError::at(
                        span,
                        format!("series history for `{name}` is not supported in wasm codegen yet"),
                    )
                })?;
                if kind == 0 {
                    func.instructions().i32_const(*offset);
                    func.instructions().call(IMPORT_SERIES_HIST);
                } else {
                    func.instructions().i32_const(kind);
                    func.instructions().i32_const(*offset);
                    func.instructions().call(IMPORT_SERIES_HIST_AT);
                }
            }
            HirExpr::Security(sec) => {
                let sym_span = expr_span(self.hir, sec.symbol);
                let tf_span = expr_span(self.hir, sec.timeframe);
                let (so, sl) = {
                    let s = require_string_literal(self.hir, sec.symbol)?;
                    self.pool
                        .get(&s)
                        .copied()
                        .ok_or_else(|| HirWasmError::at(sym_span, "missing string pool entry"))?
                };
                let (to, tl) = {
                    let s = require_string_literal(self.hir, sec.timeframe)?;
                    self.pool
                        .get(&s)
                        .copied()
                        .ok_or_else(|| HirWasmError::at(tf_span, "missing string pool entry"))?
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
            HirExpr::Financial(f) => {
                let sym_span = expr_span(self.hir, f.symbol);
                let id_span = expr_span(self.hir, f.financial_id);
                let per_span = expr_span(self.hir, f.period);
                let (so, sl) = {
                    let s = require_string_literal_for(
                        self.hir,
                        f.symbol,
                        "request.financial",
                        "symbol",
                    )?;
                    self.pool
                        .get(&s)
                        .copied()
                        .ok_or_else(|| HirWasmError::at(sym_span, "missing string pool entry"))?
                };
                let (ido, idl) = {
                    let s = require_string_literal_for(
                        self.hir,
                        f.financial_id,
                        "request.financial",
                        "financial id",
                    )?;
                    self.pool
                        .get(&s)
                        .copied()
                        .ok_or_else(|| HirWasmError::at(id_span, "missing string pool entry"))?
                };
                let (po, pl) = {
                    let s = require_string_literal_for(
                        self.hir,
                        f.period,
                        "request.financial",
                        "period",
                    )?;
                    self.pool
                        .get(&s)
                        .copied()
                        .ok_or_else(|| HirWasmError::at(per_span, "missing string pool entry"))?
                };
                let ignore = match f.ignore_invalid_symbol {
                    None => 0i32,
                    Some(iid) => {
                        let ispan = expr_span(self.hir, iid);
                        let ex = self
                            .hir
                            .exprs
                            .get(iid.0 as usize)
                            .ok_or_else(|| HirWasmError::at(ispan, "bad HirId"))?;
                        match ex {
                            HirExpr::Literal(HirLiteral::Bool(b), _) => i32::from(*b),
                            _ => {
                                return Err(HirWasmError::at(
                                    ispan,
                                    "request.financial: `ignore_invalid_symbol` must be a boolean literal for wasm codegen v0",
                                ));
                            }
                        }
                    }
                };
                func.instructions().i32_const(so);
                func.instructions().i32_const(sl);
                func.instructions().i32_const(ido);
                func.instructions().i32_const(idl);
                func.instructions().i32_const(po);
                func.instructions().i32_const(pl);
                func.instructions().i32_const(ignore);
                func.instructions().call(IMPORT_REQUEST_FINANCIAL);
            }
            HirExpr::UserCall { callee, args, .. } => {
                let fn_idx = self.user_fn_indices.get(callee).copied().ok_or_else(|| {
                    HirWasmError::at(span, "internal: user function not registered for wasm")
                })?;
                for a in args {
                    self.emit_expr(func, *a)?;
                }
                func.instructions().call(fn_idx);
            }
            HirExpr::Array { .. } => {
                return Err(HirWasmError::at(
                    span,
                    "array literals are not supported in wasm codegen v0",
                ));
            }
            HirExpr::Plot { .. } => {
                return Err(HirWasmError::at(
                    span,
                    "nested plot expression not supported",
                ));
            }
        }
        Ok(())
    }

    /// `select` expects an **`i32`** condition. Boolean literals push `i32` directly; float-shaped
    /// bools (comparisons, `f64` 0/1) use `f64.ne` against `0.0`.
    fn emit_select_condition_i32(
        &self,
        func: &mut Function,
        cond: HirId,
        span: Span,
    ) -> Result<(), HirWasmError> {
        let ex = self
            .hir
            .exprs
            .get(cond.0 as usize)
            .ok_or_else(|| HirWasmError::at(span, "bad HirId in select cond"))?;
        match ex {
            HirExpr::Literal(HirLiteral::Bool(b), _) => {
                func.instructions().i32_const(i32::from(*b));
            }
            _ => {
                self.emit_expr(func, cond)?;
                func.instructions().f64_const(0.0.into());
                func.instructions().f64_ne();
            }
        }
        Ok(())
    }

    fn emit_stmt(&self, func: &mut Function, stmt: &HirStmt) -> Result<(), HirWasmError> {
        match stmt {
            HirStmt::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let cspan = expr_span(self.hir, *cond);
                self.emit_select_condition_i32(func, *cond, cspan)?;
                func.instructions().if_(BlockType::Empty);
                for s in then_branch {
                    self.emit_stmt(func, s)?;
                }
                if let Some(else_stmts) = else_branch {
                    func.instructions().else_();
                    for s in else_stmts {
                        self.emit_stmt(func, s)?;
                    }
                }
                func.instructions().end();
                Ok(())
            }
            HirStmt::Let { symbol, value } => {
                let vspan = expr_span(self.hir, *value);
                self.emit_expr(func, *value)?;
                if let Some(&(_gi, gv)) = self.persist_globals.get(symbol) {
                    func.instructions().global_set(gv);
                } else {
                    let li = *self
                        .sym_to_local
                        .get(symbol)
                        .ok_or_else(|| HirWasmError::at(vspan, "let without local slot"))?;
                    func.instructions().local_set(li);
                }
                Ok(())
            }
            HirStmt::VarInit { symbol, value } => {
                let vspan = expr_span(self.hir, *value);
                let &(g_inited, g_val) = self.persist_globals.get(symbol).ok_or_else(|| {
                    HirWasmError::at(
                        vspan,
                        "internal: VarInit missing wasm global mapping",
                    )
                })?;
                func.instructions().global_get(g_inited);
                func.instructions().i32_eqz();
                func.instructions().if_(BlockType::Empty);
                self.emit_expr(func, *value)?;
                func.instructions().global_set(g_val);
                func.instructions().i32_const(1);
                func.instructions().global_set(g_inited);
                func.instructions().end();
                Ok(())
            }
            HirStmt::Plot { expr, .. } => {
                self.emit_expr(func, *expr)?;
                func.instructions().call(IMPORT_PLOT);
                Ok(())
            }
            HirStmt::Block(stmts) => {
                for s in stmts {
                    self.emit_stmt(func, s)?;
                }
                Ok(())
            }
        }
    }
}

impl<'a> HirWasmEmitContext for Ctx<'a> {
    fn emit_expr(&self, func: &mut Function, id: HirId) -> Result<(), HirWasmError> {
        Ctx::emit_expr(self, func, id)
    }

    fn ma_source_kind(&self, first_arg: HirId) -> i32 {
        let Some(ex) = self.hir.exprs.get(first_arg.0 as usize) else {
            return MA_SRC_CLOSE;
        };
        match ex {
            HirExpr::BuiltinCall {
                kind: BuiltinKind::TaTr,
                ..
            } => MA_SRC_TRUE_RANGE,
            _ => MA_SRC_CLOSE,
        }
    }

    fn emit_ta_period_i32(
        &self,
        func: &mut Function,
        period: HirId,
        span: Span,
    ) -> Result<(), HirWasmError> {
        self.emit_expr(func, period)?;
        let ty = hir_expr_ty(self.hir, period)?;
        let vt = hir_ty_to_val(ty, span)?;
        if vt == ValType::F64 {
            func.instructions().i32_trunc_sat_f64_s();
        }
        Ok(())
    }
}

fn encode_user_function_body(
    hir: &HirScript,
    uf: &HirUserFunction,
    pool: &HashMap<String, (i32, i32)>,
    user_fn_indices: &HashMap<SymbolId, u32>,
    persist_globals: &HashMap<SymbolId, (u32, u32)>,
) -> Result<Function, HirWasmError> {
    let mut let_pairs: Vec<(SymbolId, HirId)> = Vec::new();
    collect_lets_unique_symbols(&uf.body_stmts, &hir.persist_symbols, &mut let_pairs)?;
    let mut sym_to_local: HashMap<SymbolId, u32> = HashMap::new();
    let mut next_local: u32 = 0;
    for p in &uf.params {
        sym_to_local.insert(*p, next_local);
        next_local += 1;
    }
    let mut local_defs: Vec<(u32, ValType)> = Vec::new();
    for (sym, val) in &let_pairs {
        let vt = local_type_for_let(hir, *val)?;
        local_defs.push((1, vt));
        sym_to_local.insert(*sym, next_local);
        next_local += 1;
    }
    let scratch_l = next_local;
    local_defs.push((1, ValType::F64));
    let scratch_r = scratch_l + 1;
    local_defs.push((1, ValType::F64));

    let ctx = Ctx {
        hir,
        sym_to_local,
        pool,
        user_fn_indices,
        scratch_l,
        scratch_r,
        persist_globals,
    };
    let mut f = Function::new(local_defs);
    for s in &uf.body_stmts {
        ctx.emit_stmt(&mut f, s)?;
    }
    ctx.emit_expr(&mut f, uf.result)?;
    f.instructions().end();
    Ok(f)
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
/// | `ta_ema` | `(i32 period) -> f64` | EMA of host close series |
/// | `request_security` | `(i32 sym_off, i32 sym_len, i32 tf_off, i32 tf_len, f64 inner) -> f64` | Strings in guest memory |
/// | `request_financial` | `(i32 sym_o, i32 sym_l, i32 id_o, i32 id_l, i32 per_o, i32 per_l, i32 ignore) -> f64` | `ignore` is `0`/`1`; v0 requires string + bool literals |
/// | `plot` | `(f64) -> ()` | Plot side effect |
/// | `series_hist` | `(i32 offset) -> f64` | Primary series (`close`) value `offset` bars ago (v0) |
/// | `input_float` | `(i32 idx) -> f64` | `idx` = index of a float input in [`HirScript::inputs`] |
/// | `ta_crossover` | `(f64 a, f64 b) -> f64` | Stateful host: `1.0` when `a > b && prev_a <= prev_b` |
/// | `ta_crossunder` | `(f64 a, f64 b) -> f64` | Stateful host: `1.0` when `a < b && prev_a >= prev_b` |
/// | `math_log` | `(f64) -> f64` | Natural log (`math.log`); NaN/`na` policy on host |
/// | `math_exp` | `(f64) -> f64` | `math.exp` |
/// | `math_pow` | `(f64, f64) -> f64` | `math.pow` |
///
/// Exports: `memory`, legacy [`GUEST_EXPORT_INIT_LEGACY`] / [`GUEST_EXPORT_STEP_LEGACY`], and
/// [`GUEST_EXPORT_INIT_ABI`] / [`GUEST_EXPORT_STEP_ABI`] (same func indices as `init` / `on_bar`).
#[must_use]
pub fn emit_hir_guest_wasm(hir: &HirScript) -> Result<Vec<u8>, HirWasmError> {
    let mut pool = StringPool::new();
    collect_strings(hir, &mut pool)?;

    let persist_pairs = build_persist_global_pairs(hir);
    let persist_globals: HashMap<SymbolId, (u32, u32)> = persist_pairs
        .iter()
        .map(|(s, i, v)| (*s, (*i, *v)))
        .collect();

    let mut let_pairs: Vec<(SymbolId, HirId)> = Vec::new();
    collect_lets_unique_symbols(&hir.body, &hir.persist_symbols, &mut let_pairs)?;

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
    let t_in_float = types.len();
    types.ty().function([ValType::I32], [ValType::F64]);
    let t_sma = types.len();
    types.ty().function([ValType::I32, ValType::I32], [ValType::F64]);
    let t_sec = types.len();
    types.ty().function(
        [ValType::I32, ValType::I32, ValType::I32, ValType::I32, ValType::F64],
        [ValType::F64],
    );
    let t_plot = types.len();
    types.ty().function([ValType::F64], []);
    let t_series_hist = types.len();
    types.ty().function([ValType::I32], [ValType::F64]);
    let t_hist_at = types.len();
    types.ty().function([ValType::I32, ValType::I32], [ValType::F64]);
    let t_nz = types.len();
    types.ty().function([ValType::F64, ValType::F64], [ValType::F64]);
    let t_unary_f64 = types.len();
    types.ty().function([ValType::F64], [ValType::F64]);
    let t_cross = types.len();
    types.ty().function([ValType::F64, ValType::F64], [ValType::F64]);
    let t_financial = types.len();
    types.ty().function(
        [
            ValType::I32,
            ValType::I32,
            ValType::I32,
            ValType::I32,
            ValType::I32,
            ValType::I32,
            ValType::I32,
        ],
        [ValType::F64],
    );
    let t_void = types.len();
    types.ty().function([], []);

    let mut user_fn_type_indices: Vec<u32> = Vec::new();
    for uf in &hir.user_functions {
        let params_ty: Vec<ValType> = (0..uf.params.len())
            .map(|_| ValType::F64)
            .collect();
        let ti = types.len();
        types.ty().function(params_ty, [ValType::F64]);
        user_fn_type_indices.push(ti);
    }

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
    imports.import(
        "aether",
        "series_hist",
        EntityType::Function(t_series_hist),
    );
    // Append new imports after existing v0 indices so [`IMPORT_*`] constants stay stable.
    imports.import("aether", "ta_ema", EntityType::Function(t_sma));
    imports.import(
        "aether",
        "input_float",
        EntityType::Function(t_in_float),
    );
    imports.import(
        "aether",
        "ta_crossover",
        EntityType::Function(t_cross),
    );
    imports.import(
        "aether",
        "ta_crossunder",
        EntityType::Function(t_cross),
    );
    imports.import("aether", "series_open", EntityType::Function(t_close));
    imports.import("aether", "series_high", EntityType::Function(t_close));
    imports.import("aether", "series_low", EntityType::Function(t_close));
    imports.import("aether", "series_volume", EntityType::Function(t_close));
    imports.import("aether", "series_time", EntityType::Function(t_close));
    imports.import(
        "aether",
        "series_hist_at",
        EntityType::Function(t_hist_at),
    );
    imports.import("aether", "ta_tr", EntityType::Function(t_close));
    imports.import("aether", "ta_atr", EntityType::Function(t_series_hist));
    imports.import("aether", "nz", EntityType::Function(t_nz));
    imports.import("aether", "math_log", EntityType::Function(t_unary_f64));
    imports.import("aether", "math_exp", EntityType::Function(t_unary_f64));
    imports.import("aether", "math_pow", EntityType::Function(t_cross));
    imports.import(
        "aether",
        "request_financial",
        EntityType::Function(t_financial),
    );

    let fn_init = GUEST_FUNC_BASE;
    let fn_on_bar = fn_init + 1;
    let mut user_fn_indices: HashMap<SymbolId, u32> = HashMap::new();
    let mut next_user_fn = fn_on_bar + 1;
    for uf in &hir.user_functions {
        user_fn_indices.insert(uf.symbol, next_user_fn);
        next_user_fn += 1;
    }

    let mut functions = FunctionSection::new();
    functions.function(t_void);
    functions.function(t_void);
    for ti in &user_fn_type_indices {
        functions.function(*ti);
    }

    let mut memory = MemorySection::new();
    memory.memory(MemoryType {
        minimum: 1,
        maximum: None,
        memory64: false,
        shared: false,
        page_size_log2: None,
    });

    let mut globals = GlobalSection::new();
    for _ in 0..persist_pairs.len() {
        globals.global(
            GlobalType {
                val_type: ValType::I32,
                mutable: true,
                shared: false,
            },
            &ConstExpr::i32_const(0),
        );
        globals.global(
            GlobalType {
                val_type: ValType::F64,
                mutable: true,
                shared: false,
            },
            &ConstExpr::f64_const(0.0f64.into()),
        );
    }

    let mut exports = ExportSection::new();
    exports.export("memory", ExportKind::Memory, 0);
    exports.export(GUEST_EXPORT_INIT_LEGACY, ExportKind::Func, fn_init);
    exports.export(GUEST_EXPORT_STEP_LEGACY, ExportKind::Func, fn_on_bar);
    exports.export(GUEST_EXPORT_INIT_ABI, ExportKind::Func, fn_init);
    exports.export(GUEST_EXPORT_STEP_ABI, ExportKind::Func, fn_on_bar);

    let mut code = CodeSection::new();
    // init
    {
        let mut f = Function::new([]);
        for (_s, g_inited, _gv) in &persist_pairs {
            f.instructions().i32_const(0).global_set(*g_inited);
        }
        f.instructions().end();
        code.function(&f);
    }
    // on_bar
    {
        let scratch_l = next_local;
        local_defs.push((1, ValType::F64));
        let scratch_r = scratch_l + 1;
        local_defs.push((1, ValType::F64));

        let ctx = Ctx {
            hir,
            sym_to_local,
            pool: &pool.map,
            user_fn_indices: &user_fn_indices,
            scratch_l,
            scratch_r,
            persist_globals: &persist_globals,
        };
        let mut f = Function::new(local_defs);
        for stmt in &hir.body {
            ctx.emit_stmt(&mut f, stmt)?;
        }
        f.instructions().end();
        code.function(&f);
    }
    for uf in &hir.user_functions {
        let f = encode_user_function_body(
            hir,
            uf,
            &pool.map,
            &user_fn_indices,
            &persist_globals,
        )?;
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
    if !globals.is_empty() {
        module.section(&globals);
    }
    module.section(&exports);
    module.section(&code);
    if !pool.bytes.is_empty() {
        module.section(&data);
    }

    Ok(module.finish())
}
