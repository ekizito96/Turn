//! Parser for Turn. Precedence-climbing for expressions per spec/02-grammar.md.
//! Precedence (highest to lowest): + > == != > and > or

use crate::ast::*;
use crate::lexer::{Span, SpannedToken, Token};
use std::iter::Peekable;
use std::vec::IntoIter;

pub struct Parser {
    tokens: Peekable<IntoIter<SpannedToken>>,
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
            tokens: tokens.into_iter().peekable(),
            last_span: Span { start: 0, end: 0 },
        }
    }

    fn peek(&mut self) -> Option<&Token> {
        self.tokens.peek().map(|t| &t.token)
    }

    fn next(&mut self) -> Option<SpannedToken> {
        let t = self.tokens.next()?;
        self.last_span = t.span;
        Some(t)
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
            Token::TypeNum => Ok(Type::Num),
            Token::TypeStr => Ok(Type::Str),
            Token::TypeBool => Ok(Type::Bool),
            Token::TypeList => Ok(Type::List),
            Token::TypeMap => Ok(Type::Map),
            Token::TypeAny => Ok(Type::Any),
            Token::TypeVoid => Ok(Type::Void),
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
                Ok(Stmt::Let { name, ty, init, span })
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
            Some(Token::Try) => {
                self.next();
                let try_block = self.parse_block()?;
                self.expect(Token::Catch)?;
                self.expect(Token::LParen)?;
                let catch_var = match self.next() {
                    Some(SpannedToken { token: Token::Id(s), .. }) => s,
                    _ => return Err(ParseError::UnexpectedToken(self.span())),
                };
                self.expect(Token::RParen)?;
                let catch_block = self.parse_block()?;
                let end = catch_block.span.end;
                Ok(Stmt::TryCatch {
                    try_block,
                    catch_var,
                    catch_block,
                    span: Span {
                        start: span.start,
                        end,
                    },
                })
            }
            Some(Token::Throw) => {
                self.next();
                let expr = self.parse_expr()?;
                self.expect(Token::Semicolon)?;
                Ok(Stmt::Throw {
                    expr,
                    span: Span {
                        start: span.start,
                        end: self.last_span.end,
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
        let mut left = self.parse_add()?;
        let span = self.span();
        loop {
            let op = match self.peek() {
                Some(Token::EqEq) => BinOp::Eq,
                Some(Token::Ne) => BinOp::Ne,
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
        while matches!(self.peek(), Some(Token::Plus)) {
            self.next();
            let right = self.parse_mul()?;
            left = Expr::Binary {
                op: BinOp::Add,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_mul(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_postfix()?;
        let span = self.span();
        while matches!(self.peek(), Some(Token::Star)) {
            self.next();
            let right = self.parse_postfix()?;
            left = Expr::Binary {
                op: BinOp::Mul,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_postfix(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_primary()?;
        loop {
            match self.peek() {
                Some(Token::LBracket) => {
                    self.next();
                    let index = self.parse_expr()?;
                    self.expect(Token::RBracket)?;
                    let span = Span { start: expr.span().start, end: self.last_span.end };
                    expr = Expr::Index {
                        target: Box::new(expr),
                        index: Box::new(index),
                        span,
                    };
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
            Token::Num(n) => Ok(Expr::Literal { value: Literal::Num(n), span }),
            Token::Str(s) => Ok(Expr::Literal { value: Literal::Str(s), span }),
            Token::True => Ok(Expr::Literal { value: Literal::True, span }),
            Token::False => Ok(Expr::Literal { value: Literal::False, span }),
            Token::Null => Ok(Expr::Literal { value: Literal::Null, span }),
            Token::Id(name) => Ok(Expr::Id { name, span }),
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
            Token::Turn => {
                let mut params = Vec::new();
                if matches!(self.peek(), Some(Token::LParen)) {
                    self.next(); // consume (
                    while !matches!(self.peek(), Some(Token::RParen) | Some(Token::Eof)) {
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
                         
                         params.push((name, name_span, ty));
                         
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
                Ok(Expr::Turn { params, ret_ty, body, span })
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
