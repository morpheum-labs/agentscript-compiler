//! Path resolution: unknown `foo.bar` roots, and `strategy.*` only in strategy scripts.

use std::collections::HashSet;

use crate::bindings::NameBinding;
use crate::frontend::ast::{
    ElseBody, Expr, ExprKind, ExportDecl, FnBody, FnDecl, IfStmt, Item, NodeId, Script,
    ScriptDeclaration, ScriptKind, Stmt, StmtKind, Type,
};

use crate::frontend::ast::Span;
use crate::session::CompilerSession;

use super::super::builtins::builtin_namespace_roots;
use super::super::{AnalyzeError, SemanticDiagnostic};

struct ResolveCtx<'a> {
    script_kind: Option<ScriptKind>,
    import_aliases: HashSet<String>,
    user_type_roots: HashSet<String>,
    builtins: HashSet<&'static str>,
    issues: Vec<SemanticDiagnostic>,
    session: &'a mut CompilerSession,
}

impl<'a> ResolveCtx<'a> {
    fn new(script: &Script, session: &'a mut CompilerSession) -> Self {
        let mut import_aliases = HashSet::new();
        let mut user_type_roots = HashSet::new();
        for item in &script.items {
            if let Item::Import(i) = item {
                import_aliases.insert(i.alias.clone());
            }
            match item {
                Item::Enum(e) | Item::Export(ExportDecl::Enum(e)) => {
                    user_type_roots.insert(e.name.clone());
                }
                Item::TypeDef(t) | Item::Export(ExportDecl::TypeDef(t)) => {
                    user_type_roots.insert(t.name.clone());
                }
                _ => {}
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
            user_type_roots,
            builtins: builtin_namespace_roots(),
            issues: Vec::new(),
            session,
        }
    }

    fn root_ok(&self, root: &str) -> bool {
        self.builtins.contains(root)
            || self.import_aliases.contains(root)
            || self.user_type_roots.contains(root)
    }

    fn check_ident_path(&mut self, path: &[String], context: &str, span: Span, expr_id: NodeId) {
        if path.len() < 2 {
            return;
        }
        let root = path[0].as_str();
        let mut strategy_mismatch = false;
        if root == "strategy" && self.script_kind != Some(ScriptKind::Strategy) {
            strategy_mismatch = true;
            self.issues.push(SemanticDiagnostic {
                message: format!(
                    "`strategy.*` is only valid in `strategy()` scripts ({context})"
                ),
                span,
            });
        }
        if !self.root_ok(root) {
            self.issues.push(SemanticDiagnostic {
                message: format!(
                    "unknown namespace or import alias `{root}` in `{path}` ({context})",
                    path = path.join(".")
                ),
                span,
            });
            return;
        }
        if !strategy_mismatch && expr_id != NodeId::UNASSIGNED {
            self.session.set_name_binding(
                expr_id,
                NameBinding::QualifiedPath(path.join(".")),
            );
        }
    }

    fn walk_expr(&mut self, e: &Expr, context: &str) {
        match &e.kind {
            ExprKind::IdentPath(p) => self.check_ident_path(p, context, e.span, e.id),
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
            ExprKind::IdentPath(p) => self.check_ident_path(p, context, e.span, e.id),
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
            }
            StmtKind::Assign { value, .. } | StmtKind::TupleAssign { value, .. } => {
                self.walk_expr(value, context);
            }
            StmtKind::Expr(e) => self.walk_expr(e, context),
            StmtKind::Block(stmts) => {
                for x in stmts {
                    self.walk_stmt(x, context);
                }
            }
            StmtKind::If(i) => self.walk_if(i, context),
            StmtKind::For {
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
            StmtKind::ForIn { iterable, body, .. } => {
                self.walk_expr(iterable, context);
                for x in body {
                    self.walk_stmt(x, context);
                }
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
                for x in body {
                    self.walk_stmt(x, context);
                }
            }
            StmtKind::Break | StmtKind::Continue => {}
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
    let mut session = CompilerSession::new();
    session.prepare_analysis(script);
    resolve_script_in_session(&mut session, script)
}

/// Same as [`resolve_script`], but records [`NameBinding::QualifiedPath`] on `session` for valid paths.
pub fn resolve_script_in_session(
    session: &mut CompilerSession,
    script: &Script,
) -> Result<(), AnalyzeError> {
    let mut c = ResolveCtx::new(script, session);
    for item in &script.items {
        match item {
            Item::ScriptDecl(ScriptDeclaration { args, .. }) => {
                for (_, e) in args {
                    c.walk_expr(e, "script declaration");
                }
            }
            Item::Stmt(s) => c.walk_stmt(s, "top-level"),
            Item::FnDecl(f) => c.walk_fn(f),
            Item::Export(ExportDecl::Fn(f)) => c.walk_fn(f),
            Item::Export(ExportDecl::Var(v)) => {
                if let Some(ty) = &v.ty {
                    c.walk_type(ty, "export var");
                }
                c.walk_expr(&v.value, "export var");
            }
            Item::Export(ExportDecl::Enum(e)) | Item::Enum(e) => {
                for v in &e.variants {
                    c.walk_expr(&v.value, "enum variant");
                }
            }
            Item::Export(ExportDecl::TypeDef(t)) | Item::TypeDef(t) => {
                for f in &t.fields {
                    c.walk_type(&f.ty, "UDT field");
                    c.walk_expr(&f.default, "UDT field default");
                }
            }
            Item::Import(_) => {}
        }
    }
    if c.issues.is_empty() {
        Ok(())
    } else {
        Err(AnalyzeError::new(c.issues))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bindings::NameBinding;
    use crate::parse_script;
    use crate::Compiler;

    #[test]
    fn strategy_long_in_indicator_rejected() {
        let s = parse_script(
            "t.pine",
            "indicator(\"x\", strategy.long)\n",
        )
        .unwrap();
        let e = resolve_script(&s).unwrap_err();
        assert!(e.message().contains("strategy"));
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
        assert!(e.message().contains("not_a_real_ns"));
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

    #[test]
    fn user_enum_namespace_root_ok() {
        let s = parse_script(
            "t.pine",
            "indicator(\"x\")\nenum Side { buy = 1 }\ny = Side.buy\n",
        )
        .unwrap();
        resolve_script(&s).unwrap();
    }

    #[test]
    fn qualified_ta_sma_recorded_on_session() {
        let s = parse_script(
            "t.pine",
            "indicator(\"x\")\ny = ta.sma(close, 14)\n",
        )
        .unwrap();
        let mut c = Compiler::new();
        c.run_semantic_passes(&s).unwrap();
        assert!(
            c.session.name_bindings.iter().any(|b| {
                matches!(
                    b,
                    Some(NameBinding::QualifiedPath(p)) if p == "ta.sma"
                )
            }),
            "expected QualifiedPath(ta.sma), got {:?}",
            c.session.name_bindings
        );
    }
}
