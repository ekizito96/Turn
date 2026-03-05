use crate::ast::*;
use crate::lexer::Span;
use indexmap::IndexMap;
use std::collections::HashMap;

// --- Standard Library Signatures ---
fn get_stdlib_signatures() -> HashMap<String, Type> {
    let mut map = HashMap::new();
    // fs_read: (Str) -> Str
    map.insert(
        "fs_read".to_string(),
        Type::Function(Box::new(Type::Str), Box::new(Type::Str)),
    );
    // fs_write: (Map<Str>) -> Void
    map.insert(
        "fs_write".to_string(),
        Type::Function(
            Box::new(Type::Map(Box::new(Type::Str))),
            Box::new(Type::Void),
        ),
    );
    // echo: (Any) -> Any
    map.insert(
        "echo".to_string(),
        Type::Function(Box::new(Type::Any), Box::new(Type::Any)),
    );
    // sleep: (Num) -> Void
    map.insert(
        "sleep".to_string(),
        Type::Function(Box::new(Type::Num), Box::new(Type::Void)),
    );
    // env_get: (Str) -> Str (or Null, essentially Str/Null but we treat as Str)
    map.insert(
        "env_get".to_string(),
        Type::Function(Box::new(Type::Str), Box::new(Type::Str)),
    );
    // env_set: (Map<Str>) -> Void
    map.insert(
        "env_set".to_string(),
        Type::Function(
            Box::new(Type::Map(Box::new(Type::Str))),
            Box::new(Type::Void),
        ),
    );
    // http_get: (Str) -> Str
    map.insert(
        "http_get".to_string(),
        Type::Function(Box::new(Type::Str), Box::new(Type::Str)),
    );
    // http_post: (Map<Any>) -> Str
    map.insert(
        "http_post".to_string(),
        Type::Function(
            Box::new(Type::Map(Box::new(Type::Any))),
            Box::new(Type::Str),
        ),
    );
    // json_parse: (Str) -> Any
    map.insert(
        "json_parse".to_string(),
        Type::Function(Box::new(Type::Str), Box::new(Type::Any)),
    );
    // json_stringify: (Any) -> Str
    map.insert(
        "json_stringify".to_string(),
        Type::Function(Box::new(Type::Any), Box::new(Type::Str)),
    );
    // fs_list: (Str) -> List<Str>
    map.insert(
        "fs_list".to_string(),
        Type::Function(
            Box::new(Type::Str),
            Box::new(Type::List(Box::new(Type::Str))),
        ),
    );
    // time_now: (Any) -> Num
    map.insert(
        "time_now".to_string(),
        Type::Function(Box::new(Type::Any), Box::new(Type::Num)),
    );
    // regex_match: (Map<Str>) -> Bool
    map.insert(
        "regex_match".to_string(),
        Type::Function(
            Box::new(Type::Map(Box::new(Type::Str))),
            Box::new(Type::Bool),
        ),
    );
    // regex_replace: (Map<Str>) -> Str
    map.insert(
        "regex_replace".to_string(),
        Type::Function(
            Box::new(Type::Map(Box::new(Type::Str))),
            Box::new(Type::Str),
        ),
    );
    // list_map: (List<Any>, Function) -> List<Any>
    map.insert(
        "list_map".to_string(),
        Type::Function(
            Box::new(Type::List(Box::new(Type::Any))),
            Box::new(Type::List(Box::new(Type::Any))), // We will use Any for now as we don't have generics properly
        ),
    );
    // list_filter: (List<Any>, Function) -> List<Any>
    map.insert(
        "list_filter".to_string(),
        Type::Function(
            Box::new(Type::List(Box::new(Type::Any))),
            Box::new(Type::List(Box::new(Type::Any))),
        ),
    );
    map
}

#[derive(Debug, Clone)]
pub struct Scope {
    pub definitions: HashMap<String, (Span, Option<Type>)>,
    pub parent: Option<usize>,
    pub span: Span,           // The span covered by this scope
    pub ret_ty: Option<Type>, // Return type if this is a function scope
    pub structs: HashMap<String, IndexMap<String, Type>>, // Struct definitions
    pub type_aliases: HashMap<String, Type>, // Type aliases
    pub methods: HashMap<String, HashMap<String, (Span, Option<Type>)>>, // Type -> Method -> Signature
}

