//! Assign dense [`super::node::NodeId`] values to every expression and statement after parsing.

use super::decl::{EnumDef, ExportDecl, FnBody, FnDecl, Item, Script, ScriptDeclaration, UserTypeDef};
use super::expr::{Expr, ExprKind};
use super::node::NodeId;
use super::stmt::{ElseBody, IfStmt, Stmt, StmtKind};

/// Assign `NodeId` values starting at 1; leaves `NodeId::UNASSIGNED` if the tree is empty.
pub fn assign_node_ids(script: &mut Script) {
    let mut next: u32 = 1;
    for item in &mut script.items {
        assign_item(item, &mut next);
    }
}

/// Set every [`NodeId`] in `f` (params, defaults, body) to [`NodeId::UNASSIGNED`] so the clone can
/// be lowered in a consumer session without colliding with consumer side maps.
pub fn clear_node_ids_in_fn_decl(f: &mut FnDecl) {
    for p in &mut f.params {
        if let Some(d) = &mut p.default {
            clear_expr(d);
        }
    }
    match &mut f.body {
        FnBody::Expr(e) => clear_expr(e),
        FnBody::Block(stmts) => {
            for s in stmts {
                clear_stmt(s);
            }
        }
    }
}

fn clear_stmt(s: &mut Stmt) {
    s.id = NodeId::UNASSIGNED;
    match &mut s.kind {
        StmtKind::VarDecl(v) => clear_expr(&mut v.value),
        StmtKind::Assign { value, .. } | StmtKind::TupleAssign { value, .. } => clear_expr(value),
        StmtKind::Expr(e) => clear_expr(e),
        StmtKind::Block(stmts) => {
            for x in stmts {
                clear_stmt(x);
            }
        }
        StmtKind::If(i) => clear_if(i),
        StmtKind::For {
            from,
            to,
            by,
            body,
            ..
        } => {
            clear_expr(from);
            clear_expr(to);
            if let Some(b) = by {
                clear_expr(b);
            }
            for x in body {
                clear_stmt(x);
            }
        }
        StmtKind::ForIn { iterable, body, .. } => {
            clear_expr(iterable);
            for x in body {
                clear_stmt(x);
            }
        }
        StmtKind::Switch {
            scrutinee,
            cases,
            default,
        } => {
            if let Some(s) = scrutinee {
                clear_expr(s);
            }
            for (e, arm) in cases {
                clear_expr(e);
                clear_stmt(arm);
            }
            if let Some(d) = default {
                clear_stmt(d.as_mut());
            }
        }
        StmtKind::While { cond, body } => {
            clear_expr(cond);
            for x in body {
                clear_stmt(x);
            }
        }
        StmtKind::Break | StmtKind::Continue => {}
    }
}

fn clear_if(i: &mut IfStmt) {
    clear_expr(&mut i.cond);
    for x in &mut i.then_body {
        clear_stmt(x);
    }
    if let Some(else_b) = &mut i.else_body {
        match else_b {
            ElseBody::If(inner) => clear_if(inner),
            ElseBody::Block(stmts) => {
                for x in stmts.iter_mut() {
                    clear_stmt(x);
                }
            }
        }
    }
}

fn clear_expr(e: &mut Expr) {
    e.id = NodeId::UNASSIGNED;
    match &mut e.kind {
        ExprKind::Int(_)
        | ExprKind::Float(_)
        | ExprKind::String(_)
        | ExprKind::Bool(_)
        | ExprKind::Na
        | ExprKind::Color(_)
        | ExprKind::HexColor(_)
        | ExprKind::IdentPath(_) => {}
        ExprKind::Member { base, .. } => clear_expr(base.as_mut()),
        ExprKind::Call {
            callee,
            type_args: _,
            args,
        } => {
            clear_expr(callee.as_mut());
            for (_, a) in args {
                clear_expr(a);
            }
        }
        ExprKind::Index { base, index } => {
            clear_expr(base.as_mut());
            clear_expr(index.as_mut());
        }
        ExprKind::Array(elts) => {
            for x in elts {
                clear_expr(x);
            }
        }
        ExprKind::Unary { expr, .. } => clear_expr(expr.as_mut()),
        ExprKind::Binary { left, right, .. } => {
            clear_expr(left.as_mut());
            clear_expr(right.as_mut());
        }
        ExprKind::Ternary {
            cond,
            then_b,
            else_b,
        } => {
            clear_expr(cond.as_mut());
            clear_expr(then_b.as_mut());
            clear_expr(else_b.as_mut());
        }
        ExprKind::IfExpr {
            cond,
            then_b,
            else_b,
        } => {
            clear_expr(cond.as_mut());
            clear_expr(then_b.as_mut());
            clear_expr(else_b.as_mut());
        }
    }
}

/// Largest [`NodeId`] present in the tree (0 if none assigned).
#[must_use]
pub fn max_node_id(script: &Script) -> u32 {
    let mut m = 0u32;
    for item in &script.items {
        max_item(item, &mut m);
    }
    m
}

