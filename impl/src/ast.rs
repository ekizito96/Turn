//! AST node definitions per spec/02-grammar.md and spec/01-minimal-core.md.

use indexmap::IndexMap;
use crate::lexer::Span;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Type {
    Num,
    Str,
    Bool,
    List(Box<Type>),
    Map(Box<Type>),
    Function(Box<Type>, Box<Type>), // Arg -> Ret
    Struct(String, IndexMap<String, Type>), // Name, Fields
    Any,
    Void,
    Pid,
    Vec,
}

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
        ty: Option<Type>,
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
    ImplDef {
        type_name: String,
        methods: Vec<Stmt>, // Stmt::Let (functions)
        span: Span,
    },
    TypeAlias {
        name: String,
        ty: Type,
        span: Span,
    },
    StructDef {
        name: String,
        fields: IndexMap<String, Type>,
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
    Suspend {
        span: Span,
    },
    TryCatch {
        try_block: Block,
        catch_var: String,
        catch_block: Block,
        span: Span,
    },
    Throw {
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
    MethodCall {
        target: Box<Expr>,
        name: String,
        arg: Box<Expr>,
        span: Span,
    },
    Recall { key: Box<Expr>, span: Span },
    Call { name: Box<Expr>, arg: Box<Expr>, span: Span },
    Use { module: Box<Expr>, span: Span },
    Spawn { expr: Box<Expr>, span: Span },
    Send { pid: Box<Expr>, msg: Box<Expr>, span: Span },
    Receive { span: Span },
    Vec { items: Vec<Expr>, span: Span },
    Confidence { expr: Box<Expr>, span: Span },
    StructInit {
        name: String,
        fields: IndexMap<String, Expr>,
        span: Span,
    },
    Index { target: Box<Expr>, index: Box<Expr>, span: Span },
    Turn {
        params: Vec<(String, Span, Option<Type>)>,
        ret_ty: Option<Type>,
        body: Block,
        span: Span,
    },
    Infer {
        target_ty: Type,
        body: Block,
        span: Span,
    },
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
    Unary {
        op: UnOp,
        expr: Box<Expr>,
        span: Span,
    },
    Paren(Box<Expr>),
}

    impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Literal { span, .. } => *span,
            Expr::Id { span, .. } => *span,
            Expr::MethodCall { span, .. } => *span,
            Expr::Recall { span, .. } => *span,
            Expr::Call { span, .. } => *span,
            Expr::Use { span, .. } => *span,
            Expr::Spawn { span, .. } => *span,
            Expr::Send { span, .. } => *span,
            Expr::Receive { span } => *span,
            Expr::Vec { span, .. } => *span,
            Expr::Confidence { span, .. } => *span,
            Expr::Index { span, .. } => *span,
            Expr::Turn { span, .. } => *span,
            Expr::Infer { span, .. } => *span,
            Expr::List { span, .. } => *span,
            Expr::Map { span, .. } => *span,
            Expr::Binary { span, .. } => *span,
            Expr::Unary { span, .. } => *span,
            Expr::Paren(inner) => inner.span(),
            Expr::StructInit { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Not,
    Neg,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Mul,
    Div,
    Add,
    Sub,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    Similarity,
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
