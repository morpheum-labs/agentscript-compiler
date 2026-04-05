//! Default AST walking hooks; extend with specific visitors instead of growing central `match` trees.

use crate::frontend::ast::{
    ElseBody, Expr, ExprKind, FnBody, FnDecl, IfStmt, Item, Script, Stmt, StmtKind,
};

/// Walk expressions and statements; override selective methods for single-purpose passes.
pub trait AstVisitor {
    type Error;

    fn visit_script(&mut self, script: &Script) -> Result<(), Self::Error> {
        for item in &script.items {
            self.visit_item(item)?;
        }
        Ok(())
    }

    fn visit_item(&mut self, item: &Item) -> Result<(), Self::Error> {
        match item {
            Item::Stmt(s) => self.visit_stmt(s),
            Item::FnDecl(f) => self.visit_fn_decl(f),
            Item::Export(crate::frontend::ast::ExportDecl::Fn(f)) => self.visit_fn_decl(f),
            Item::ScriptDecl(d) => {
                for (_, e) in &d.args {
                    self.visit_expr(e)?;
                }
                Ok(())
            }
            Item::Enum(e) | Item::Export(crate::frontend::ast::ExportDecl::Enum(e)) => {
                for v in &e.variants {
                    self.visit_expr(&v.value)?;
                }
                Ok(())
            }
            Item::TypeDef(t) | Item::Export(crate::frontend::ast::ExportDecl::TypeDef(t)) => {
                for field in &t.fields {
                    self.visit_expr(&field.default)?;
                }
                Ok(())
            }
            Item::Export(crate::frontend::ast::ExportDecl::Var(v)) => {
                self.visit_expr(&v.value)?;
                Ok(())
            }
            Item::Import(_) => Ok(()),
        }
    }

    fn visit_fn_decl(&mut self, f: &FnDecl) -> Result<(), Self::Error> {
        for p in &f.params {
            if let Some(d) = &p.default {
                self.visit_expr(d)?;
            }
        }
        match &f.body {
            FnBody::Expr(e) => self.visit_expr(e),
            FnBody::Block(stmts) => {
                for s in stmts {
                    self.visit_stmt(s)?;
                }
                Ok(())
            }
        }
    }

    fn visit_stmt(&mut self, stmt: &Stmt) -> Result<(), Self::Error> {
        match &stmt.kind {
            StmtKind::VarDecl(v) => self.visit_expr(&v.value),
            StmtKind::Assign { value, .. } | StmtKind::TupleAssign { value, .. } => {
                self.visit_expr(value)
            }
            StmtKind::Expr(e) => self.visit_expr(e),
            StmtKind::Block(stmts) => {
                for s in stmts {
                    self.visit_stmt(s)?;
                }
                Ok(())
            }
            StmtKind::If(i) => self.visit_if_stmt(i),
            StmtKind::For {
                from,
                to,
                by,
                body,
                ..
            } => {
                self.visit_expr(from)?;
                self.visit_expr(to)?;
                if let Some(b) = by {
                    self.visit_expr(b)?;
                }
                for s in body {
                    self.visit_stmt(s)?;
                }
                Ok(())
            }
            StmtKind::ForIn { iterable, body, .. } => {
                self.visit_expr(iterable)?;
                for s in body {
                    self.visit_stmt(s)?;
                }
                Ok(())
            }
            StmtKind::Switch {
                scrutinee,
                cases,
                default,
            } => {
                if let Some(s) = scrutinee {
                    self.visit_expr(s)?;
                }
                for (e, arm) in cases {
                    self.visit_expr(e)?;
                    self.visit_stmt(arm)?;
                }
                if let Some(d) = default {
                    self.visit_stmt(d)?;
                }
                Ok(())
            }
            StmtKind::While { cond, body } => {
                self.visit_expr(cond)?;
                for s in body {
                    self.visit_stmt(s)?;
                }
                Ok(())
            }
            StmtKind::Break | StmtKind::Continue => Ok(()),
        }
    }

    fn visit_if_stmt(&mut self, i: &IfStmt) -> Result<(), Self::Error> {
        self.visit_expr(&i.cond)?;
        for s in &i.then_body {
            self.visit_stmt(s)?;
        }
        if let Some(else_b) = &i.else_body {
            match else_b {
                ElseBody::If(inner) => self.visit_if_stmt(inner),
                ElseBody::Block(stmts) => {
                    for s in stmts {
                        self.visit_stmt(s)?;
                    }
                    Ok(())
                }
            }
        } else {
            Ok(())
        }
    }

    fn visit_expr(&mut self, expr: &Expr) -> Result<(), Self::Error> {
        match &expr.kind {
            ExprKind::Member { base, .. } => self.visit_expr(base.as_ref()),
            ExprKind::Call {
                callee,
                type_args: _,
                args,
            } => {
                self.visit_expr(callee.as_ref())?;
                for (_, a) in args {
                    self.visit_expr(a)?;
                }
                Ok(())
            }
            ExprKind::Index { base, index } => {
                self.visit_expr(base.as_ref())?;
                self.visit_expr(index.as_ref())
            }
            ExprKind::Array(elts) => {
                for x in elts {
                    self.visit_expr(x)?;
                }
                Ok(())
            }
            ExprKind::Unary { expr, .. } => self.visit_expr(expr.as_ref()),
            ExprKind::Binary { left, right, .. } => {
                self.visit_expr(left.as_ref())?;
                self.visit_expr(right.as_ref())
            }
            ExprKind::Ternary {
                cond,
                then_b,
                else_b,
            } => {
                self.visit_expr(cond.as_ref())?;
                self.visit_expr(then_b.as_ref())?;
                self.visit_expr(else_b.as_ref())
            }
            ExprKind::IfExpr {
                cond,
                then_b,
                else_b,
            } => {
                self.visit_expr(cond.as_ref())?;
                self.visit_expr(then_b.as_ref())?;
                self.visit_expr(else_b.as_ref())
            }
            ExprKind::Int(_)
            | ExprKind::Float(_)
            | ExprKind::String(_)
            | ExprKind::Bool(_)
            | ExprKind::Na
            | ExprKind::Color(_)
            | ExprKind::HexColor(_)
            | ExprKind::IdentPath(_) => Ok(()),
        }
    }
}
