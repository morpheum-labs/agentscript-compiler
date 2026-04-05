//! Lexical name resolution: unqualified identifiers must bind to locals, hoisted top-level
//! functions, import aliases, or known single-segment builtins.
//!
//! Walk order matches the minimal typechecker (implicit declaration via first `=` assign).

use std::collections::HashSet;

use crate::frontend::ast::{
    ElseBody, ExportDecl, Expr, ExprKind, FnBody, FnDecl, IfStmt, Item, Script, ScriptDeclaration,
    Stmt, StmtKind, Type,
};

use super::super::builtins::is_unqualified_builtin_ident;
use super::super::AnalyzeError;

struct LexicalCtx {
    /// Innermost scope last. Hoisted names live in the root frame.
    scopes: Vec<HashSet<String>>,
    issues: Vec<String>,
}

impl LexicalCtx {
    fn new(script: &Script) -> Self {
        let mut root = HashSet::new();
        for item in &script.items {
            if let Item::Import(i) = item {
                root.insert(i.alias.clone());
            }
            match item {
                Item::FnDecl(f) | Item::Export(ExportDecl::Fn(f)) => {
                    root.insert(f.name.clone());
                }
                _ => {}
            }
        }
        Self {
            scopes: vec![root],
            issues: Vec::new(),
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashSet::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn name_in_any_scope(&self, name: &str) -> bool {
        self.scopes.iter().rev().any(|s| s.contains(name))
    }

    /// Pine-style first assignment introduces `name` in the innermost scope.
    fn define_implicit(&mut self, name: &str) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string());
        }
    }

    fn define_var_decl(&mut self, name: &str, ctx: &str) {
        if let Some(scope) = self.scopes.last_mut() {
            if !scope.insert(name.to_string()) {
                self.issues
                    .push(format!("duplicate declaration of `{name}` ({ctx})"));
            }
        }
    }

    fn define_param(&mut self, name: &str) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string());
        }
    }

    fn resolve_ident(&mut self, name: &str, context: &str) {
        if self.name_in_any_scope(name) {
            return;
        }
        if is_unqualified_builtin_ident(name) {
            return;
        }
        self.issues.push(format!(
            "unknown identifier `{name}` ({context})"
        ));
    }

    fn walk_expr(&mut self, e: &Expr, context: &str) {
        match &e.kind {
            ExprKind::IdentPath(p) => {
                if p.len() == 1 {
                    self.resolve_ident(&p[0], context);
                }
            }
            ExprKind::Member { base, .. } => self.walk_expr(base, context),
            ExprKind::Call {
                callee,
                type_args,
                args,
            } => {
                self.walk_callee(callee.as_ref(), context);
                if let Some(ta) = type_args {
                    for t in ta {
                        self.walk_type(t, context);
                    }
                }
                for (_, a) in args {
                    self.walk_expr(a, context);
                }
            }
            ExprKind::Index { base, index } => {
                self.walk_expr(base, context);
                self.walk_expr(index, context);
            }
            ExprKind::Array(elts) => {
                for x in elts {
                    self.walk_expr(x, context);
                }
            }
            ExprKind::Unary { expr, .. } => self.walk_expr(expr, context),
            ExprKind::Binary { left, right, .. } => {
                self.walk_expr(left, context);
                self.walk_expr(right, context);
            }
            ExprKind::Ternary {
                cond,
                then_b,
                else_b,
            } => {
                self.walk_expr(cond, context);
                self.walk_expr(then_b, context);
                self.walk_expr(else_b, context);
            }
            ExprKind::IfExpr {
                cond,
                then_b,
                else_b,
            } => {
                self.walk_expr(cond, context);
                self.walk_expr(then_b, context);
                self.walk_expr(else_b, context);
            }
            ExprKind::Int(_)
            | ExprKind::Float(_)
            | ExprKind::String(_)
            | ExprKind::Bool(_)
            | ExprKind::Na
            | ExprKind::Color(_)
            | ExprKind::HexColor(_) => {}
        }
    }

    fn walk_callee(&mut self, e: &Expr, context: &str) {
        match &e.kind {
            ExprKind::IdentPath(p) if p.len() == 1 => {
                self.resolve_ident(&p[0], context);
            }
            ExprKind::Member { base, .. } => self.walk_expr(base, context),
            _ => self.walk_expr(e, context),
        }
    }

    fn walk_type(&mut self, t: &Type, context: &str) {
        match t {
            Type::Primitive(_) | Type::Named(_) => {}
            Type::Array(inner) | Type::Matrix(inner) => self.walk_type(inner, context),
            Type::Map(a, b) => {
                self.walk_type(a, context);
                self.walk_type(b, context);
            }
            Type::Label
            | Type::Line
            | Type::BoxType
            | Type::Table
            | Type::Polyline
            | Type::Linefill
            | Type::ChartPoint
            | Type::VolumeRow => {}
        }
    }

    fn walk_stmt(&mut self, s: &Stmt, context: &str) {
        match &s.kind {
            StmtKind::VarDecl(v) => {
                if let Some(ty) = &v.ty {
                    self.walk_type(ty, context);
                }
                self.walk_expr(&v.value, context);
                self.define_var_decl(&v.name, context);
            }
            StmtKind::Assign { name, value, .. } => {
                self.walk_expr(value, context);
                if !self.name_in_any_scope(name) {
                    self.define_implicit(name);
                }
            }
            StmtKind::TupleAssign { names, value, .. } => {
                self.walk_expr(value, context);
                for n in names {
                    if !self.name_in_any_scope(n) {
                        self.define_implicit(n);
                    }
                }
            }
            StmtKind::Expr(e) => self.walk_expr(e, context),
            StmtKind::Block(stmts) => {
                self.push_scope();
                for x in stmts {
                    self.walk_stmt(x, context);
                }
                self.pop_scope();
            }
            StmtKind::If(i) => self.walk_if(i, context),
            StmtKind::For {
                var,
                from,
                to,
                by,
                body,
            } => {
                self.walk_expr(from, context);
                self.walk_expr(to, context);
                if let Some(b) = by {
                    self.walk_expr(b, context);
                }
                self.push_scope();
                self.define_param(var);
                for x in body {
                    self.walk_stmt(x, context);
                }
                self.pop_scope();
            }
            StmtKind::ForIn { pattern, iterable, body } => {
                self.walk_expr(iterable, context);
                self.push_scope();
                match pattern {
                    crate::frontend::ast::ForInPattern::Name(n) => self.define_param(n),
                    crate::frontend::ast::ForInPattern::Pair(i, v) => {
                        self.define_param(i);
                        self.define_param(v);
                    }
                }
                for x in body {
                    self.walk_stmt(x, context);
                }
                self.pop_scope();
            }
            StmtKind::Switch {
                scrutinee,
                cases,
                default,
            } => {
                if let Some(s) = scrutinee {
                    self.walk_expr(s, context);
                }
                for (e, arm) in cases {
                    self.walk_expr(e, context);
                    self.walk_stmt(arm, context);
                }
                if let Some(d) = default {
                    self.walk_stmt(d, context);
                }
            }
            StmtKind::While { cond, body } => {
                self.walk_expr(cond, context);
                self.push_scope();
                for x in body {
                    self.walk_stmt(x, context);
                }
                self.pop_scope();
            }
            StmtKind::Break | StmtKind::Continue => {}
        }
    }

    fn walk_if(&mut self, i: &IfStmt, context: &str) {
        self.walk_expr(&i.cond, context);
        self.push_scope();
        for x in &i.then_body {
            self.walk_stmt(x, context);
        }
        self.pop_scope();
        if let Some(else_b) = &i.else_body {
            match else_b {
                ElseBody::If(inner) => self.walk_if(inner, context),
                ElseBody::Block(stmts) => {
                    self.push_scope();
                    for x in stmts {
                        self.walk_stmt(x, context);
                    }
                    self.pop_scope();
                }
            }
        }
    }

    fn walk_fn(&mut self, f: &FnDecl) {
        let ctx = format!("function `{}`", f.name);
        for p in &f.params {
            if let Some(ty) = &p.ty {
                self.walk_type(ty, &ctx);
            }
            if let Some(d) = &p.default {
                self.walk_expr(d, &ctx);
            }
        }
        self.push_scope();
        for p in &f.params {
            self.define_param(&p.name);
        }
        match &f.body {
            FnBody::Expr(e) => self.walk_expr(e, &ctx),
            FnBody::Block(stmts) => {
                for s in stmts {
                    self.walk_stmt(s, &ctx);
                }
            }
        }
        self.pop_scope();
    }

    fn walk_item(&mut self, item: &Item) {
        match item {
            Item::ScriptDecl(ScriptDeclaration { args, .. }) => {
                for (_, e) in args {
                    self.walk_expr(e, "script declaration");
                }
            }
            Item::Stmt(s) => self.walk_stmt(s, "top-level"),
            Item::FnDecl(f) | Item::Export(ExportDecl::Fn(f)) => self.walk_fn(f),
            Item::Export(ExportDecl::Var(v)) => {
                if let Some(ty) = &v.ty {
                    self.walk_type(ty, "export var");
                }
                self.walk_expr(&v.value, "export var");
                self.define_var_decl(&v.name, "export var");
            }
            Item::Export(ExportDecl::Enum(e)) | Item::Enum(e) => {
                for v in &e.variants {
                    self.walk_expr(&v.value, "enum variant");
                }
            }
            Item::Export(ExportDecl::TypeDef(t)) | Item::TypeDef(t) => {
                for f in &t.fields {
                    self.walk_type(&f.ty, "UDT field");
                    self.walk_expr(&f.default, "UDT field default");
                }
            }
            Item::Import(_) => {}
        }
    }
}