#[derive(Debug)]
pub struct Analysis {
    pub scopes: Vec<Scope>,
    pub active_scope_idx: usize,
    /// Maps usage span to definition span
    pub usages: HashMap<usize, Span>, // Key is usage.start (assuming unique starts for simplicity)
    pub diagnostics: Vec<(Span, String)>,
}

impl Default for Analysis {
    fn default() -> Self {
        Self::new()
    }
}

impl Analysis {
    pub fn new() -> Self {
        let mut global_scope = Scope {
            definitions: HashMap::new(),
            parent: None,
            span: Span {
                start: 0,
                end: usize::MAX,
            },
            ret_ty: None,
            structs: HashMap::new(),
            type_aliases: HashMap::new(),
            methods: HashMap::new(),
        };

        // Add stdlib signatures
        for (name, ty) in get_stdlib_signatures() {
            global_scope
                .definitions
                .insert(name, (Span { start: 0, end: 0 }, Some(ty)));
        }

        Self {
            scopes: vec![global_scope],
            active_scope_idx: 0,
            usages: HashMap::new(),
            diagnostics: Vec::new(),
        }
    }

    pub fn analyze(&mut self, program: &Program) {
        for stmt in &program.stmts {
            self.visit_stmt(stmt);
        }
    }

    fn enter_scope(&mut self, span: Span, ret_ty: Option<Type>) {
        let new_scope_idx = self.scopes.len();
        self.scopes.push(Scope {
            definitions: HashMap::new(),
            parent: Some(self.active_scope_idx),
            span,
            ret_ty,
            structs: HashMap::new(),
            type_aliases: HashMap::new(),
            methods: HashMap::new(),
        });
        self.active_scope_idx = new_scope_idx;
    }

    fn exit_scope(&mut self) {
        if let Some(parent) = self.scopes[self.active_scope_idx].parent {
            self.active_scope_idx = parent;
        }
    }

