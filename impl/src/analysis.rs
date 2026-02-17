use crate::ast::*;
use crate::lexer::Span;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Scope {
    pub definitions: HashMap<String, (Span, Option<Type>)>,
    pub parent: Option<usize>,
    pub span: Span, // The span covered by this scope
}

#[derive(Debug)]
pub struct Analysis {
    pub scopes: Vec<Scope>,
    pub active_scope_idx: usize,
    /// Maps usage span to definition span
    pub usages: HashMap<usize, Span>, // Key is usage.start (assuming unique starts for simplicity)
}

impl Analysis {
    pub fn new() -> Self {
        Self {
            scopes: vec![Scope {
                definitions: HashMap::new(),
                parent: None,
                span: Span { start: 0, end: usize::MAX },
            }],
            active_scope_idx: 0,
            usages: HashMap::new(),
        }
    }

    pub fn analyze(&mut self, program: &Program) {
        for stmt in &program.stmts {
            self.visit_stmt(stmt);
        }
    }

    fn enter_scope(&mut self, span: Span) {
        let new_scope_idx = self.scopes.len();
        self.scopes.push(Scope {
            definitions: HashMap::new(),
            parent: Some(self.active_scope_idx),
            span,
        });
        self.active_scope_idx = new_scope_idx;
    }

    fn exit_scope(&mut self) {
        if let Some(parent) = self.scopes[self.active_scope_idx].parent {
            self.active_scope_idx = parent;
        }
    }

    fn add_definition(&mut self, name: &str, span: Span, ty: Option<Type>) {
        self.scopes[self.active_scope_idx].definitions.insert(name.to_string(), (span, ty));
    }

    fn record_usage(&mut self, name: &str, span: Span) {
        let mut current_idx = Some(self.active_scope_idx);
        while let Some(idx) = current_idx {
            if let Some((def_span, _)) = self.scopes[idx].definitions.get(name) {
                self.usages.insert(span.start, *def_span);
                return;
            }
            current_idx = self.scopes[idx].parent;
        }
    }

    fn visit_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let { name, ty, init, span } => {
                self.visit_expr(init);
                self.add_definition(name, *span, ty.clone());
            }
            Stmt::Turn { body, .. } => {
                // Turn is an expression usually, but here it's a statement (expression statement?)
                // Wait, Stmt::Turn is deprecated/legacy? 
                // In `ast.rs`: `Stmt::Turn { body, span }` exists.
                // `Expr::Turn` also exists.
                // Let's handle it.
                self.enter_scope(body.span);
                self.visit_block(body);
                self.exit_scope();
            }
            Stmt::If { cond, then_block, else_block, .. } => {
                self.visit_expr(cond);
                // If/Else blocks don't create new scope in current VM, but let's pretend they do for cleaner future?
                // No, sticking to VM reality: they share scope.
                self.visit_block(then_block);
                if let Some(b) = else_block {
                    self.visit_block(b);
                }
            }
            Stmt::While { cond, body, .. } => {
                self.visit_expr(cond);
                self.visit_block(body);
            }
            Stmt::TryCatch { try_block, catch_var, catch_block, span: _ } => {
                self.visit_block(try_block);
                
                // Catch block definitely needs a new scope for the catch_var
                self.enter_scope(catch_block.span);
                // We don't have span for catch_var ID. Use block start?
                let var_span = Span { start: catch_block.span.start, end: catch_block.span.start }; 
                self.add_definition(catch_var, var_span, Some(Type::Any)); // Catch var is Any (usually Error string)
                self.visit_block(catch_block);
                self.exit_scope();
            }
            Stmt::Return { expr, .. } => self.visit_expr(expr),
            Stmt::ExprStmt { expr, .. } => self.visit_expr(expr),
            Stmt::ContextAppend { expr, .. } => self.visit_expr(expr),
            Stmt::Remember { key, value, .. } => {
                self.visit_expr(key);
                self.visit_expr(value);
            }
            Stmt::CallStmt { tool, arg, .. } => {
                self.visit_expr(tool);
                self.visit_expr(arg);
            }
            Stmt::Throw { expr, .. } => self.visit_expr(expr),
        }
    }

    fn visit_block(&mut self, block: &Block) {
        for stmt in &block.stmts {
            self.visit_stmt(stmt);
        }
    }

    fn visit_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Id { name, span } => {
                self.record_usage(name, *span);
            }
            Expr::Turn { params, body, span, .. } => {
                self.enter_scope(*span);
                for (name, param_span, ty) in params {
                     self.add_definition(name, *param_span, ty.clone());
                }
                self.visit_block(body);
                self.exit_scope();
            }
            Expr::Binary { left, right, .. } => {
                self.visit_expr(left);
                self.visit_expr(right);
            }
            Expr::Unary { expr, .. } => self.visit_expr(expr),
            Expr::Paren(expr) => self.visit_expr(expr),
            Expr::Call { name, arg, .. } => {
                self.visit_expr(name);
                self.visit_expr(arg);
            }
            Expr::Recall { key, .. } => self.visit_expr(key),
            Expr::Use { module, .. } => self.visit_expr(module),
            Expr::Index { target, index, .. } => {
                self.visit_expr(target);
                self.visit_expr(index);
            }
            Expr::List { items, .. } => {
                for item in items {
                    self.visit_expr(item);
                }
            }
            Expr::Map { entries, .. } => {
                for (_, val) in entries {
                    self.visit_expr(val);
                }
            }
            Expr::Literal { .. } => {}
        }
    }

    /// Find the scope that covers the given offset
    pub fn find_scope(&self, offset: usize) -> Option<&Scope> {
        // Find the deepest scope that contains the offset
        let mut best_match: Option<&Scope> = None;
        for scope in &self.scopes {
            if offset >= scope.span.start && offset <= scope.span.end {
                match best_match {
                    None => best_match = Some(scope),
                    Some(prev) => {
                        // If this scope is smaller/deeper than prev, it's a better match
                        if (scope.span.end - scope.span.start) < (prev.span.end - prev.span.start) {
                            best_match = Some(scope);
                        }
                    }
                }
            }
        }
        best_match
    }

    pub fn completion_items(&self, offset: usize) -> Vec<String> {
        let mut items = Vec::new();
        // Find scope chain
        if let Some(start_scope) = self.find_scope(offset) {
            // Walk up scope chain
            // We need to find scope by reference or index. 
            // My find_scope returns reference, but I need to walk up parents which are indices.
            // Let's change find_scope to return index.
            
            // Re-implement logic inline for now or fix helper
            let mut best_idx = 0;
            let mut found = false;
            for (i, scope) in self.scopes.iter().enumerate() {
                if offset >= scope.span.start && offset <= scope.span.end {
                     if !found || (scope.span.end - scope.span.start) < (self.scopes[best_idx].span.end - self.scopes[best_idx].span.start) {
                         best_idx = i;
                         found = true;
                     }
                }
            }
            
            if found {
                let mut curr_idx = Some(best_idx);
                while let Some(idx) = curr_idx {
                    let scope = &self.scopes[idx];
                    for (name, (_, ty)) in &scope.definitions {
                        // We can return type info too! But signature is Vec<String>.
                        // For now just name.
                        items.push(name.clone());
                        // TODO: Use ty for better completion icons/details
                        let _ = ty; 
                    }
                    curr_idx = scope.parent;
                }
            }
            
            // Avoid unused variable warning for start_scope
            let _ = start_scope;
        }
        items
    }
}
