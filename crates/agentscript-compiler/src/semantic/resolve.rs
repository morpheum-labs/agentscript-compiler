//! Path resolution: unknown `foo.bar` roots, and `strategy.*` only in strategy scripts.

use std::collections::HashSet;

use crate::ast::{
    ElseBody, Expr, FnBody, FnDecl, IfStmt, Item, Script, ScriptDeclaration, ScriptKind, Stmt,
};

use super::builtins::builtin_namespace_roots;
use super::AnalyzeError;

struct ResolveCtx {
    script_kind: Option<ScriptKind>,
    import_aliases: HashSet<String>,
    builtins: HashSet<&'static str>,
    issues: Vec<String>,
}

impl ResolveCtx {
    fn new(script: &Script) -> Self {
        let mut import_aliases = HashSet::new();
        for item in &script.items {
            if let Item::Import(i) = item {
                import_aliases.insert(i.alias.clone());
            }
        }
        Self {
            script_kind: script
                .items
                .iter()
                .find_map(|it| match it {
                    Item::ScriptDecl(ScriptDeclaration { kind, .. }) => Some(*kind),
                    _ => None,
                }),
            import_aliases,
            builtins: builtin_namespace_roots(),
            issues: Vec::new(),
        }
    }

    fn root_ok(&self, root: &str) -> bool {
        self.builtins.contains(root) || self.import_aliases.contains(root)
    }

    fn check_ident_path(&mut self, path: &[String], context: &str) {
        if path.len() < 2 {
            return;
        }
        let root = path[0].as_str();
        if root == "strategy"
            && self.script_kind != Some(ScriptKind::Strategy)
        {
            self.issues.push(format!(
                "`strategy.*` is only valid in `strategy()` scripts ({context})"
            ));
        }
        if !self.root_ok(root) {
            self.issues.push(format!(
                "unknown namespace or import alias `{root}` in `{path}` ({context})",
                path = path.join(".")
            ));
        }
    }

    fn walk_expr(&mut self, e: &Expr, context: &str) {
        match e {
            Expr::IdentPath(p) => self.check_ident_path(p, context),
            Expr::Member { base, .. } => self.walk_expr(base, context),
            Expr::Call {
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
            Expr::Index { base, index } => {
                self.walk_expr(base, context);
                self.walk_expr(index, context);
            }
            Expr::Array(elts) => {
                for x in elts {
                    self.walk_expr(x, context);
                }
            }
            Expr::Unary { expr, .. } => self.walk_expr(expr, context),
            Expr::Binary { left, right, .. } => {
                self.walk_expr(left, context);
                self.walk_expr(right, context);
            }
            Expr::Ternary {
                cond,
                then_b,
                else_b,
            } => {
                self.walk_expr(cond, context);
                self.walk_expr(then_b, context);
                self.walk_expr(else_b, context);
            }
            Expr::IfExpr {
                cond,
                then_b,
                else_b,
            } => {
                self.walk_expr(cond, context);
                self.walk_expr(then_b, context);
                self.walk_expr(else_b, context);
            }
            Expr::Int(_)
            | Expr::Float(_)
            | Expr::String(_)
            | Expr::Bool(_)
            | Expr::Na
            | Expr::Color(_)
            | Expr::HexColor(_) => {}
        }
    }

    fn walk_callee(&mut self, e: &Expr, context: &str) {
        match e {
            Expr::IdentPath(p) => self.check_ident_path(p, context),
            Expr::Member { base, .. } => self.walk_expr(base, context),
            _ => self.walk_expr(e, context),
        }
    }

    fn walk_type(&mut self, t: &crate::ast::Type, context: &str) {
        use crate::ast::Type;
        match t {
            Type::Primitive(_) => {}
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
        match s {
            Stmt::VarDecl(v) => {
                if let Some(ty) = &v.ty {
                    self.walk_type(ty, context);
                }
                self.walk_expr(&v.value, context);
            }
            Stmt::Assign { value, .. } | Stmt::TupleAssign { value, .. } => {
                self.walk_expr(value, context);
            }
            Stmt::Expr(e) => self.walk_expr(e, context),
            Stmt::Block(stmts) => {
                for x in stmts {
                    self.walk_stmt(x, context);
                }
            }
            Stmt::If(i) => self.walk_if(i, context),
            Stmt::For {
                var: _,
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
                for x in body {
                    self.walk_stmt(x, context);
                }
            }
            Stmt::ForIn { iterable, body, .. } => {
                self.walk_expr(iterable, context);
                for x in body {
                    self.walk_stmt(x, context);
                }
            }
            Stmt::Switch {
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
            Stmt::While { cond, body } => {
                self.walk_expr(cond, context);
                for x in body {
                    self.walk_stmt(x, context);
                }
            }
            Stmt::Break | Stmt::Continue => {}
        }
    }

    fn walk_if(&mut self, i: &IfStmt, context: &str) {
        self.walk_expr(&i.cond, context);
        for x in &i.then_body {
            self.walk_stmt(x, context);
        }
        if let Some(else_b) = &i.else_body {
            match else_b {
                ElseBody::If(inner) => self.walk_if(inner, context),
                ElseBody::Block(stmts) => {
                    for x in stmts {
                        self.walk_stmt(x, context);
                    }
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
        match &f.body {
            FnBody::Expr(e) => self.walk_expr(e, &ctx),
            FnBody::Block(stmts) => {
                for s in stmts {
                    self.walk_stmt(s, &ctx);
                }
            }
        }
    }
}

/// Reject unknown dotted roots and misplaced `strategy.*` (Phase 1 — no full symbol table yet).
pub fn resolve_script(script: &Script) -> Result<(), AnalyzeError> {
    let mut c = ResolveCtx::new(script);
    for item in &script.items {
        match item {
            Item::ScriptDecl(ScriptDeclaration { args, .. }) => {
                for (_, e) in args {
                    c.walk_expr(e, "script declaration");
                }
            }
            Item::Stmt(s) => c.walk_stmt(s, "top-level"),
            Item::FnDecl(f) => c.walk_fn(f),
            Item::Export(crate::ast::ExportDecl::Fn(f)) => c.walk_fn(f),
            Item::Export(crate::ast::ExportDecl::Var(v)) => {
                if let Some(ty) = &v.ty {
                    c.walk_type(ty, "export var");
                }
                c.walk_expr(&v.value, "export var");
            }
            Item::Import(_) => {}
        }
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
    fn strategy_long_in_indicator_rejected() {
        let s = parse_script(
            "t.pine",
            "indicator(\"x\", strategy.long)\n",
        )
        .unwrap();
        let e = resolve_script(&s).unwrap_err();
        assert!(e.message.contains("strategy"));
    }

    #[test]
    fn ta_sma_in_indicator_ok() {
        let s = parse_script(
            "t.pine",
            "indicator(\"x\")\ny = ta.sma(close, 14)\n",
        )
        .unwrap();
        resolve_script(&s).unwrap();
    }

    #[test]
    fn unknown_namespace_rejected() {
        let s = parse_script(
            "t.pine",
            "indicator(\"x\")\ny = not_a_real_ns.foo()\n",
        )
        .unwrap();
        let e = resolve_script(&s).unwrap_err();
        assert!(e.message.contains("not_a_real_ns"));
    }

    #[test]
    fn import_alias_as_root_ok() {
        let s = parse_script(
            "t.pine",
            "import User/Lib/1 as m\nindicator(\"x\")\ny = m.sin(1.0)\n",
        )
        .unwrap();
        resolve_script(&s).unwrap();
    }
}
