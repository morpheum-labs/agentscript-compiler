//! Registry-style emission for [`crate::hir::BuiltinKind`] in the guest v0 pipeline.
//!
//! **OCP:** add a variant by implementing [`BuiltinWasmEmit`] and registering it in [`emit_builtin_call`].

use wasm_encoder::Function;

use crate::frontend::ast::Span;
use crate::hir::{BuiltinKind, HirId};

use super::wasm::abi::{
    IMPORT_MATH_EXP, IMPORT_MATH_LOG, IMPORT_MATH_POW, IMPORT_NZ, IMPORT_TA_ATR, IMPORT_TA_CROSSOVER,
    IMPORT_TA_CROSSUNDER, IMPORT_TA_EMA, IMPORT_TA_SMA, IMPORT_TA_TR,
};
use super::wasm::error::HirWasmError;

/// Minimal surface for emitting nested expressions inside a builtin handler.
pub trait HirWasmEmitContext {
    fn emit_expr(&self, func: &mut Function, id: HirId) -> Result<(), HirWasmError>;
    /// Emit `id` as an `f64` stack value, promoting `i32` HIR (`int` / `bool` as `i32`) with `f64.convert_i32_s`.
    fn emit_expr_as_f64(&self, func: &mut Function, id: HirId, span: Span) -> Result<(), HirWasmError>;
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
        ctx.emit_expr_as_f64(func, args[0], span)?;
        ctx.emit_expr_as_f64(func, args[1], span)?;
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
        ctx.emit_expr_as_f64(func, args[0], span)?;
        ctx.emit_expr_as_f64(func, args[1], span)?;
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
        ctx.emit_expr_as_f64(func, args[0], span)?;
        ctx.emit_expr_as_f64(func, args[1], span)?;
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
        ctx.emit_expr_as_f64(func, args[0], span)?;
        ctx.emit_expr_as_f64(func, args[1], span)?;
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
        ctx.emit_expr_as_f64(func, args[0], span)?;
        func.instructions().f64_abs();
        Ok(())
    }
}

struct MathSqrtEmit;

impl BuiltinWasmEmit for MathSqrtEmit {
    fn emit(
        &self,
        ctx: &dyn HirWasmEmitContext,
        func: &mut Function,
        span: Span,
        args: &[HirId],
    ) -> Result<(), HirWasmError> {
        if args.len() != 1 {
            return Err(HirWasmError::at(span, "math.sqrt arity"));
        }
        ctx.emit_expr_as_f64(func, args[0], span)?;
        func.instructions().f64_sqrt();
        Ok(())
    }
}

struct MathRoundEmit;

impl BuiltinWasmEmit for MathRoundEmit {
    fn emit(
        &self,
        ctx: &dyn HirWasmEmitContext,
        func: &mut Function,
        span: Span,
        args: &[HirId],
    ) -> Result<(), HirWasmError> {
        if args.len() != 1 {
            return Err(HirWasmError::at(span, "math.round arity"));
        }
        ctx.emit_expr_as_f64(func, args[0], span)?;
        func.instructions().f64_nearest();
        Ok(())
    }
}

struct MathLogEmit;

impl BuiltinWasmEmit for MathLogEmit {
    fn emit(
        &self,
        ctx: &dyn HirWasmEmitContext,
        func: &mut Function,
        span: Span,
        args: &[HirId],
    ) -> Result<(), HirWasmError> {
        if args.len() != 1 {
            return Err(HirWasmError::at(span, "math.log arity"));
        }
        ctx.emit_expr_as_f64(func, args[0], span)?;
        func.instructions().call(IMPORT_MATH_LOG);
        Ok(())
    }
}

struct MathExpEmit;

impl BuiltinWasmEmit for MathExpEmit {
    fn emit(
        &self,
        ctx: &dyn HirWasmEmitContext,
        func: &mut Function,
        span: Span,
        args: &[HirId],
    ) -> Result<(), HirWasmError> {
        if args.len() != 1 {
            return Err(HirWasmError::at(span, "math.exp arity"));
        }
        ctx.emit_expr_as_f64(func, args[0], span)?;
        func.instructions().call(IMPORT_MATH_EXP);
        Ok(())
    }
}

struct MathPowEmit;

