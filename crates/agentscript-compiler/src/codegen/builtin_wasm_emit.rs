//! Registry-style emission for [`crate::hir::BuiltinKind`] in the guest v0 pipeline.
//!
//! **OCP:** add a variant by implementing [`BuiltinWasmEmit`] and registering it in [`emit_builtin_call`].

use wasm_encoder::Function;

use crate::frontend::ast::Span;
use crate::hir::{BuiltinKind, HirId};

use super::wasm::abi::{
    IMPORT_NZ, IMPORT_TA_ATR, IMPORT_TA_CROSSOVER, IMPORT_TA_CROSSUNDER, IMPORT_TA_EMA, IMPORT_TA_SMA,
    IMPORT_TA_TR,
};
use super::wasm::error::HirWasmError;

/// Minimal surface for emitting nested expressions inside a builtin handler.
pub trait HirWasmEmitContext {
    fn emit_expr(&self, func: &mut Function, id: HirId) -> Result<(), HirWasmError>;
    /// `ta_sma` / `ta_ema` first argument: [`super::wasm::abi::MA_SRC_CLOSE`] vs [`super::wasm::abi::MA_SRC_TRUE_RANGE`].
    fn ma_source_kind(&self, first_arg: HirId) -> i32;
    /// Push the period operand as **`i32`** (truncate f convert when HIR used `f64`).
    fn emit_ta_period_i32(&self, func: &mut Function, period: HirId, span: Span)
        -> Result<(), HirWasmError>;
}

/// One [`BuiltinKind`] lowering to wasm instructions.
pub trait BuiltinWasmEmit {
    fn emit(
        &self,
        ctx: &dyn HirWasmEmitContext,
        func: &mut Function,
        span: Span,
        args: &[HirId],
    ) -> Result<(), HirWasmError>;
}

struct InputIntEmit;

impl BuiltinWasmEmit for InputIntEmit {
    fn emit(
        &self,
        ctx: &dyn HirWasmEmitContext,
        func: &mut Function,
        span: Span,
        args: &[HirId],
    ) -> Result<(), HirWasmError> {
        if args.len() != 1 {
            return Err(HirWasmError::at(span, "input.int arity"));
        }
        ctx.emit_expr(func, args[0])?;
        Ok(())
    }
}

struct InputFloatEmit;

impl BuiltinWasmEmit for InputFloatEmit {
    fn emit(
        &self,
        ctx: &dyn HirWasmEmitContext,
        func: &mut Function,
        span: Span,
        args: &[HirId],
    ) -> Result<(), HirWasmError> {
        if args.len() != 1 {
            return Err(HirWasmError::at(span, "input.float arity"));
        }
        ctx.emit_expr(func, args[0])?;
        Ok(())
    }
}

struct TaSmaEmit;

impl BuiltinWasmEmit for TaSmaEmit {
    fn emit(
        &self,
        ctx: &dyn HirWasmEmitContext,
        func: &mut Function,
        span: Span,
        args: &[HirId],
    ) -> Result<(), HirWasmError> {
        if args.len() != 2 {
            return Err(HirWasmError::at(span, "ta.sma arity"));
        }
        let src = ctx.ma_source_kind(args[0]);
        func.instructions().i32_const(src);
        ctx.emit_ta_period_i32(func, args[1], span)?;
        func.instructions().call(IMPORT_TA_SMA);
        Ok(())
    }
}

struct TaEmaEmit;

impl BuiltinWasmEmit for TaEmaEmit {
    fn emit(
        &self,
        ctx: &dyn HirWasmEmitContext,
        func: &mut Function,
        span: Span,
        args: &[HirId],
    ) -> Result<(), HirWasmError> {
        if args.len() != 2 {
            return Err(HirWasmError::at(span, "ta.ema arity"));
        }
        let src = ctx.ma_source_kind(args[0]);
        func.instructions().i32_const(src);
        ctx.emit_ta_period_i32(func, args[1], span)?;
        func.instructions().call(IMPORT_TA_EMA);
        Ok(())
    }
}

struct TaCrossoverEmit;

impl BuiltinWasmEmit for TaCrossoverEmit {
    fn emit(
        &self,
        ctx: &dyn HirWasmEmitContext,
        func: &mut Function,
        span: Span,
        args: &[HirId],
    ) -> Result<(), HirWasmError> {
        if args.len() != 2 {
            return Err(HirWasmError::at(span, "ta.crossover arity"));
        }
        ctx.emit_expr(func, args[0])?;
        ctx.emit_expr(func, args[1])?;
        func.instructions().call(IMPORT_TA_CROSSOVER);
        Ok(())
    }
}

struct TaCrossunderEmit;

impl BuiltinWasmEmit for TaCrossunderEmit {
    fn emit(
        &self,
        ctx: &dyn HirWasmEmitContext,
        func: &mut Function,
        span: Span,
        args: &[HirId],
    ) -> Result<(), HirWasmError> {
        if args.len() != 2 {
            return Err(HirWasmError::at(span, "ta.crossunder arity"));
        }
        ctx.emit_expr(func, args[0])?;
        ctx.emit_expr(func, args[1])?;
        func.instructions().call(IMPORT_TA_CROSSUNDER);
        Ok(())
    }
}

