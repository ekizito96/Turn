//! Parser for Turn. Precedence-climbing for expressions per spec/02-grammar.md.
//! Precedence (highest to lowest): + > == != > and > or

use crate::ast::*;
use crate::lexer::{Span, SpannedToken, Token};
use indexmap::IndexMap;

pub struct Parser {
    tokens: Vec<SpannedToken>,
    pos: usize,
    last_span: Span,
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("unexpected token at {0:?}")]
    UnexpectedToken(Span),
    #[error("unexpected end of input")]
    UnexpectedEof,
}

impl ParseError {
    pub fn offset(&self) -> usize {
        match self {
            Self::UnexpectedToken(span) => span.start,
            Self::UnexpectedEof => 0,
        }
    }
}

impl Parser {
    pub fn new(tokens: Vec<SpannedToken>) -> Self {
        Self {
            tokens,
            pos: 0,
            last_span: Span { start: 0, end: 0 },
        }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos).map(|t| &t.token)
    }

    fn peek_at(&self, offset: usize) -> Option<&Token> {
        self.tokens.get(self.pos + offset).map(|t| &t.token)
    }

    fn next(&mut self) -> Option<SpannedToken> {
        if self.pos < self.tokens.len() {
            let t = self.tokens[self.pos].clone();
            self.pos += 1;
            self.last_span = t.span;
            Some(t)
        } else {
            None
        }
    }

    fn expect(&mut self, expected: Token) -> Result<Span, ParseError> {
        let t = self.next().ok_or(ParseError::UnexpectedEof)?;
        if std::mem::discriminant(&t.token) == std::mem::discriminant(&expected) {
            Ok(t.span)
        } else {
            Err(ParseError::UnexpectedToken(t.span))
        }
    }

    fn span(&self) -> Span {
        self.last_span
    }

    pub fn parse(&mut self) -> Result<Program, ParseError> {
        let mut stmts = Vec::new();
        while !matches!(self.peek(), Some(Token::Eof)) {
            stmts.push(self.parse_stmt()?);
        }
        Ok(Program { stmts })
    }

    fn parse_type(&mut self) -> Result<Type, ParseError> {
        let t = self.next().ok_or(ParseError::UnexpectedEof)?;
        match t.token {
            Token::LParen => {
                let arg_type = self.parse_type()?;
                self.expect(Token::RParen)?;
                self.expect(Token::Arrow)?;
                let ret_type = self.parse_type()?;
                Ok(Type::Function(Box::new(arg_type), Box::new(ret_type)))
            }
            Token::TypeNum => Ok(Type::Num),
            Token::TypeStr => Ok(Type::Str),
            Token::TypeBool => Ok(Type::Bool),
            Token::TypeList => {
                if matches!(self.peek(), Some(Token::Less)) {
                    self.next(); // consume <
                    let inner = self.parse_type()?;
                    self.expect(Token::Greater)?;
                    Ok(Type::List(Box::new(inner)))
                } else {
                    Ok(Type::List(Box::new(Type::Any)))
                }
            }
            Token::TypeMap => {
                if matches!(self.peek(), Some(Token::Less)) {
                    self.next(); // consume <
                    let key = self.parse_type()?;
                    self.expect(Token::Comma)?;
                    let value = self.parse_type()?;
                    self.expect(Token::Greater)?;
                    Ok(Type::Map(Box::new(key), Box::new(value)))
                } else {
                    Ok(Type::Map(Box::new(Type::Str), Box::new(Type::Any))) // By default Map is String -> Any
                }
            }
            Token::TypeAny => Ok(Type::Any),
            Token::TypeVoid => Ok(Type::Void),
            Token::TypePid => Ok(Type::Pid),
            Token::TypeVec => Ok(Type::Vec),
            Token::TypeCap => Ok(Type::Cap),
            Token::TypeResult => {
                self.expect(Token::Less)?;
                let ok_ty = self.parse_type()?;
                self.expect(Token::Comma)?;
                let err_ty = self.parse_type()?;
                self.expect(Token::Greater)?;
                Ok(Type::Result(Box::new(ok_ty), Box::new(err_ty)))
            }
            Token::Id(name) => Ok(Type::Struct(name, IndexMap::new())), // Placeholder until resolution
            _ => Err(ParseError::UnexpectedToken(t.span)),
        }
    }

    fn parse_stmt(&mut self) -> Result<Stmt, ParseError> {
        let span = self.span();
        match self.peek() {
            Some(Token::Turn) => {
                self.next();
                let body = self.parse_block()?;
                Ok(Stmt::Turn { body, span })
            }
            Some(Token::Type) => {
                self.next();
                let Token::Id(name) = self.next().ok_or(ParseError::UnexpectedEof)?.token else {
                    return Err(ParseError::UnexpectedToken(self.span()));
                };
                self.expect(Token::Eq)?;
                let ty = self.parse_type()?;
                self.expect(Token::Semicolon)?;
                Ok(Stmt::TypeAlias { name, ty, span })
            }
            Some(Token::Impl) => {
                self.next();
                let Token::Id(type_name) = self.next().ok_or(ParseError::UnexpectedEof)?.token
                else {
                    return Err(ParseError::UnexpectedToken(self.span()));
                };
                self.expect(Token::LBrace)?;
                let mut methods = Vec::new();
                while !matches!(self.peek(), Some(Token::RBrace) | Some(Token::Eof)) {
                    // Only Let statements (functions) allowed in impl block for now
                    if matches!(self.peek(), Some(Token::Let)) {
                        methods.push(self.parse_stmt()?);
                    } else {
                        // Or allow arbitrary logic? No, definitions only.
                        // For simplicity, reuse parse_stmt, but validation logic belongs in analysis.
                        methods.push(self.parse_stmt()?);
                    }
                }
                self.expect(Token::RBrace)?;
                if matches!(self.peek(), Some(Token::Semicolon)) {
                    self.next();
                }
                Ok(Stmt::ImplDef {
                    type_name,
                    methods,
                    span,
                })
            }
            Some(Token::Struct) => {
                // parse struct definition: struct Foo { x: Num, y: Str };
                self.next();
                let name_token = self.next().ok_or(ParseError::UnexpectedEof)?;
                let name = match name_token.token {
                    Token::Id(s) => s,
                    _ => return Err(ParseError::UnexpectedToken(name_token.span)),
                };

                self.expect(Token::LBrace)?;
                let mut fields = IndexMap::new();
                while !matches!(self.peek(), Some(Token::RBrace) | Some(Token::Eof)) {
                    let field_name_token = self.next().ok_or(ParseError::UnexpectedEof)?;
                    let field_name = match field_name_token.token {
                        Token::Id(s) => s,
                        _ => return Err(ParseError::UnexpectedToken(field_name_token.span)),
                    };

                    self.expect(Token::Colon)?;
                    let ty = self.parse_type()?;
                    fields.insert(field_name, ty);

                    if matches!(self.peek(), Some(Token::Comma)) {
                        self.next();
                    } else {
                        break;
                    }
                }
                self.expect(Token::RBrace)?;
                self.expect(Token::Semicolon)?; // struct Foo { ... };
                Ok(Stmt::StructDef { name, fields, span })
            }
            Some(Token::Let) => {
                self.next();
                let Token::Id(name) = self.next().ok_or(ParseError::UnexpectedEof)?.token else {
                    return Err(ParseError::UnexpectedToken(self.span()));
                };

                let ty = if matches!(self.peek(), Some(Token::Colon)) {
                    self.next();
                    Some(self.parse_type()?)
                } else {
                    None
                };

                self.expect(Token::Eq)?;
                let init = self.parse_expr()?;
                self.expect(Token::Semicolon)?;
                Ok(Stmt::Let {
                    name,
                    ty,
                    init,
                    span,
                })
            }
            Some(Token::Context) => {
                self.next();
                self.expect(Token::Dot)?;
                self.expect(Token::Append)?;
                self.expect(Token::LParen)?;
                let expr = self.parse_expr()?;
                self.expect(Token::RParen)?;
                self.expect(Token::Semicolon)?;
                Ok(Stmt::ContextAppend { expr, span })
            }
            Some(Token::Remember) => {
                self.next();
                self.expect(Token::LParen)?;
                let key = self.parse_expr()?;
                self.expect(Token::Comma)?;
                let value = self.parse_expr()?;
                self.expect(Token::RParen)?;
                self.expect(Token::Semicolon)?;
                Ok(Stmt::Remember { key, value, span })
            }
            Some(Token::Call) => {
                self.next();
                self.expect(Token::LParen)?;
                let tool = self.parse_expr()?;
                self.expect(Token::Comma)?;
                let arg = self.parse_expr()?;
                self.expect(Token::RParen)?;
                self.expect(Token::Semicolon)?;
                Ok(Stmt::CallStmt { tool, arg, span })
            }
            Some(Token::Return) => {
                self.next();
                let expr = self.parse_expr()?;
                self.expect(Token::Semicolon)?;
                Ok(Stmt::Return { expr, span })
            }
            Some(Token::If) => {
                self.next();
                let cond = self.parse_expr()?;
                let then_block = self.parse_block()?;
                let else_block = if matches!(self.peek(), Some(Token::Else)) {
                    self.next();
                    Some(self.parse_block()?)
                } else {
                    None
                };
                Ok(Stmt::If {
                    cond,
                    then_block,
                    else_block,
                    span,
                })
            }
            Some(Token::While) => {
                self.next();
                let cond = self.parse_expr()?;
                let body = self.parse_block()?;
                Ok(Stmt::While { cond, body, span })
            }
            Some(Token::Suspend) => {
                self.next();
                self.expect(Token::Semicolon)?;
                Ok(Stmt::Suspend { span })
            }
            Some(Token::Match) => {
                self.next();
                let expr = self.parse_expr()?;
                self.expect(Token::LBrace)?;

                self.expect(Token::Ok)?;
                self.expect(Token::LParen)?;
                let ok_binding = match self.next() {
                    Some(SpannedToken {
                        token: Token::Id(s),
                        ..
                    }) => s,
                    _ => return Err(ParseError::UnexpectedToken(self.span())),
                };
                self.expect(Token::RParen)?;
                self.expect(Token::Arrow)?;
                let ok_block = self.parse_block()?;

                if matches!(self.peek(), Some(Token::Comma)) {
                    self.next(); // Optional comma
                }

                self.expect(Token::Err)?;
                self.expect(Token::LParen)?;
                let err_binding = match self.next() {
                    Some(SpannedToken {
                        token: Token::Id(s),
                        ..
                    }) => s,
                    _ => return Err(ParseError::UnexpectedToken(self.span())),
                };
                self.expect(Token::RParen)?;
                self.expect(Token::Arrow)?;
                let err_block = self.parse_block()?;

                if matches!(self.peek(), Some(Token::Comma)) {
                    self.next(); // Optional comma
                }

                self.expect(Token::RBrace)?;

                let end = self.span().end;
                Ok(Stmt::Match {
                    expr,
                    ok_binding,
                    ok_block,
                    err_binding,
                    err_block,
                    span: Span {
                        start: span.start,
                        end,
                    },
                })
            }
            _ => {
                let expr = self.parse_expr()?;
                self.expect(Token::Semicolon)?;
                Ok(Stmt::ExprStmt { expr, span })
            }
        }
    }

    fn parse_block(&mut self) -> Result<Block, ParseError> {
        let start = self.span().start;
        self.expect(Token::LBrace)?;
        let mut stmts = Vec::new();
        while !matches!(self.peek(), Some(Token::RBrace) | Some(Token::Eof)) {
            stmts.push(self.parse_stmt()?);
        }
        self.expect(Token::RBrace)?;
        let end = self.span().end;
        Ok(Block {
            stmts,
            span: Span { start, end },
        })
    }

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_and()?;
        let span = self.span();
        while matches!(self.peek(), Some(Token::Or)) {
            self.next();
            let right = self.parse_and()?;
            left = Expr::Binary {
                op: BinOp::Or,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_eq()?;
        let span = self.span();
        while matches!(self.peek(), Some(Token::And)) {
            self.next();
            let right = self.parse_eq()?;
            left = Expr::Binary {
                op: BinOp::And,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_eq(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_rel()?;
        let span = self.span();
        loop {
            let op = match self.peek() {
                Some(Token::EqEq) => BinOp::Eq,
                Some(Token::Ne) => BinOp::Ne,
                Some(Token::Similarity) => BinOp::Similarity,
                _ => break,
            };
            self.next();
            let right = self.parse_rel()?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_rel(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_add()?;
        let span = self.span();
        loop {
            let op = match self.peek() {
                Some(Token::Less) => BinOp::Lt,
                Some(Token::Greater) => BinOp::Gt,
                Some(Token::LessEq) => BinOp::Le,
                Some(Token::GreaterEq) => BinOp::Ge,
                _ => break,
            };
            self.next();
            let right = self.parse_add()?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_add(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_mul()?;
        let span = self.span();
        loop {
            let op = match self.peek() {
                Some(Token::Plus) => BinOp::Add,
                Some(Token::Minus) => BinOp::Sub,
                _ => break,
            };
            self.next();
            let right = self.parse_mul()?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_mul(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_unary()?;
        let span = self.span();
        loop {
            let op = match self.peek() {
                Some(Token::Star) => BinOp::Mul,
                Some(Token::Slash) => BinOp::Div,
                _ => break,
            };
            self.next();
            let right = self.parse_unary()?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        let span = self.span();
        match self.peek() {
            Some(Token::Minus) => {
                self.next();
                let expr = self.parse_unary()?;
                Ok(Expr::Unary {
                    op: UnOp::Neg,
                    expr: Box::new(expr),
                    span,
                })
            }
            Some(Token::Spawn) => {
                self.next();
                let expr = self.parse_unary()?; // High precedence prefix
                Ok(Expr::Spawn {
                    expr: Box::new(expr),
                    span,
                })
            }
            Some(Token::Infer) => {
                self.next();
                let target_ty = self.parse_type()?;
                let tools = if matches!(self.peek(), Some(Token::With)) {
                    self.next(); // consume `with`
                    self.expect(Token::LBracket)?;
                    let mut items = Vec::new();
                    while !matches!(self.peek(), Some(Token::RBracket) | Some(Token::Eof)) {
                        items.push(self.parse_expr()?);
                        if matches!(self.peek(), Some(Token::Comma)) {
                            self.next();
                        } else {
                            break;
                        }
                    }
                    self.expect(Token::RBracket)?;
                    Some(items)
                } else {
                    None
                };
                let body = self.parse_block()?;
                Ok(Expr::Infer {
                    target_ty,
                    tools,
                    body,
                    span,
                })
            }
            Some(Token::SpawnRemote) => {
                self.next();
                self.expect(Token::LParen)?;
                let node_id = self.parse_expr()?;
                self.expect(Token::Comma)?;
                let closure = self.parse_expr()?;
                self.expect(Token::RParen)?;
                Ok(Expr::SpawnRemote {
                    node_id: Box::new(node_id),
                    closure: Box::new(closure),
                    span,
                })
            }
            Some(Token::Send) => {
                self.next();
                // send <pid>, <msg>
                // Treat send as high precedence prefix?
                // parse_unary(pid) -> expect(Comma) -> parse_expr(msg)?
                // If msg contains binary ops, parse_expr handles it.
                let pid = self.parse_unary()?;
                self.expect(Token::Comma)?;
                let msg = self.parse_expr()?;
                Ok(Expr::Send {
                    pid: Box::new(pid),
                    msg: Box::new(msg),
                    span,
                })
            }
            Some(Token::Link) => {
                self.next();
                let pid = self.parse_unary()?;
                Ok(Expr::Link {
                    pid: Box::new(pid),
                    span,
                })
            }
            Some(Token::Monitor) => {
                self.next();
                let pid = self.parse_unary()?;
                Ok(Expr::Monitor {
                    pid: Box::new(pid),
                    span,
                })
            }
            Some(Token::Confidence) => {
                self.next();
                let expr = self.parse_unary()?;
                Ok(Expr::Confidence {
                    expr: Box::new(expr),
                    span,
                })
            }
            Some(Token::Bang) => {
                self.next();
                let expr = self.parse_unary()?;
                Ok(Expr::Unary {
                    op: UnOp::Not,
                    expr: Box::new(expr),
                    span,
                })
            }
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_primary()?;
        loop {
            match self.peek() {
                Some(Token::LBracket) => {
                    self.next();
                    let index = self.parse_expr()?;
                    self.expect(Token::RBracket)?;
                    let span = Span {
                        start: expr.span().start,
                        end: self.last_span.end,
                    };
                    expr = Expr::Index {
                        target: Box::new(expr),
                        index: Box::new(index),
                        span,
                    };
                }
                Some(Token::Dot) => {
                    self.next();
                    let Token::Id(name) = self.next().ok_or(ParseError::UnexpectedEof)?.token
                    else {
                        return Err(ParseError::UnexpectedToken(self.span()));
                    };

                    if matches!(self.peek(), Some(Token::LParen)) {
                        self.next(); // consume (
                        let mut args = Vec::new();
                        while !matches!(self.peek(), Some(Token::RParen) | Some(Token::Eof)) {
                            args.push(self.parse_expr()?);
                            if matches!(self.peek(), Some(Token::Comma)) {
                                self.next();
                            } else {
                                break;
                            }
                        }
                        self.expect(Token::RParen)?;

                        let arg = if args.is_empty() {
                            Expr::Literal {
                                value: Literal::Null,
                                span: self.last_span,
                            }
                        } else if args.len() == 1 {
                            args.into_iter().next().unwrap()
                        } else {
                            Expr::List {
                                items: args,
                                span: self.last_span,
                            }
                        };

                        let span = Span {
                            start: expr.span().start,
                            end: self.last_span.end,
                        };
                        expr = Expr::MethodCall {
                            target: Box::new(expr),
                            name,
                            arg: Box::new(arg),
                            span,
                        };
                    } else {
                        // Property access sugar: obj.prop -> obj["prop"]
                        let span = Span {
                            start: expr.span().start,
                            end: self.last_span.end,
                        };
                        expr = Expr::Index {
                            target: Box::new(expr),
                            index: Box::new(Expr::Literal {
                                value: Literal::Str(name),
                                span: self.last_span,
                            }),
                            span,
                        };
                    }
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        let t = self.next().ok_or(ParseError::UnexpectedEof)?;
        let span = t.span;
        match t.token {
            Token::Ok => {
                self.expect(Token::LParen)?;
                let expr = self.parse_expr()?;
                self.expect(Token::RParen)?;
                Ok(Expr::Ok(Box::new(expr), span))
            }
            Token::Err => {
                self.expect(Token::LParen)?;
                let expr = self.parse_expr()?;
                self.expect(Token::RParen)?;
                Ok(Expr::Err(Box::new(expr), span))
            }
            Token::Num(n) => Ok(Expr::Literal {
                value: Literal::Num(n),
                span,
            }),
            Token::Str(s) => Ok(Expr::Literal {
                value: Literal::Str(s),
                span,
            }),
            Token::True => Ok(Expr::Literal {
                value: Literal::True,
                span,
            }),
            Token::False => Ok(Expr::Literal {
                value: Literal::False,
                span,
            }),
            Token::Null => Ok(Expr::Literal {
                value: Literal::Null,
                span,
            }),
            Token::Id(name) => {
                // Check if it's a struct instantiation: Foo { x: 1 }
                let mut is_struct_init = false;
                if matches!(self.peek(), Some(Token::LBrace)) {
                    // Lookahead to distinguish from block: if x { ... }
                    // StructInit must be: { <ID> : ... } or { }
                    match self.peek_at(1) {
                        Some(Token::RBrace) => is_struct_init = true, // Foo {}
                        Some(Token::Id(_)) => {
                            if matches!(self.peek_at(2), Some(Token::Colon)) {
                                is_struct_init = true; // Foo { x: ... }
                            }
                        }
                        _ => {}
                    }
                }

                if is_struct_init {
                    // If it's Foo { ... }, it's struct init.

                    self.next(); // consume LBrace
                    let mut fields = IndexMap::new();
                    while !matches!(self.peek(), Some(Token::RBrace) | Some(Token::Eof)) {
                        let field_token = self.next().ok_or(ParseError::UnexpectedEof)?;
                        let field_name = match field_token.token {
                            Token::Id(s) => s,
                            _ => return Err(ParseError::UnexpectedToken(field_token.span)),
                        };
                        self.expect(Token::Colon)?;
                        let val = self.parse_expr()?;
                        fields.insert(field_name, val);
                        if matches!(self.peek(), Some(Token::Comma)) {
                            self.next();
                        } else {
                            break;
                        }
                    }
                    self.expect(Token::RBrace)?;
                    Ok(Expr::StructInit { name, fields, span })
                } else {
                    Ok(Expr::Id { name, span })
                }
            }
            Token::Recall => {
                self.expect(Token::LParen)?;
                let key = self.parse_expr()?;
                self.expect(Token::RParen)?;
                Ok(Expr::Recall {
                    key: Box::new(key),
                    span,
                })
            }
            Token::Call => {
                self.expect(Token::LParen)?;
                let name = self.parse_expr()?;
                self.expect(Token::Comma)?;
                let arg = self.parse_expr()?;
                self.expect(Token::RParen)?;
                Ok(Expr::Call {
                    name: Box::new(name),
                    arg: Box::new(arg),
                    span,
                })
            }
            Token::Use => {
                let module = self.parse_expr()?;
                Ok(Expr::Use {
                    module: Box::new(module),
                    span,
                })
            }
            Token::Receive => Ok(Expr::Receive { span }),
            Token::Vec => {
                self.expect(Token::LBracket)?;
                let mut items = Vec::new();
                while !matches!(self.peek(), Some(Token::RBracket) | Some(Token::Eof)) {
                    items.push(self.parse_expr()?);
                    if matches!(self.peek(), Some(Token::Comma)) {
                        self.next();
                    } else {
                        break;
                    }
                }
                self.expect(Token::RBracket)?;
                Ok(Expr::Vec { items, span })
            }
            Token::Tool | Token::Turn => {
                let is_tool = t.token == Token::Tool;
                if is_tool {
                    self.expect(Token::Turn)?;
                }

                let mut params = Vec::new();
                if matches!(self.peek(), Some(Token::LParen)) {
                    self.next(); // consume (
                    while !matches!(self.peek(), Some(Token::RParen) | Some(Token::Eof)) {
                        let mut is_secret = false;
                        if matches!(self.peek(), Some(Token::Secret)) {
                            self.next();
                            is_secret = true;
                        }

                        let name_token = self.next().ok_or(ParseError::UnexpectedEof)?;
                        let (name, name_span) = match name_token.token {
                            Token::Id(s) => (s, name_token.span),
                            _ => return Err(ParseError::UnexpectedToken(name_token.span)),
                        };

                        let ty = if matches!(self.peek(), Some(Token::Colon)) {
                            self.next();
                            Some(self.parse_type()?)
                        } else {
                            None
                        };

                        params.push((name, name_span, ty, is_secret));

                        if matches!(self.peek(), Some(Token::Comma)) {
                            self.next();
                        } else {
                            break;
                        }
                    }
                    self.expect(Token::RParen)?;
                }

                let ret_ty = if matches!(self.peek(), Some(Token::Arrow)) {
                    self.next();
                    Some(self.parse_type()?)
                } else {
                    None
                };

                let body = self.parse_block()?;
                Ok(Expr::Turn {
                    is_tool,
                    params,
                    ret_ty,
                    body,
                    span,
                })
            }
            Token::LParen => {
                let inner = self.parse_expr()?;
                self.expect(Token::RParen)?;
                Ok(Expr::Paren(Box::new(inner)))
            }
            Token::LBracket => {
                let mut items = Vec::new();
                while !matches!(self.peek(), Some(Token::RBracket) | Some(Token::Eof)) {
                    items.push(self.parse_expr()?);
                    if matches!(self.peek(), Some(Token::Comma)) {
                        self.next();
                    } else {
                        break;
                    }
                }
                self.expect(Token::RBracket)?;
                Ok(Expr::List { items, span })
            }
            Token::LBrace => {
                let mut entries = Vec::new();
                while !matches!(self.peek(), Some(Token::RBrace) | Some(Token::Eof)) {
                    let key_token = self.next().ok_or(ParseError::UnexpectedEof)?;
                    let key = match key_token.token {
                        Token::Str(s) => s,
                        _ => return Err(ParseError::UnexpectedToken(key_token.span)),
                    };
                    self.expect(Token::Colon)?;
                    let val = self.parse_expr()?;
                    entries.push((key, val));
                    if matches!(self.peek(), Some(Token::Comma)) {
                        self.next();
                    } else {
                        break;
                    }
                }
                self.expect(Token::RBrace)?;
                Ok(Expr::Map { entries, span })
            }
            _ => Err(ParseError::UnexpectedToken(span)),
        }
    }
}
