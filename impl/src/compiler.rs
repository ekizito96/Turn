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
            Instr::MatchResult(ref mut t) => *t = target,
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
            Stmt::Let { name, ty, init, is_persistent, .. } => {
                self.compile_expr(init);
                if let Some(t) = ty {
                    self.emit(Instr::CheckType(t.clone()));
                }
                self.emit(Instr::Store(name.clone()));
                if *is_persistent {
                    self.emit(Instr::Load(name.clone()));
                    self.emit(Instr::StorePersist(name.clone()));
                }
            }
            Stmt::Assign { target, value, .. } => {
                self.compile_expr(value);
                if let Expr::Id { name, .. } = target {
                    self.emit(Instr::Store(name.clone()));
                } else {
                    panic!("Compiler error: Reassignment currently only supports simple identifiers");
                }
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
            Stmt::Match {
                expr,
                ok_binding,
                ok_block,
                err_binding,
                err_block,
                ..
            } => {
                // Compile the target expression. It leaves Result(Ok | Err) on stack
                self.compile_expr(expr);

                // Emits a conditional jump. If stack top is Ok(v), it unwraps `v` and falls through.
                // If stack top is Err(e), it unwraps `e` and jumps to the target offset.
                let match_instr = self.emit(Instr::MatchResult(0));

                // Fallthrough (Ok path)
                self.emit(Instr::Store(ok_binding.clone()));
                self.compile_block(ok_block);
                let jump_to_end = self.emit(Instr::Jump(0));

                // Err path
                let err_start = self.code.len() as u32;
                self.patch_jump(match_instr, err_start);
                self.emit(Instr::Store(err_binding.clone()));
                self.compile_block(err_block);

                let end = self.code.len() as u32;
                self.patch_jump(jump_to_end, end);
            }
            Stmt::ExprStmt { expr, .. } => {
                self.compile_expr(expr);
                self.emit(Instr::Pop);
            }
            Stmt::StructDef { name, fields, .. } => {
                self.emit(Instr::DefineStruct(name.clone(), fields.clone()));
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
            Stmt::TestDef { name: _, mocks, body, .. } => {
                // Compile test body. During Phase 5, if `MockStart` is encountered, it registers
                // the struct overrides in the current VM Frame's env to intercept `Infer` calls.
                for mock in mocks {
                    self.compile_expr(&mock.mock_value);
                    self.emit(Instr::MockDef(mock.target_ty.clone()));
                }
                
                self.compile_block(body);
                
                // Clear mocks from environment at the end
                self.emit(Instr::MockClear);
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
            Expr::Ok(inner, _) => {
                self.compile_expr(inner);
                self.emit(Instr::MakeOk);
            }
            Expr::Err(inner, _) => {
                self.compile_expr(inner);
                self.emit(Instr::MakeErr);
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

            Expr::UseWasm { url, .. } => {
                self.emit(Instr::PushStr("sys_wasm_adapter".to_string()));
                self.emit(Instr::PushStr("url".to_string()));
                self.compile_expr(url);
                self.emit(Instr::MakeMap(1));
                self.emit(Instr::CallTool);
            }
            Expr::Turn {
                is_tool,
                params,
                ret_ty: _,
                body,
                ..
            } => {
                let jump_over = self.emit(Instr::Jump(0));
                let start_addr = self.code.len() as u32;

                // Parameter type checks
                // Assumes arguments are already in env (via CallTool)
                for (name, _, ty, _) in params {
                    if let Some(t) = ty {
                        self.emit(Instr::Load(name.clone()));
                        self.emit(Instr::CheckType(t.clone()));
                        self.emit(Instr::Pop); // Discard value, just check
                    }
                }

                self.compile_block(body);
                // Implicit return
                let has_return = body
                    .stmts
                    .last()
                    .is_some_and(|s| matches!(s, Stmt::Return { .. }));
                if !has_return {
                    self.emit(Instr::PushNull);
                    self.emit(Instr::Return);
                }
                let after_addr = self.code.len() as u32;
                self.patch_jump(jump_over, after_addr);
                let param_info = params
                    .iter()
                    .map(|(n, _, ty, is_sec)| (n.clone(), ty.clone(), *is_sec))
                    .collect();
                self.emit(Instr::MakeTurn(start_addr, *is_tool, param_info));
            }
            Expr::Infer {
                target_ty,
                tools,
                body,
                driver,
                threshold,
                fallback,
                ..
            } => {
                let tool_count = tools.as_ref().map(|t| t.len()).unwrap_or(0);

                // Compile body as an expression (leave result on stack, acts as the prompt)
                let len = body.stmts.len();
                if len == 0 {
                    self.emit(Instr::PushNull);
                } else {
                    for (i, stmt) in body.stmts.iter().enumerate() {
                        if i == len - 1 {
                            match stmt {
                                Stmt::ExprStmt { expr, .. } => {
                                    self.compile_expr(expr);
                                    // Do NOT pop
                                }
                                _ => {
                                    self.compile_stmt(stmt);
                                    // Result is Null if stmt is not an expression
                                    self.emit(Instr::PushNull);
                                }
                            }
                        } else {
                            self.compile_stmt(stmt);
                        }
                    }
                }

                // Compile tools AFTER body, so they sit on top of the prompt in the stack
                if let Some(ts) = tools {
                    for t in ts {
                        self.compile_expr(t);
                    }
                }

                let has_driver = if let Some(drv) = driver {
                    self.compile_expr(drv);
                    true
                } else {
                    false
                };

                let has_threshold = if let Some(thresh) = threshold {
                    self.compile_expr(thresh);
                    true
                } else {
                    false
                };

                let infer_idx = self.emit(Instr::Infer(
                    target_ty.clone(),
                    tool_count as u32,
                    has_driver,
                    has_threshold,
                    0, // fallback offset placeholder
                ));

                if let Some(fb) = fallback {
                    let jump_end_idx = self.emit(Instr::Jump(0)); // jump over fallback block on success
                    let jump_target = self.code.len() as u32;
                    if let Instr::Infer(_, _, _, _, ref mut offset) = self.code[infer_idx as usize] {
                        *offset = jump_target; // Jump here if inferior/confident fails
                    }
                    self.compile_block(fb);
                    self.code[jump_end_idx as usize] = Instr::Jump(self.code.len() as u32);
                }
            }
            Expr::Budget {
                tokens,
                time,
                body,
                ..
            } => {
                if let Some(t) = time {
                    self.compile_expr(t);
                } else {
                    self.emit(Instr::PushNull);
                }
                if let Some(toks) = tokens {
                    self.compile_expr(toks);
                } else {
                    self.emit(Instr::PushNull);
                }
                self.emit(Instr::PushBudget);

                let len = body.stmts.len();
                if len == 0 {
                    self.emit(Instr::PushNull);
                } else {
                    for (i, stmt) in body.stmts.iter().enumerate() {
                        if i == len - 1 {
                            match stmt {
                                Stmt::ExprStmt { expr, .. } => {
                                    self.compile_expr(expr);
                                }
                                _ => {
                                    self.compile_stmt(stmt);
                                    self.emit(Instr::PushNull);
                                }
                            }
                        } else {
                            self.compile_stmt(stmt);
                        }
                    }
                }

                self.emit(Instr::PopBudget);
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
            Expr::Trace { pid_expr, span: _ } => {
                self.compile_expr(pid_expr);
                self.emit(Instr::TraceProcess);
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
            Expr::Binary {
                op, left, right, ..
            } => {
                self.compile_expr(left);
                self.compile_expr(right);
                match op {
                    BinOp::Add => {
                        self.emit(Instr::Add);
                    }
                    BinOp::Sub => {
                        self.emit(Instr::Sub);
                    }
                    BinOp::Mul => {
                        self.emit(Instr::Mul);
                    }
                    BinOp::Div => {
                        self.emit(Instr::Div);
                    }
                    BinOp::Eq => {
                        self.emit(Instr::Eq);
                    }
                    BinOp::Ne => {
                        self.emit(Instr::Ne);
                    }
                    BinOp::Lt => {
                        self.emit(Instr::Lt);
                    }
                    BinOp::Gt => {
                        self.emit(Instr::Gt);
                    }
                    BinOp::Le => {
                        self.emit(Instr::Le);
                    }
                    BinOp::Ge => {
                        self.emit(Instr::Ge);
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
            Expr::Spawn { expr, linked, monitored, .. } => {
                self.compile_expr(expr);
                self.emit(Instr::Spawn(*linked, *monitored));
            }
            Expr::SpawnRemote {
                node_id, closure, ..
            } => {
                self.compile_expr(node_id);
                self.compile_expr(closure);
                self.emit(Instr::SpawnRemote);
            }
            Expr::Send { pid, msg, .. } => {
                self.compile_expr(pid);
                self.compile_expr(msg);
                self.emit(Instr::Send);
            }
            Expr::Receive { is_blocking, .. } => {
                self.emit(Instr::Receive(*is_blocking));
            }
            Expr::Harvest { .. } => {
                self.emit(Instr::Harvest);
            }
            Expr::Suspend { expected_type, msg, .. } => {
                self.compile_expr(msg);
                self.emit(Instr::Suspend(expected_type.clone()));
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
            Expr::MethodCall {
                target, name, arg, ..
            } => {
                self.compile_expr(target);
                self.compile_expr(arg);
                self.emit(Instr::CallMethod(name.clone()));
            }
        }
    }
}