struct MathMaxEmit;

impl BuiltinWasmEmit for MathMaxEmit {
    fn emit(
        &self,
        ctx: &dyn HirWasmEmitContext,
        func: &mut Function,
        span: Span,
        args: &[HirId],
    ) -> Result<(), HirWasmError> {
        if args.len() != 2 {
            return Err(HirWasmError::at(span, "math.max arity"));
        }
        ctx.emit_expr(func, args[0])?;
        ctx.emit_expr(func, args[1])?;
        func.instructions().f64_max();
        Ok(())
    }
}

struct MathMinEmit;

impl BuiltinWasmEmit for MathMinEmit {
    fn emit(
        &self,
        ctx: &dyn HirWasmEmitContext,
        func: &mut Function,
        span: Span,
        args: &[HirId],
    ) -> Result<(), HirWasmError> {
        if args.len() != 2 {
            return Err(HirWasmError::at(span, "math.min arity"));
        }
        ctx.emit_expr(func, args[0])?;
        ctx.emit_expr(func, args[1])?;
        func.instructions().f64_min();
        Ok(())
    }
}

struct MathAbsEmit;

impl BuiltinWasmEmit for MathAbsEmit {
    fn emit(
        &self,
        ctx: &dyn HirWasmEmitContext,
        func: &mut Function,
        span: Span,
        args: &[HirId],
    ) -> Result<(), HirWasmError> {
        if args.len() != 1 {
            return Err(HirWasmError::at(span, "math.abs arity"));
        }
        ctx.emit_expr(func, args[0])?;
        func.instructions().f64_abs();
        Ok(())
    }
}

struct TaTrEmit;

impl BuiltinWasmEmit for TaTrEmit {
    fn emit(
        &self,
        ctx: &dyn HirWasmEmitContext,
        func: &mut Function,
        span: Span,
        args: &[HirId],
    ) -> Result<(), HirWasmError> {
        if !args.is_empty() {
            return Err(HirWasmError::at(span, "ta.tr expects no arguments"));
        }
        let _: &dyn HirWasmEmitContext = ctx;
        func.instructions().call(IMPORT_TA_TR);
        Ok(())
    }
}

struct TaAtrEmit;

impl BuiltinWasmEmit for TaAtrEmit {
    fn emit(
        &self,
        ctx: &dyn HirWasmEmitContext,
        func: &mut Function,
        span: Span,
        args: &[HirId],
    ) -> Result<(), HirWasmError> {
        if args.len() != 1 {
            return Err(HirWasmError::at(span, "ta.atr arity"));
        }
        ctx.emit_ta_period_i32(func, args[0], span)?;
        func.instructions().call(IMPORT_TA_ATR);
        Ok(())
    }
}

struct NzEmit;

impl BuiltinWasmEmit for NzEmit {
    fn emit(
        &self,
        ctx: &dyn HirWasmEmitContext,
        func: &mut Function,
        span: Span,
        args: &[HirId],
    ) -> Result<(), HirWasmError> {
        if args.len() != 2 {
            return Err(HirWasmError::at(span, "nz arity"));
        }
        ctx.emit_expr(func, args[0])?;
        ctx.emit_expr(func, args[1])?;
        func.instructions().call(IMPORT_NZ);
        Ok(())
    }
}

static INPUT_INT: InputIntEmit = InputIntEmit;
static INPUT_FLOAT: InputFloatEmit = InputFloatEmit;
static TA_SMA: TaSmaEmit = TaSmaEmit;
static TA_EMA: TaEmaEmit = TaEmaEmit;
static TA_CROSSOVER: TaCrossoverEmit = TaCrossoverEmit;
static TA_CROSSUNDER: TaCrossunderEmit = TaCrossunderEmit;
static MATH_MAX: MathMaxEmit = MathMaxEmit;
static MATH_MIN: MathMinEmit = MathMinEmit;
static MATH_ABS: MathAbsEmit = MathAbsEmit;
static TA_TR: TaTrEmit = TaTrEmit;
static TA_ATR: TaAtrEmit = TaAtrEmit;
static NZ: NzEmit = NzEmit;

fn handler(kind: BuiltinKind) -> &'static dyn BuiltinWasmEmit {
    match kind {
        BuiltinKind::InputInt => &INPUT_INT,
        BuiltinKind::InputFloat => &INPUT_FLOAT,
        BuiltinKind::TaSma => &TA_SMA,
        BuiltinKind::TaEma => &TA_EMA,
        BuiltinKind::TaCrossover => &TA_CROSSOVER,
        BuiltinKind::TaCrossunder => &TA_CROSSUNDER,
        BuiltinKind::MathMax => &MATH_MAX,
        BuiltinKind::MathMin => &MATH_MIN,
        BuiltinKind::MathAbs => &MATH_ABS,
        BuiltinKind::TaTr => &TA_TR,
        BuiltinKind::TaAtr => &TA_ATR,
        BuiltinKind::Nz => &NZ,
    }
}

pub(crate) fn emit_builtin_call(
    kind: BuiltinKind,
    ctx: &dyn HirWasmEmitContext,
    func: &mut Function,
    span: Span,
    args: &[HirId],
) -> Result<(), HirWasmError> {
    handler(kind).emit(ctx, func, span, args)
}