/// Resolve unqualified identifiers; reject unknown locals (Phase 1 lexical groundwork).
pub fn lexical_resolve_script(script: &Script) -> Result<(), AnalyzeError> {
    let mut c = LexicalCtx::new(script);
    for item in &script.items {
        c.walk_item(item);
    }
    if c.issues.is_empty() {
        Ok(())
    } else {
        Err(AnalyzeError {
            message: c.issues.join("\n"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_script;

    #[test]
    fn unknown_identifier_rejected() {
        let s = parse_script(
            "t.pine",
            "indicator(\"x\")\ny = nope + 1\n",
        )
        .unwrap();
        let e = lexical_resolve_script(&s).unwrap_err();
        assert!(e.message.contains("nope"), "{}", e.message);
    }

    #[test]
    fn builtin_close_ok() {
        let s = parse_script(
            "t.pine",
            "indicator(\"x\")\ny = close + 1\n",
        )
        .unwrap();
        lexical_resolve_script(&s).unwrap();
    }

    #[test]
    fn hoisted_fn_call_before_decl_ok() {
        let s = parse_script(
            "t.pine",
            "indicator(\"x\")\na = inc(1)\nf inc(x) => x + 1\n",
        )
        .unwrap();
        lexical_resolve_script(&s).unwrap();
    }

    #[test]
    fn block_inner_uses_outer_implicit_decl() {
        let s = parse_script(
            "t.pine",
            "indicator(\"x\")\na = 1\n{\n  b = a + 1\n}\n",
        )
        .unwrap();
        lexical_resolve_script(&s).unwrap();
    }

    #[test]
    fn duplicate_var_decl_same_scope_rejected() {
        let s = parse_script(
            "t.pine",
            "indicator(\"x\")\nfloat a = 1.0\nfloat a = 2.0\n",
        )
        .unwrap();
        let e = lexical_resolve_script(&s).unwrap_err();
        assert!(e.message.contains("duplicate"), "{}", e.message);
        assert!(e.message.contains('a'), "{}", e.message);
    }

    #[test]
    fn import_alias_as_value_ok() {
        let s = parse_script(
            "t.pine",
            "import User/Lib/1 as m\nindicator(\"x\")\ny = m\n",
        )
        .unwrap();
        lexical_resolve_script(&s).unwrap();
    }
}