    fn add_definition(&mut self, name: &str, span: Span, ty: Option<Type>) {
        self.scopes[self.active_scope_idx]
            .definitions
            .insert(name.to_string(), (span, ty));
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

    fn find_struct(&self, name: &str) -> Option<IndexMap<String, Type>> {
        let mut current_idx = Some(self.active_scope_idx);
        while let Some(idx) = current_idx {
            if let Some(fields) = self.scopes[idx].structs.get(name) {
                return Some(fields.clone());
            }
            current_idx = self.scopes[idx].parent;
        }
        None
    }

    fn find_alias(&self, name: &str) -> Option<Type> {
        let mut current_idx = Some(self.active_scope_idx);
        while let Some(idx) = current_idx {
            if let Some(ty) = self.scopes[idx].type_aliases.get(name) {
                return Some(ty.clone());
            }
            current_idx = self.scopes[idx].parent;
        }
        None
    }

    fn find_method(&self, type_name: &str, method_name: &str) -> Option<(Span, Option<Type>)> {
        let mut current_idx = Some(self.active_scope_idx);
        while let Some(idx) = current_idx {
            if let Some(methods) = self.scopes[idx].methods.get(type_name) {
                if let Some(sig) = methods.get(method_name) {
                    return Some(sig.clone());
                }
            }
            current_idx = self.scopes[idx].parent;
        }
        None
    }

    fn resolve_type(&self, ty: &Type) -> Type {
        match ty {
            Type::Struct(name, fields) if fields.is_empty() => {
                // Check aliases
                if let Some(alias) = self.find_alias(name) {
                    return self.resolve_type(&alias);
                }
                // Check struct definitions
                if let Some(def_fields) = self.find_struct(name) {
                    return Type::Struct(name.clone(), def_fields);
                }
                ty.clone()
            }
            Type::List(inner) => Type::List(Box::new(self.resolve_type(inner))),
            Type::Map(inner) => Type::Map(Box::new(self.resolve_type(inner))),
            Type::Function(arg, ret) => Type::Function(
                Box::new(self.resolve_type(arg)),
                Box::new(self.resolve_type(ret)),
            ),
            _ => ty.clone(),
        }
    }

    fn check_assignment(&mut self, target_ty: &Option<Type>, expr: &Expr, span: Span) {
        if let Some(target) = target_ty {
            let resolved_target = self.resolve_type(target);
            if resolved_target == Type::Any {
                return;
            }

            if let Some(expr_ty) = self.infer_expr_type(expr) {
                let resolved_expr = self.resolve_type(&expr_ty);
                // If either is Any, we allow it.
                if !self.is_compatible(&resolved_target, &resolved_expr) {
                    self.diagnostics.push((
                        span,
                        format!(
                            "Type mismatch: expected {:?}, got {:?}",
                            resolved_target, resolved_expr
                        ),
                    ));
                }
            }
        }
    }
    fn infer_expr_type(&self, expr: &Expr) -> Option<Type> {
        match expr {
            Expr::Spawn { .. } => Some(Type::Pid),
            Expr::SpawnLink { .. } => Some(Type::Pid),
            Expr::Send { .. } => Some(Type::Bool),
            Expr::Receive { .. } => Some(Type::Any),
            Expr::Confidence { .. } => Some(Type::Num),
            Expr::Grant { .. } => Some(Type::Identity),
            Expr::Infer { target_ty, .. } => Some(target_ty.clone()),
            Expr::Vec { items, .. } => {
                // Infer type as Vec.
                // Assuming Vec<Num>.
                // Check all items are Num.
                for item in items {
                    let ty = self.infer_expr_type(item);
                    if let Some(t) = ty {
                        if !self.is_compatible(&Type::Num, &t) {
                            // Mismatch
                        }
                    }
                }
                Some(Type::Vec)
            }
            Expr::StructInit { name, fields, .. } => {
                // Return struct type if defined
                if let Some(def_fields) = self.find_struct(name) {
                    // Check fields match
                    // 1. Check if all required fields are present
                    for (field_name, field_ty) in &def_fields {
                        if let Some(init_expr) = fields.get(field_name) {
                            let init_ty = self.infer_expr_type(init_expr);
                            if let Some(ty) = init_ty {
                                if !self.is_compatible(field_ty, &ty) {
                                    // Diagnostics handled in visit_expr? No, infer doesn't add diagnostics generally.
                                    // But StructInit is an expression. We should validate it here or in visit_expr.
                                    // Usually validation happens in visit_*.
                                }
                            }
                        } else {
                            // Missing field
                        }
                    }
                    return Some(Type::Struct(name.clone(), def_fields));
                }
                None // Unknown struct
            }
            Expr::Literal { value, .. } => match value {
                Literal::Num(_) => Some(Type::Num),
                Literal::Str(_) => Some(Type::Str),
                Literal::True | Literal::False => Some(Type::Bool),
                Literal::Null => Some(Type::Any), // Null can be anything? Or specific Null type?
            },
            Expr::List { items, .. } => {
                // Infer type from first item? Or common type?
                // For now, let's assume List<Any> or try to infer from first element.
                // If empty, List<Any>.
                if let Some(first) = items.first() {
                    if let Some(inner) = self.infer_expr_type(first) {
                        return Some(Type::List(Box::new(inner)));
                    }
                }
                Some(Type::List(Box::new(Type::Any)))
            }
            Expr::Map { entries, .. } => {
                // Similar logic for Map values
                if let Some((_, first_val)) = entries.first() {
                    if let Some(inner) = self.infer_expr_type(first_val) {
                        return Some(Type::Map(Box::new(inner)));
                    }
                }
                Some(Type::Map(Box::new(Type::Any)))
            }
            Expr::Id { name, .. } => {
                let mut current_idx = Some(self.active_scope_idx);
                while let Some(idx) = current_idx {
                    if let Some((_, ty)) = self.scopes[idx].definitions.get(name) {
                        return ty.clone();
                    }
                    current_idx = self.scopes[idx].parent;
                }
                None
            }
            Expr::Binary {
                op, left, right, ..
            } => {
                let left_ty = self.infer_expr_type(left);
                let right_ty = self.infer_expr_type(right);

                match op {
                    BinOp::Add => {
                        if left_ty == Some(Type::Num) && right_ty == Some(Type::Num) {
                            Some(Type::Num)
                        } else if left_ty == Some(Type::Str) || right_ty == Some(Type::Str) {
                            Some(Type::Str) // String concatenation
                        } else if left_ty == Some(Type::Vec) && right_ty == Some(Type::Vec) {
                            Some(Type::Vec) // Vector addition
                        } else {
                            // If either is Any, result is Any (or Num if we are strict?)
                            // For gradual typing, Any propagates.
                            if left_ty == Some(Type::Any) || right_ty == Some(Type::Any) {
                                Some(Type::Any)
                            } else {
                                None // Unknown or mismatch
                            }
                        }
                    }
                    BinOp::Sub => {
                        if left_ty == Some(Type::Num) && right_ty == Some(Type::Num) {
                            Some(Type::Num)
                        } else if left_ty == Some(Type::Vec) && right_ty == Some(Type::Vec) {
                            Some(Type::Vec)
                        } else if left_ty == Some(Type::Any) || right_ty == Some(Type::Any) {
                            Some(Type::Any)
                        } else {
                            None
                        }
                    }
                    BinOp::Div => {
                        if left_ty == Some(Type::Num) && right_ty == Some(Type::Num) {
                            Some(Type::Num)
                        } else if left_ty == Some(Type::Any) || right_ty == Some(Type::Any) {
                            Some(Type::Any)
                        } else {
                            None
                        }
                    }
                    BinOp::Mul => {
                        if left_ty == Some(Type::Num) && right_ty == Some(Type::Num) {
                            Some(Type::Num)
                        } else if (left_ty == Some(Type::Vec) && right_ty == Some(Type::Num))
                            || (left_ty == Some(Type::Num) && right_ty == Some(Type::Vec))
                        {
                            Some(Type::Vec) // Scalar multiplication
                        } else if left_ty == Some(Type::Vec) && right_ty == Some(Type::Vec) {
                            Some(Type::Num) // Dot product
                        } else if left_ty == Some(Type::Any) || right_ty == Some(Type::Any) {
                            Some(Type::Any)
                        } else {
                            None
                        }
                    }
                    BinOp::Similarity => Some(Type::Num),
                    BinOp::Eq
                    | BinOp::Ne
                    | BinOp::Lt
                    | BinOp::Gt
                    | BinOp::Le
                    | BinOp::Ge
                    | BinOp::And
                    | BinOp::Or => Some(Type::Bool),
                }
            }
            Expr::Turn { params, ret_ty, .. } => {
                // Infer type as Function(Map<Any>, ret_ty)
                // Wait, params are (name, span, ty).
                // Function type takes a single Type for arg?
                // Our tools take a single Value (often Map).
                // Turn functions take named arguments, which means the caller passes a Map or single value.
                // If 1 param, single value. If >1, Map.
                // For simplicity, let's say the argument type is Map<Any> if > 1 param.
                // Or if 1 param `x: Num`, arg type is `Num`.

                let arg_ty = if params.len() == 1 {
                    params[0].2.clone().unwrap_or(Type::Any)
                } else if params.is_empty() {
                    Type::Void
                } else {
                    // Multiple params = Map<Any> (can't specify keys yet)
                    Type::Map(Box::new(Type::Any))
                };

                let ret = ret_ty.clone().unwrap_or(Type::Any);
                Some(Type::Function(Box::new(arg_ty), Box::new(ret)))
            }
            Expr::Call { name, .. } => {
                // Try to find the function definition
                // This requires more complex analysis (looking up the ID, checking if it's a Turn expr)
                // For now, let's assume Any or try to look up if it's a simple ID.
                if let Expr::Id {
                    name: func_name, ..
                } = &**name
                {
                    // Look up definition
                    let mut current_idx = Some(self.active_scope_idx);
                    while let Some(idx) = current_idx {
                        if let Some((_, ty)) = self.scopes[idx].definitions.get(func_name) {
                            // If it's a function type, return return type
                            if let Some(Type::Function(_, ret_ty)) = ty {
                                return Some(*ret_ty.clone());
                            }
                            // Else if it's Any, return Any
                            return ty.clone();
                        }
                        current_idx = self.scopes[idx].parent;
                    }
                }
                // Also check if name is a literal string (calling tool directly)
                if let Expr::Literal {
                    value: Literal::Str(tool_name),
                    ..
                } = &**name
                {
                    // We populated global scope with tool names too!
                    // But global scope has them as IDs, not strings.
                    // Wait, `get_stdlib_signatures` returns keys as strings.
                    // And we insert them into `definitions`.
                    // `definitions` keys are strings.
                    // But `Expr::Id` name is string.
                    // `Expr::Literal` value is string.
                    // We can look up the tool name in global scope!
                    if let Some((_, Some(Type::Function(_, ret_ty)))) =
                        self.scopes[0].definitions.get(tool_name)
                    {
                        return Some(*ret_ty.clone());
                    }
                }
                Some(Type::Any)
            }
            Expr::If {
                then_block: _,
                else_block: _,
                ..
            } => {
                // To be precise we should infer type of block. For now Any.
                Some(Type::Any)
            }
            _ => Some(Type::Any),
        }
    }

    fn is_compatible(&self, target: &Type, source: &Type) -> bool {
        if *target == Type::Any || *source == Type::Any {
            return true;
        }
        match (target, source) {
            (Type::Vec, Type::Vec) => true,
            (Type::List(t), Type::List(s)) => self.is_compatible(t, s),
            (Type::Map(t), Type::Map(s)) => self.is_compatible(t, s),
            (Type::Struct(name1, _), Type::Struct(name2, _)) => name1 == name2,
            _ => target == source,
        }
    }

    fn visit_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::TypeAlias { name, ty, span: _ } => {
                self.scopes[self.active_scope_idx]
                    .type_aliases
                    .insert(name.clone(), ty.clone());
            }
            Stmt::ImplDef {
                type_name,
                methods,
                span: _,
            } => {
                let mut collected: Vec<(String, Span, Option<Type>)> = Vec::new();
                for stmt in methods {
                    if let Stmt::Let {
                        name,
                        ty,
                        init,
                        span,
                    } = stmt
                    {
                        let method_ty = ty.clone().or_else(|| self.infer_expr_type(init));
                        collected.push((name.clone(), *span, method_ty));
                    }
                }

                let methods_map = self.scopes[self.active_scope_idx]
                    .methods
                    .entry(type_name.clone())
                    .or_default();
                for (name, span, ty) in collected {
                    methods_map.insert(name, (span, ty));
                }

                for stmt in methods {
                    self.visit_stmt(stmt);
                }
            }
            Stmt::Let {
                name,
                ty,
                init,
                span,
            } => {
                self.visit_expr(init);

                // Check type
                self.check_assignment(ty, init, *span);

                // If explicit type is missing, try to infer from init
                let stored_ty = ty.clone().or_else(|| self.infer_expr_type(init));

                self.add_definition(name, *span, stored_ty);
            }
            Stmt::Turn { body, .. } => {
                // Turn is an expression usually, but here it's a statement (expression statement?)
                // Wait, Stmt::Turn is deprecated/legacy?
                // In `ast.rs`: `Stmt::Turn { body, span }` exists.
                // `Expr::Turn` also exists.
                // Let's handle it.
                self.enter_scope(body.span, None);
                self.visit_block(body);
                self.exit_scope();
            }
            Stmt::While { cond, body, .. } => {
                self.visit_expr(cond);
                self.visit_block(body);
            }
            Stmt::TryCatch {
                try_block,
                catch_var,
                catch_block,
                span: _,
            } => {
                self.visit_block(try_block);

                // Catch block definitely needs a new scope for the catch_var
                self.enter_scope(catch_block.span, None);
                // We don't have span for catch_var ID. Use block start?
                let var_span = Span {
                    start: catch_block.span.start,
                    end: catch_block.span.start,
                };
                self.add_definition(catch_var, var_span, Some(Type::Any)); // Catch var is Any (usually Error string)
                self.visit_block(catch_block);
                self.exit_scope();
            }
            Stmt::StructDef {
                name,
                fields,
                span: _,
            } => {
                // Register struct in current scope
                self.scopes[self.active_scope_idx]
                    .structs
                    .insert(name.clone(), fields.clone());
            }
            Stmt::Return { expr, span } => {
                self.visit_expr(expr);

                // Check return type
                let mut expected_ty = None;
                let mut current_idx = Some(self.active_scope_idx);
                while let Some(idx) = current_idx {
                    if let Some(ty) = &self.scopes[idx].ret_ty {
                        expected_ty = Some(ty.clone());
                        break;
                    }
                    if idx == 0 {
                        break;
                    }
                    current_idx = self.scopes[idx].parent;
                }

                if let Some(expected) = expected_ty {
                    self.check_assignment(&Some(expected), expr, *span);
                }
            }
            Stmt::ExprStmt { expr, .. } => self.visit_expr(expr),
            Stmt::ContextAppend { expr, .. } => self.visit_expr(expr),
            Stmt::ContextSystem { expr, .. } => self.visit_expr(expr),
            Stmt::Remember { key, value, .. } => {
                self.visit_expr(key);
                self.visit_expr(value);
            }
            Stmt::Throw { expr, .. } => self.visit_expr(expr),
            Stmt::Suspend { .. } => {}
        }
    }

    fn visit_block(&mut self, block: &Block) {
        for stmt in &block.stmts {
            self.visit_stmt(stmt);
        }
    }

    fn visit_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::StructInit {
                name,
                fields,
                spread,
                span,
            } => {
                if let Some(def_fields) = self.find_struct(name) {
                    for (field_name, field_ty) in &def_fields {
                        if let Some(init_expr) = fields.get(field_name) {
                            self.visit_expr(init_expr);
                            self.check_assignment(&Some(field_ty.clone()), init_expr, *span);
                        } else if spread.is_none() {
                            self.diagnostics.push((
                                *span,
                                format!("Missing field '{}' in struct '{}'", field_name, name),
                            ));
                        }
                    }
                    // Check for extra fields
                    for field_name in fields.keys() {
                        if !def_fields.contains_key(field_name) {
                            self.diagnostics.push((
                                *span,
                                format!("Unknown field '{}' in struct '{}'", field_name, name),
                            ));
                        }
                    }
                    if let Some(spread_expr) = spread {
                        self.visit_expr(spread_expr);
                    }
                } else {
                    self.diagnostics
                        .push((*span, format!("Unknown struct '{}'", name)));
                }
            }
            Expr::Spawn { expr, span: _ } | Expr::SpawnLink { expr, span: _ } => {
                self.visit_expr(expr);
                // Expected: Function
                // But could be Any.
                // We should check that expr is a Function type or Any.
                // For now, no strict check enforced by diagnostics unless we want to be strict.
                let ty = self.infer_expr_type(expr);
                if let Some(t) = ty {
                    if !matches!(t, Type::Function(_, _) | Type::Any) {
                        // Diagnostics?
                    }
                }
            }
            Expr::SpawnEach {
                list,
                closure,
                span: _,
            }
            | Expr::ListMap {
                list,
                closure,
                span: _,
            }
            | Expr::ListFilter {
                list,
                closure,
                span: _,
            } => {
                self.visit_expr(list);
                self.visit_expr(closure);
            }
            Expr::Send { pid, msg, span } => {
                self.visit_expr(pid);
                self.visit_expr(msg);
                self.check_assignment(&Some(Type::Pid), pid, *span);
            }
            Expr::Receive { .. } => {}
            Expr::Confidence { expr, .. } => {
                self.visit_expr(expr);
            }
            Expr::Infer { body, .. } => {
                self.visit_block(body);
            }
            Expr::If {
                cond,
                then_block,
                else_block,
                ..
            } => {
                self.visit_expr(cond);
                self.visit_block(then_block);
                if let Some(b) = else_block {
                    self.visit_block(b);
                }
            }
            Expr::Id { name, span } => {
                self.record_usage(name, *span);
            }
            Expr::Turn {
                params,
                ret_ty,
                body,
                span,
                ..
            } => {
                self.enter_scope(*span, ret_ty.clone());
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
            Expr::Call { name, args, .. } => {
                self.visit_expr(name);
                for arg in args {
                    self.visit_expr(arg);
                }
            }
            Expr::MethodCall {
                target,
                name,
                args,
                span,
            } => {
                self.visit_expr(target);
                for arg in args {
                    self.visit_expr(arg);
                }
                // Infer target type
                let target_ty = self.infer_expr_type(target);
                if let Some(Type::Struct(type_name, _)) = target_ty {
                    // Look up method
                    if let Some((_, method_ty)) = self.find_method(&type_name, name) {
                        // Check arg type
                        // Method signature: Function(Arg, Ret)
                        if let Some(Type::Function(arg_ty, _ret_ty)) = method_ty {
                            // Check arguments (for now, just check the first one if we can)
                            if let Some(first_arg) = args.first() {
                                self.check_assignment(&Some(*arg_ty), first_arg, *span);
                            }
                            // We don't return type from visit_expr, but we checked logic
                        }
                    } else {
                        self.diagnostics.push((
                            *span,
                            format!("Unknown method '{}' for type '{}'", name, type_name),
                        ));
                    }
                }
            }
            Expr::Recall { key, .. } => self.visit_expr(key),
            Expr::Use { module, .. } => self.visit_expr(module),
            Expr::UseSchema { url, .. } => self.visit_expr(url),
            Expr::Grant { .. } => {}
            Expr::Index { target, index, .. } => {
                self.visit_expr(target);
                self.visit_expr(index);
            }
            Expr::List { items, .. } => {
                for item in items {
                    self.visit_expr(item);
                }
            }
            Expr::Vec { items, .. } => {
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
                if offset >= scope.span.start
                    && offset <= scope.span.end
                    && (!found
                        || (scope.span.end - scope.span.start)
                            < (self.scopes[best_idx].span.end - self.scopes[best_idx].span.start))
                {
                    best_idx = i;
                    found = true;
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
