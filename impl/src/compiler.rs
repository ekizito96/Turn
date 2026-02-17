//! Compiles AST to bytecode.

use crate::ast::*;
use crate::bytecode::Instr;

pub struct Compiler {
    code: Vec<Instr>,
}

impl Compiler {
    pub fn new() -> Self {
        Self { code: Vec::new() }
    }

    fn emit(&mut self, instr: Instr) -> u32 {
        let addr = self.code.len() as u32;
        self.code.push(instr);
        addr
    }

    fn patch_jump(&mut self, addr: u32, target: u32) {
        let instr = &mut self.code[addr as usize];
        match instr {
            Instr::Jump(ref mut t) => *t = target,
            Instr::JumpIfFalse(ref mut t) => *t = target,
            Instr::JumpIfTrue(ref mut t) => *t = target,
            Instr::EnterTurn(ref mut t) => *t = target,
            _ => {}
        }
    }

    pub fn compile(&mut self, program: &Program) -> Vec<Instr> {
        for stmt in &program.stmts {
            self.compile_stmt(stmt);
        }
        self.code.clone()
    }

    fn compile_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Turn { body, .. } => {
                let enter_turn_addr = self.emit(Instr::EnterTurn(0));
                self.compile_block(body);
                // Implicit return if not present
                let has_return = body.stmts.last().map_or(false, |s| matches!(s, Stmt::Return { .. }));
                if !has_return {
                    self.emit(Instr::PushNull);
                    self.emit(Instr::Return);
                }
                let after_turn = self.code.len() as u32;
                self.patch_jump(enter_turn_addr, after_turn);
            }
            Stmt::Let { name, init, .. } => {
                self.compile_expr(init);
                self.emit(Instr::Store(name.clone()));
            }
            Stmt::ContextAppend { expr, .. } => {
                self.compile_expr(expr);
                self.emit(Instr::ContextAppend);
            }
            Stmt::Remember { key, value, .. } => {
                self.compile_expr(key);
                self.compile_expr(value);
                self.emit(Instr::Remember);
            }
            Stmt::CallStmt { tool, arg, .. } => {
                self.compile_expr(tool);
                self.compile_expr(arg);
                self.emit(Instr::CallTool);
                self.emit(Instr::Pop); // discard result
            }
            Stmt::Return { expr, .. } => {
                self.compile_expr(expr);
                self.emit(Instr::Return);
            }
            Stmt::If {
                cond,
                then_block,
                else_block,
                ..
            } => {
                self.compile_expr(cond);
                let jump_false = self.emit(Instr::JumpIfFalse(0));
                self.compile_block(then_block);
                let else_jump = if else_block.is_some() {
                    Some(self.emit(Instr::Jump(0)))
                } else {
                    None
                };
                self.patch_jump(jump_false, self.code.len() as u32);
                if let Some(ref block) = else_block {
                    self.compile_block(block);
                }
                if let Some(addr) = else_jump {
                    self.patch_jump(addr, self.code.len() as u32);
                }
            }
            Stmt::While { cond, body, .. } => {
                let loop_start = self.code.len() as u32;
                self.compile_expr(cond);
                let exit_jump = self.emit(Instr::JumpIfFalse(0));
                self.compile_block(body);
                self.emit(Instr::Jump(loop_start));
                self.patch_jump(exit_jump, self.code.len() as u32);
            }
            Stmt::ExprStmt { expr, .. } => {
                self.compile_expr(expr);
                self.emit(Instr::Pop);
            }
        }
    }

    fn compile_block(&mut self, block: &Block) {
        for stmt in &block.stmts {
            self.compile_stmt(stmt);
        }
    }

    fn compile_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Literal(lit) => match lit {
                Literal::Num(n) => {
                    self.emit(Instr::PushNum(*n));
                }
                Literal::Str(s) => {
                    self.emit(Instr::PushStr(s.clone()));
                }
                Literal::True => {
                    self.emit(Instr::PushTrue);
                }
                Literal::False => {
                    self.emit(Instr::PushFalse);
                }
                Literal::Null => {
                    self.emit(Instr::PushNull);
                }
            },
            Expr::Id(name) => {
                self.emit(Instr::Load(name.clone()));
            }
            Expr::Recall { key, .. } => {
                self.compile_expr(key);
                self.emit(Instr::Recall);
            }
            Expr::Call { name, arg, .. } => {
                self.compile_expr(name);
                self.compile_expr(arg);
                self.emit(Instr::CallTool);
            }
            Expr::Binary { op, left, right, .. } => {
                self.compile_expr(left);
                self.compile_expr(right);
                match op {
                    BinOp::Add => { self.emit(Instr::Add); }
                    BinOp::Eq => { self.emit(Instr::Eq); }
                    BinOp::Ne => { self.emit(Instr::Ne); }
                    BinOp::And => { self.emit(Instr::And); }
                    BinOp::Or => { self.emit(Instr::Or); }
                }
            }
            Expr::Paren(inner) => self.compile_expr(inner),
        }
    }
}