impl BuiltinWasmEmit for MathPowEmit {
    fn emit(
        &self,
        ctx: &dyn HirWasmEmitContext,
        func: &mut Function,
        span: Span,
        args: &[HirId],
    ) -> Result<(), HirWasmError> {
        if args.len() != 2 {
            return Err(HirWasmError::at(span, "math.pow arity"));
        }
        ctx.emit_expr_as_f64(func, args[0], span)?;
        ctx.emit_expr_as_f64(func, args[1], span)?;
        func.instructions().call(IMPORT_MATH_POW);
        Ok(())
    }
}

struct MathCeilEmit;

impl BuiltinWasmEmit for MathCeilEmit {
    fn emit(
        &self,
        ctx: &dyn HirWasmEmitContext,
        func: &mut Function,
        span: Span,
        args: &[HirId],
    ) -> Result<(), HirWasmError> {
        if args.len() != 1 {
            return Err(HirWasmError::at(span, "math.ceil arity"));
        }
        ctx.emit_expr_as_f64(func, args[0], span)?;
        func.instructions().f64_ceil();
        Ok(())
    }
}

struct MathFloorEmit;

impl BuiltinWasmEmit for MathFloorEmit {
    fn emit(
        &self,
        ctx: &dyn HirWasmEmitContext,
        func: &mut Function,
        span: Span,
        args: &[HirId],
    ) -> Result<(), HirWasmError> {
        if args.len() != 1 {
            return Err(HirWasmError::at(span, "math.floor arity"));
        }
        ctx.emit_expr_as_f64(func, args[0], span)?;
        func.instructions().f64_floor();
        Ok(())
    }
}

struct MathTruncEmit;

impl BuiltinWasmEmit for MathTruncEmit {
    fn emit(
        &self,
        ctx: &dyn HirWasmEmitContext,
        func: &mut Function,
        span: Span,
        args: &[HirId],
    ) -> Result<(), HirWasmError> {
        if args.len() != 1 {
            return Err(HirWasmError::at(span, "math.trunc arity"));
        }
        ctx.emit_expr_as_f64(func, args[0], span)?;
        func.instructions().f64_trunc();
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
        ctx.emit_expr_as_f64(func, args[0], span)?;
        ctx.emit_expr_as_f64(func, args[1], span)?;
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
static MATH_SQRT: MathSqrtEmit = MathSqrtEmit;
static MATH_ROUND: MathRoundEmit = MathRoundEmit;
static MATH_LOG: MathLogEmit = MathLogEmit;
static MATH_EXP: MathExpEmit = MathExpEmit;
static MATH_POW: MathPowEmit = MathPowEmit;
static MATH_CEIL: MathCeilEmit = MathCeilEmit;
static MATH_FLOOR: MathFloorEmit = MathFloorEmit;
static MATH_TRUNC: MathTruncEmit = MathTruncEmit;
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
        BuiltinKind::MathSqrt => &MATH_SQRT,
        BuiltinKind::MathRound => &MATH_ROUND,
        BuiltinKind::MathLog => &MATH_LOG,
        BuiltinKind::MathExp => &MATH_EXP,
        BuiltinKind::MathPow => &MATH_POW,
        BuiltinKind::MathCeil => &MATH_CEIL,
        BuiltinKind::MathFloor => &MATH_FLOOR,
        BuiltinKind::MathTrunc => &MATH_TRUNC,
        BuiltinKind::TaTr => &TA_TR,
        BuiltinKind::TaAtr => &TA_ATR,
        BuiltinKind::Nz => &NZ,
        BuiltinKind::SyminfoTicker | BuiltinKind::SyminfoPrefix => &INPUT_INT, // unreachable: filtered in emit_builtin_call
    }
}

pub(crate) fn emit_builtin_call(
    kind: BuiltinKind,
    ctx: &dyn HirWasmEmitContext,
    func: &mut Function,
    span: Span,
    args: &[HirId],
) -> Result<(), HirWasmError> {
    if matches!(
        kind,
        BuiltinKind::SyminfoTicker | BuiltinKind::SyminfoPrefix
    ) {
        return Err(HirWasmError::at(
            span,
            "`syminfo.ticker` / `syminfo.prefix` are only supported as `request.security` symbol/timeframe args in wasm codegen v0",
        ));
    }
    handler(kind).emit(ctx, func, span, args)
}