fn assign_item(item: &mut Item, next: &mut u32) {
    match item {
        Item::Import(i) => {
            i.id = NodeId(*next);
            *next += 1;
        }
        Item::Export(e) => match e {
            ExportDecl::Fn(f) => assign_fn(f, next),
            ExportDecl::Var(v) => {
                assign_expr(&mut v.value, next);
            }
            ExportDecl::Enum(e) => assign_enum(e, next),
            ExportDecl::TypeDef(t) => assign_udt(t, next),
        },
        Item::ScriptDecl(ScriptDeclaration { args, .. }) => {
            for (_, e) in args {
                assign_expr(e, next);
            }
        }
        Item::FnDecl(f) => assign_fn(f, next),
        Item::Enum(e) => assign_enum(e, next),
        Item::TypeDef(t) => assign_udt(t, next),
        Item::Stmt(s) => assign_stmt(s, next),
    }
}

fn max_item(item: &Item, m: &mut u32) {
    match item {
        Item::Import(i) => {
            *m = (*m).max(i.id.0);
        }
        Item::Export(e) => match e {
            ExportDecl::Fn(f) => max_fn(f, m),
            ExportDecl::Var(v) => max_expr(&v.value, m),
            ExportDecl::Enum(e) => max_enum(e, m),
            ExportDecl::TypeDef(t) => max_udt(t, m),
        },
        Item::ScriptDecl(ScriptDeclaration { args, .. }) => {
            for (_, e) in args {
                max_expr(e, m);
            }
        }
        Item::FnDecl(f) => max_fn(f, m),
        Item::Enum(e) => max_enum(e, m),
        Item::TypeDef(t) => max_udt(t, m),
        Item::Stmt(s) => max_stmt(s, m),
    }
}

fn assign_fn(f: &mut FnDecl, next: &mut u32) {
    for p in &mut f.params {
        if let Some(d) = &mut p.default {
            assign_expr(d, next);
        }
    }
    match &mut f.body {
        FnBody::Expr(e) => assign_expr(e, next),
        FnBody::Block(stmts) => {
            for s in stmts {
                assign_stmt(s, next);
            }
        }
    }
}

fn max_fn(f: &FnDecl, m: &mut u32) {
    for p in &f.params {
        if let Some(d) = &p.default {
            max_expr(d, m);
        }
    }
    match &f.body {
        FnBody::Expr(e) => max_expr(e, m),
        FnBody::Block(stmts) => {
            for s in stmts {
                max_stmt(s, m);
            }
        }
    }
}

fn assign_enum(e: &mut EnumDef, next: &mut u32) {
    for v in &mut e.variants {
        assign_expr(&mut v.value, next);
    }
}

fn max_enum(e: &EnumDef, m: &mut u32) {
    for v in &e.variants {
        max_expr(&v.value, m);
    }
}

fn assign_udt(t: &mut UserTypeDef, next: &mut u32) {
    for f in &mut t.fields {
        assign_expr(&mut f.default, next);
    }
}

fn max_udt(t: &UserTypeDef, m: &mut u32) {
    for f in &t.fields {
        max_expr(&f.default, m);
    }
}

fn assign_stmt(s: &mut Stmt, next: &mut u32) {
    s.id = NodeId(*next);
    *next += 1;
    match &mut s.kind {
        StmtKind::VarDecl(v) => {
            assign_expr(&mut v.value, next);
        }
        StmtKind::Assign { value, .. } | StmtKind::TupleAssign { value, .. } => {
            assign_expr(value, next);
        }
        StmtKind::Expr(e) => assign_expr(e, next),
        StmtKind::Block(stmts) => {
            for x in stmts {
                assign_stmt(x, next);
            }
        }
        StmtKind::If(i) => assign_if(i, next),
        StmtKind::For {
            from,
            to,
            by,
            body,
            ..
        } => {
            assign_expr(from, next);
            assign_expr(to, next);
            if let Some(b) = by {
                assign_expr(b, next);
            }
            for x in body {
                assign_stmt(x, next);
            }
        }
        StmtKind::ForIn { iterable, body, .. } => {
            assign_expr(iterable, next);
            for x in body {
                assign_stmt(x, next);
            }
        }
        StmtKind::Switch {
            scrutinee,
            cases,
            default,
        } => {
            if let Some(s) = scrutinee {
                assign_expr(s, next);
            }
            for (e, arm) in cases {
                assign_expr(e, next);
                assign_stmt(arm, next);
            }
            if let Some(d) = default {
                assign_stmt(d.as_mut(), next);
            }
        }
        StmtKind::While { cond, body } => {
            assign_expr(cond, next);
            for x in body {
                assign_stmt(x, next);
            }
        }
        StmtKind::Break | StmtKind::Continue => {}
    }
}

