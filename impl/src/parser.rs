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
                self.expect(Token::Eq)?;
                let init = self.parse_expr()?;
                self.expect(Token::Semicolon)?;
                Ok(Stmt::Let { name, init, span })
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
        let mut left = self.parse_primary()?;
        let span = self.span();
        while matches!(self.peek(), Some(Token::Plus)) {
            self.next();
            let right = self.parse_primary()?;
            left = Expr::Binary {
                op: BinOp::Add,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        let t = self.next().ok_or(ParseError::UnexpectedEof)?;
        let span = t.span;
        match t.token {
            Token::Num(n) => Ok(Expr::Literal(Literal::Num(n))),
            Token::Str(s) => Ok(Expr::Literal(Literal::Str(s))),
            Token::True => Ok(Expr::Literal(Literal::True)),
            Token::False => Ok(Expr::Literal(Literal::False)),
            Token::Null => Ok(Expr::Literal(Literal::Null)),
            Token::Id(name) => Ok(Expr::Id(name)),
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
            Token::LParen => {
                let inner = self.parse_expr()?;
                self.expect(Token::RParen)?;
                Ok(Expr::Paren(Box::new(inner)))
            }
            _ => Err(ParseError::UnexpectedToken(span)),
        }
    }
}
