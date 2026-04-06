//! Registry-style emission for [`crate::hir::BuiltinKind`] in the guest v0 pipeline.
//!
//! **OCP:** add a variant by implementing [`BuiltinWasmEmit`] and registering it in [`emit_builtin_call`].

use wasm_encoder::Function;

use crate::frontend::ast::Span;
use crate::hir::{BuiltinKind, HirId};

use super::wasm::abi::{
    IMPORT_TA_CROSSOVER, IMPORT_TA_CROSSUNDER, IMPORT_TA_EMA, IMPORT_TA_SMA,
};
use super::wasm::error::HirWasmError;

/// Minimal surface for emitting nested expressions inside a builtin handler.
pub trait HirWasmEmitContext {
    fn emit_expr(&self, func: &mut Function, id: HirId) -> Result<(), HirWasmError>;
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
        ctx.emit_expr(func, args[1])?;
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
        ctx.emit_expr(func, args[1])?;
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

static INPUT_INT: InputIntEmit = InputIntEmit;
static INPUT_FLOAT: InputFloatEmit = InputFloatEmit;
static TA_SMA: TaSmaEmit = TaSmaEmit;
static TA_EMA: TaEmaEmit = TaEmaEmit;
static TA_CROSSOVER: TaCrossoverEmit = TaCrossoverEmit;
static TA_CROSSUNDER: TaCrossunderEmit = TaCrossunderEmit;
static MATH_MAX: MathMaxEmit = MathMaxEmit;
static MATH_MIN: MathMinEmit = MathMinEmit;
static MATH_ABS: MathAbsEmit = MathAbsEmit;

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
