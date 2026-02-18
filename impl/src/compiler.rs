//! Compiles AST to bytecode.

use crate::ast::*;
use crate::bytecode::Instr;

pub struct Compiler {
    code: Vec<Instr>,
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
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
            Instr::PushHandler(ref mut t) => *t = target,
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
                let has_return = body
                    .stmts
                    .last()
                    .is_some_and(|s| matches!(s, Stmt::Return { .. }));
                if !has_return {
                    self.emit(Instr::PushNull);
                    self.emit(Instr::Return);
                }
                let after_turn = self.code.len() as u32;
                self.patch_jump(enter_turn_addr, after_turn);
            }
            Stmt::Let { name, ty, init, .. } => {
                self.compile_expr(init);
                if let Some(t) = ty {
                    self.emit(Instr::CheckType(t.clone()));
                }
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
            Stmt::TryCatch {
                try_block,
                catch_var,
                catch_block,
                ..
            } => {
                let push_handler_idx = self.emit(Instr::PushHandler(0)); // Placeholder
                
                self.compile_block(try_block);
                self.emit(Instr::PopHandler);
                let jump_after_catch = self.emit(Instr::Jump(0));
                
                // Catch block starts here
                let catch_start = self.code.len() as u32;
                // Patch PushHandler to point here
                self.patch_jump(push_handler_idx, catch_start);
                
                // Catch block expects error on stack. Store it in catch_var.
                self.emit(Instr::Store(catch_var.clone()));
                self.compile_block(catch_block);
                
                let after_catch = self.code.len() as u32;
                self.patch_jump(jump_after_catch, after_catch);
            }
            Stmt::Throw { expr, .. } => {
                self.compile_expr(expr);
                self.emit(Instr::Throw);
            }
            Stmt::ExprStmt { expr, .. } => {
                self.compile_expr(expr);
                self.emit(Instr::Pop);
            }
            Stmt::StructDef { .. } => {
                // Struct definition is purely static info for now. No runtime effect.
                // Or we could register it in runtime if we want runtime type tags.
                // For now, no-op in bytecode.
            }
            Stmt::ImplDef { methods, .. } => {
                // Compile methods so they exist as functions in the scope
                for method in methods {
                    self.compile_stmt(method);
                }
            }
            Stmt::TypeAlias { .. } => {
                // Type aliases are static-only.
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
            Expr::Literal { value, .. } => match value {
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
            Expr::Id { name, .. } => {
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
            Expr::Use { module, .. } => {
                self.compile_expr(module);
                self.emit(Instr::LoadModule);
            }
            Expr::Turn { params, ret_ty: _, body, .. } => {
                let jump_over = self.emit(Instr::Jump(0));
                let start_addr = self.code.len() as u32;

                // Parameter type checks
                // Assumes arguments are already in env (via CallTool)
                for (name, _, ty) in params {
                    if let Some(t) = ty {
                        self.emit(Instr::Load(name.clone()));
                        self.emit(Instr::CheckType(t.clone()));
                        self.emit(Instr::Pop); // Discard value, just check
                    }
                }

                self.compile_block(body);
                // Implicit return
                let has_return = body.stmts.last().map_or(false, |s| matches!(s, Stmt::Return { .. }));
                if !has_return {
                    self.emit(Instr::PushNull);
                    self.emit(Instr::Return);
                }
                let after_addr = self.code.len() as u32;
                self.patch_jump(jump_over, after_addr);
                self.emit(Instr::MakeTurn(start_addr));
            }
            Expr::Index { target, index, .. } => {
                self.compile_expr(target);
                self.compile_expr(index);
                self.emit(Instr::Index);
            }
            Expr::List { items, .. } => {
                let len = items.len();
                for item in items {
                    self.compile_expr(item);
                }
                self.emit(Instr::MakeList(len));
            }
            Expr::Vec { items, .. } => {
                let len = items.len();
                for item in items {
                    self.compile_expr(item);
                }
                self.emit(Instr::MakeVec(len));
            }
            Expr::Map { entries, .. } => {
                let len = entries.len();
                for (key, val) in entries {
                    self.emit(Instr::PushStr(key.clone()));
                    self.compile_expr(val);
                }
                self.emit(Instr::MakeMap(len));
            }
            Expr::Binary { op, left, right, .. } => {
                self.compile_expr(left);
                self.compile_expr(right);
                match op {
                    BinOp::Add => {
                        self.emit(Instr::Add);
                    }
                    BinOp::Mul => {
                        self.emit(Instr::Mul);
                    }
                    BinOp::Eq => {
                        self.emit(Instr::Eq);
                    }
                    BinOp::Ne => {
                        self.emit(Instr::Ne);
                    }
                    BinOp::And => {
                        self.emit(Instr::And);
                    }
                    BinOp::Or => {
                        self.emit(Instr::Or);
                    }
                    BinOp::Similarity => {
                        self.emit(Instr::Similarity);
                    }
                }
            }
            Expr::Unary { op, expr, .. } => {
                self.compile_expr(expr);
                match op {
                    UnOp::Not => {
                        self.emit(Instr::Not);
                    }
                    UnOp::Neg => {
                        // Negation is mul by -1? Or Instr::Neg?
                        // For simplicity, let's use Mul -1
                        self.emit(Instr::PushNum(-1.0));
                        self.emit(Instr::Mul);
                    }
                }
            }
            Expr::Spawn { expr, .. } => {
                self.compile_expr(expr);
                self.emit(Instr::Spawn);
            }
            Expr::Send { pid, msg, .. } => {
                self.compile_expr(pid);
                self.compile_expr(msg);
                self.emit(Instr::Send);
            }
            Expr::Receive { .. } => {
                self.emit(Instr::Receive);
            }
            Expr::Confidence { expr, .. } => {
                self.compile_expr(expr);
                self.emit(Instr::Confidence);
            }
            Expr::Paren(inner) => self.compile_expr(inner),
            Expr::StructInit { name, fields, .. } => {
                // Compile as Struct creation
                let len = fields.len();
                for (key, val) in fields {
                    self.emit(Instr::PushStr(key.clone()));
                    self.compile_expr(val);
                }
                self.emit(Instr::MakeStruct(name.clone(), len));
            }
            Expr::MethodCall { target, name, arg, .. } => {
                // Lower `obj.method(arg)` to `call(method, arg)` logic.
                // If `arg` is Null (no args), pass `target` as the argument.
                // If `arg` is present, pass `arg` (and `target` is effectively ignored by current runtime, 
                // unless we implement multi-arg calling convention or `self`).
                self.emit(Instr::Load(name.clone()));
                
                // Check if arg is literal Null (from parser empty parens)
                let is_null_arg = matches!(**arg, Expr::Literal { value: Literal::Null, .. });
                
                if is_null_arg {
                    self.compile_expr(target);
                } else {
                    self.compile_expr(arg);
                }
                
                self.emit(Instr::CallTool);
            }
        }
    }
}
