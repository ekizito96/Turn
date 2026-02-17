//! AST node definitions per spec/02-grammar.md and spec/01-minimal-core.md.

use crate::lexer::Span;

#[derive(Debug, Clone)]
pub struct Program {
    pub stmts: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Turn {
        body: Block,
        span: Span,
    },
    Let {
        name: String,
        init: Expr,
        span: Span,
    },
    ContextAppend {
        expr: Expr,
        span: Span,
    },
    Remember {
        key: Expr,
        value: Expr,
        span: Span,
    },
    CallStmt {
        tool: Expr,
        arg: Expr,
        span: Span,
    },
    Return {
        expr: Expr,
        span: Span,
    },
    If {
        cond: Expr,
        then_block: Block,
        else_block: Option<Block>,
        span: Span,
    },
    While {
        cond: Expr,
        body: Block,
        span: Span,
    },
    ExprStmt {
        expr: Expr,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Literal { value: Literal, span: Span },
    Id { name: String, span: Span },
    Recall { key: Box<Expr>, span: Span },
    Call { name: Box<Expr>, arg: Box<Expr>, span: Span },
    Index { target: Box<Expr>, index: Box<Expr>, span: Span },
    List {
        items: Vec<Expr>,
        span: Span,
    },
    Map {
        entries: Vec<(String, Expr)>,
        span: Span,
    },
    Binary {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
        span: Span,
    },
    Paren(Box<Expr>),
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Literal { span, .. } => *span,
            Expr::Id { span, .. } => *span,
            Expr::Recall { span, .. } => *span,
            Expr::Call { span, .. } => *span,
            Expr::Index { span, .. } => *span,
            Expr::List { span, .. } => *span,
            Expr::Map { span, .. } => *span,
            Expr::Binary { span, .. } => *span,
            Expr::Paren(inner) => inner.span(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Eq,
    Ne,
    And,
    Or,
}

#[derive(Debug, Clone)]
pub enum Literal {
    Num(f64),
    Str(String),
    True,
    False,
    Null,
}