fn max_stmt(s: &Stmt, m: &mut u32) {
    *m = (*m).max(s.id.0);
    match &s.kind {
        StmtKind::VarDecl(v) => max_expr(&v.value, m),
        StmtKind::Assign { value, .. } | StmtKind::TupleAssign { value, .. } => max_expr(value, m),
        StmtKind::Expr(e) => max_expr(e, m),
        StmtKind::Block(stmts) => {
            for x in stmts {
                max_stmt(x, m);
            }
        }
        StmtKind::If(i) => max_if(i, m),
        StmtKind::For {
            from, to, by, body, ..
        } => {
            max_expr(from, m);
            max_expr(to, m);
            if let Some(b) = by {
                max_expr(b, m);
            }
            for x in body {
                max_stmt(x, m);
            }
        }
        StmtKind::ForIn { iterable, body, .. } => {
            max_expr(iterable, m);
            for x in body {
                max_stmt(x, m);
            }
        }
        StmtKind::Switch {
            scrutinee,
            cases,
            default,
        } => {
            if let Some(s) = scrutinee {
                max_expr(s, m);
            }
            for (e, arm) in cases {
                max_expr(e, m);
                max_stmt(arm, m);
            }
            if let Some(d) = default {
                max_stmt(d, m);
            }
        }
        StmtKind::While { cond, body } => {
            max_expr(cond, m);
            for x in body {
                max_stmt(x, m);
            }
        }
        StmtKind::Break | StmtKind::Continue => {}
    }
}

fn assign_if(i: &mut IfStmt, next: &mut u32) {
    assign_expr(&mut i.cond, next);
    for x in &mut i.then_body {
        assign_stmt(x, next);
    }
    if let Some(else_b) = &mut i.else_body {
        match else_b {
            ElseBody::If(inner) => assign_if(inner, next),
            ElseBody::Block(stmts) => {
                for x in stmts.iter_mut() {
                    assign_stmt(x, next);
                }
            }
        }
    }
}

fn max_if(i: &IfStmt, m: &mut u32) {
    max_expr(&i.cond, m);
    for x in &i.then_body {
        max_stmt(x, m);
    }
    if let Some(else_b) = &i.else_body {
        match else_b {
            ElseBody::If(inner) => max_if(inner, m),
            ElseBody::Block(stmts) => {
                for x in stmts.iter() {
                    max_stmt(x, m);
                }
            }
        }
    }
}

fn assign_expr(e: &mut Expr, next: &mut u32) {
    e.id = NodeId(*next);
    *next += 1;
    match &mut e.kind {
        ExprKind::Int(_)
        | ExprKind::Float(_)
        | ExprKind::String(_)
        | ExprKind::Bool(_)
        | ExprKind::Na
        | ExprKind::Color(_)
        | ExprKind::HexColor(_)
        | ExprKind::IdentPath(_) => {}
        ExprKind::Member { base, .. } => assign_expr(base.as_mut(), next),
        ExprKind::Call {
            callee,
            type_args: _,
            args,
        } => {
            assign_expr(callee.as_mut(), next);
            for (_, a) in args {
                assign_expr(a, next);
            }
        }
        ExprKind::Index { base, index } => {
            assign_expr(base.as_mut(), next);
            assign_expr(index.as_mut(), next);
        }
        ExprKind::Array(elts) => {
            for x in elts {
                assign_expr(x, next);
            }
        }
        ExprKind::Unary { expr, .. } => assign_expr(expr.as_mut(), next),
        ExprKind::Binary { left, right, .. } => {
            assign_expr(left.as_mut(), next);
            assign_expr(right.as_mut(), next);
        }
        ExprKind::Ternary {
            cond,
            then_b,
            else_b,
        } => {
            assign_expr(cond.as_mut(), next);
            assign_expr(then_b.as_mut(), next);
            assign_expr(else_b.as_mut(), next);
        }
        ExprKind::IfExpr {
            cond,
            then_b,
            else_b,
        } => {
            assign_expr(cond.as_mut(), next);
            assign_expr(then_b.as_mut(), next);
            assign_expr(else_b.as_mut(), next);
        }
    }
}

fn max_expr(e: &Expr, m: &mut u32) {
    *m = (*m).max(e.id.0);
    match &e.kind {
        ExprKind::Int(_)
        | ExprKind::Float(_)
        | ExprKind::String(_)
        | ExprKind::Bool(_)
        | ExprKind::Na
        | ExprKind::Color(_)
        | ExprKind::HexColor(_)
        | ExprKind::IdentPath(_) => {}
        ExprKind::Member { base, .. } => max_expr(base, m),
        ExprKind::Call {
            callee,
            type_args: _,
            args,
        } => {
            max_expr(callee, m);
            for (_, a) in args {
                max_expr(a, m);
            }
        }
        ExprKind::Index { base, index } => {
            max_expr(base, m);
            max_expr(index, m);
        }
        ExprKind::Array(elts) => {
            for x in elts {
                max_expr(x, m);
            }
        }
        ExprKind::Unary { expr, .. } => max_expr(expr, m),
        ExprKind::Binary { left, right, .. } => {
            max_expr(left, m);
            max_expr(right, m);
        }
        ExprKind::Ternary {
            cond,
            then_b,
            else_b,
        } => {
            max_expr(cond, m);
            max_expr(then_b, m);
            max_expr(else_b, m);
        }
        ExprKind::IfExpr {
            cond,
            then_b,
            else_b,
        } => {
            max_expr(cond, m);
            max_expr(then_b, m);
            max_expr(else_b, m);
        }
    }
}
