//! AST walking: composable [`VisitExpr`] / [`VisitStmt`] hooks plus [`AstWalk`] recursion.
//!
//! The error type parameter `E` defaults to `()` so simple visitors avoid spelling it. Loop bodies
//! call [`AstWalk::push_loop_frame`] / [`AstWalk::pop_loop_frame`] so passes such as `break` /
//! `continue` validation can track nesting without duplicating the statement `match`.

use crate::frontend::ast::{
    ElseBody, Expr, ExprKind, ExportDecl, FnBody, FnDecl, IfStmt, Item, Script, Stmt, StmtKind,
};

/// Per-expression hook (default no-op). Invoked at each node before children in [`AstWalk::walk_expr`].
pub trait VisitExpr<E = ()> {
    fn visit_expr(&mut self, _expr: &Expr) -> Result<(), E> {
        Ok(())
    }
}

/// Per-statement hook (default no-op). Invoked at each node before children in [`AstWalk::walk_stmt`].
pub trait VisitStmt<E = ()> {
    fn visit_stmt(&mut self, _stmt: &Stmt) -> Result<(), E> {
        Ok(())
    }
}

/// Recursive pre-order walk; override [`VisitExpr`] / [`VisitStmt`] and/or loop-frame hooks only.
pub trait AstWalk<E = ()>: VisitExpr<E> + VisitStmt<E> {
    /// Called when entering the body of `for` / `for … in` / `while`.
    fn push_loop_frame(&mut self) {}

    /// Called after walking a loop body.
    fn pop_loop_frame(&mut self) {}

    fn walk_script(&mut self, script: &Script) -> Result<(), E> {
        for item in &script.items {
            self.walk_item(item)?;
        }
        Ok(())
    }

    fn walk_item(&mut self, item: &Item) -> Result<(), E> {
        match item {
            Item::Stmt(s) => self.walk_stmt(s),
            Item::FnDecl(f) => self.walk_fn_decl(f),
            Item::Export(ExportDecl::Fn(f)) => self.walk_fn_decl(f),
            Item::ScriptDecl(d) => {
                for (_, e) in &d.args {
                    self.walk_expr(e)?;
                }
                Ok(())
            }
            Item::Enum(e) | Item::Export(ExportDecl::Enum(e)) => {
                for v in &e.variants {
                    self.walk_expr(&v.value)?;
                }
                Ok(())
            }
            Item::TypeDef(t) | Item::Export(ExportDecl::TypeDef(t)) => {
                for field in &t.fields {
                    self.walk_expr(&field.default)?;
                }
                Ok(())
            }
            Item::Export(ExportDecl::Var(v)) => {
                self.walk_expr(&v.value)?;
                Ok(())
            }
            Item::Import(_) => Ok(()),
        }
    }

    fn walk_fn_decl(&mut self, f: &FnDecl) -> Result<(), E> {
        for p in &f.params {
            if let Some(d) = &p.default {
                self.walk_expr(d)?;
            }
        }
        match &f.body {
            FnBody::Expr(e) => self.walk_expr(e),
            FnBody::Block(stmts) => {
                for s in stmts {
                    self.walk_stmt(s)?;
                }
                Ok(())
            }
        }
    }

    fn walk_stmt(&mut self, stmt: &Stmt) -> Result<(), E> {
        self.visit_stmt(stmt)?;
        match &stmt.kind {
            StmtKind::VarDecl(v) => self.walk_expr(&v.value),
            StmtKind::Assign { value, .. } | StmtKind::TupleAssign { value, .. } => {
                self.walk_expr(value)
            }
            StmtKind::Expr(e) => self.walk_expr(e),
            StmtKind::Block(stmts) => {
                for s in stmts {
                    self.walk_stmt(s)?;
                }
                Ok(())
            }
            StmtKind::If(i) => self.walk_if_stmt(i),
            StmtKind::For {
                from,
                to,
                by,
                body,
                ..
            } => {
                self.walk_expr(from)?;
                self.walk_expr(to)?;
                if let Some(b) = by {
                    self.walk_expr(b)?;
                }
                self.push_loop_frame();
                for s in body {
                    self.walk_stmt(s)?;
                }
                self.pop_loop_frame();
                Ok(())
            }
            StmtKind::ForIn { iterable, body, .. } => {
                self.walk_expr(iterable)?;
                self.push_loop_frame();
                for s in body {
                    self.walk_stmt(s)?;
                }
                self.pop_loop_frame();
                Ok(())
            }
            StmtKind::Switch {
                scrutinee,
                cases,
                default,
            } => {
                if let Some(s) = scrutinee {
                    self.walk_expr(s)?;
                }
                for (e, arm) in cases {
                    self.walk_expr(e)?;
                    self.walk_stmt(arm)?;
                }
                if let Some(d) = default {
                    self.walk_stmt(d)?;
                }
                Ok(())
            }
            StmtKind::While { cond, body } => {
                self.walk_expr(cond)?;
                self.push_loop_frame();
                for s in body {
                    self.walk_stmt(s)?;
                }
                self.pop_loop_frame();
                Ok(())
            }
            StmtKind::Break | StmtKind::Continue => Ok(()),
        }
    }

    fn walk_if_stmt(&mut self, i: &IfStmt) -> Result<(), E> {
        self.walk_expr(&i.cond)?;
        for s in &i.then_body {
            self.walk_stmt(s)?;
        }
        if let Some(else_b) = &i.else_body {
            match else_b {
                ElseBody::If(inner) => self.walk_if_stmt(inner),
                ElseBody::Block(stmts) => {
                    for s in stmts {
                        self.walk_stmt(s)?;
                    }
                    Ok(())
                }
            }
        } else {
            Ok(())
        }
    }

    fn walk_expr(&mut self, expr: &Expr) -> Result<(), E> {
        self.visit_expr(expr)?;
        match &expr.kind {
            ExprKind::Member { base, .. } => self.walk_expr(base.as_ref()),
            ExprKind::Call {
                callee,
                type_args: _,
                args,
            } => {
                self.walk_expr(callee.as_ref())?;
                for (_, a) in args {
                    self.walk_expr(a)?;
                }
                Ok(())
            }
            ExprKind::Index { base, index } => {
                self.walk_expr(base.as_ref())?;
                self.walk_expr(index.as_ref())
            }
            ExprKind::Array(elts) => {
                for x in elts {
                    self.walk_expr(x)?;
                }
                Ok(())
            }
            ExprKind::Unary { expr, .. } => self.walk_expr(expr.as_ref()),
            ExprKind::Binary { left, right, .. } => {
                self.walk_expr(left.as_ref())?;
                self.walk_expr(right.as_ref())
            }
            ExprKind::Ternary {
                cond,
                then_b,
                else_b,
            } => {
                self.walk_expr(cond.as_ref())?;
                self.walk_expr(then_b.as_ref())?;
                self.walk_expr(else_b.as_ref())
            }
            ExprKind::IfExpr {
                cond,
                then_b,
                else_b,
            } => {
                self.walk_expr(cond.as_ref())?;
                self.walk_expr(then_b.as_ref())?;
                self.walk_expr(else_b.as_ref())
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

/// `visit_*` entry points delegate to [`AstWalk`]; hooks live on [`VisitExpr`] / [`VisitStmt`].
pub trait AstVisitor<E = ()>: AstWalk<E> {
    fn visit_script(&mut self, script: &Script) -> Result<(), E> {
        self.walk_script(script)
    }

    fn visit_item(&mut self, item: &Item) -> Result<(), E> {
        self.walk_item(item)
    }

    fn visit_fn_decl(&mut self, f: &FnDecl) -> Result<(), E> {
        self.walk_fn_decl(f)
    }

    fn visit_stmt(&mut self, stmt: &Stmt) -> Result<(), E> {
        self.walk_stmt(stmt)
    }

    fn visit_if_stmt(&mut self, i: &IfStmt) -> Result<(), E> {
        self.walk_if_stmt(i)
    }

    fn visit_expr(&mut self, expr: &Expr) -> Result<(), E> {
        self.walk_expr(expr)
    }
}

impl<E, T: AstWalk<E>> AstVisitor<E> for T {}
