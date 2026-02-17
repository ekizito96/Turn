# Turn Module System (v1.1)

**Status:** Draft.

## 1. Philosophy: Cognitive Units, Not Just Files

In traditional languages (Python/TS), modules are namespaces for code organization. In Turn, we view modules as **Cognitive Units** or **Skill Bundles**.

When an agent "uses" a module, it is *acquiring a skill* or *loading a context*.

### The Physics of Composition
*   **Encapsulation:** A module must define a clear boundary. Internal state should not leak unless intended.
*   **Reusability:** A module should be a portable unit of behavior (e.g., a "ResearchSkill" or "SlackTool").
*   **Determinism:** Loading a module must be deterministic. The same module code must produce the same exports.

## 2. Syntax

We introduce the `use` keyword. It is a statement that binds a module's exports to a variable.

```turn
// Import a local file module
let utils = use "./std/utils.turn";

// Usage
let result = utils.helper_function("data");
```

### Exporting
By default, top-level `let` bindings in a module are **private** (internal to the module).
To make them available, we must explicitly `return` a map of exports, or use a specific `export` keyword.

**Decision:** To keep the "minimal core" philosophy and "First-Class Values", a module is simply a script that **returns a Value**. This is similar to Lua or how Node.js `module.exports` works conceptually, but simpler.

**Module `math.turn`:**
```turn
let pi = 3.14159;

// Private helper
let double = turn { 
    let x = recall("x"); 
    return x * 2; 
};

// The module evaluates to this map
return {
    "PI": pi,
    "area": turn {
        let r = recall("r");
        return pi * r * r;
    }
};
```

**Main Agent:**
```turn
let math = use "./math.turn";
let area = call(math.area, { "r": 10 });
```

## 3. Semantics

1.  **Resolution:** `use "path"` resolves relative to the current file.
2.  **Evaluation:**
    *   The module code is compiled and executed in a **fresh environment** (isolated scope).
    *   It does *not* share the caller's memory or context (unless explicitly passed). This enforces "Cognitive Isolation".
    *   The result of the execution (the `return` value) becomes the module object.
3.  **Caching:**
    *   Modules are cached by canonical path. Subsequent `use` calls return the *same* value (singleton pattern by default).
    *   This mirrors how "learning" works: once you learn a skill, you recall it; you don't relearn it every time.

## 4. Implementation Strategy

1.  **Compiler:**
    *   When `Compiler` encounters `use`, it technically acts as a runtime instruction `Instr::LoadModule`.
    *   However, for performance and valid bytecode, we might want to compile dependencies ahead of time.
    *   **Approach:** `use` is a runtime operation (like `require` in Node). This allows dynamic paths: `use config.module_path`.
    
2.  **VM Instruction: `LoadModule`**
    *   Pop path from stack.
    *   Check `Runtime.module_cache`. If hit, push cached value.
    *   If miss:
        *   Pause VM (suspend).
        *   Host (Runner) resolves file, reads source, compiles it.
        *   Host executes module bytecode to completion (recursively).
        *   Host caches result and resumes main VM with the result.
    
    *Wait, recursive VM execution?*
    Yes. The Runner needs to handle "Module Loading" similarly to a Tool Call, or we implement a nested VM invocation. Given our `Universal Loop`, it might be cleaner to treat `use` as a special **System Tool** (`sys_import`)?
    
    **Refinement:** Let's treat `use` as a built-in language feature, but implemented via a `ModuleLoader` component in the Runner.

## 5. Future: System Capabilities

Later, `use "slack"` could resolve not to a file, but to a "System Bundle" (injected host object), mirroring the Javiscore design.

```turn
// Future Phase
let slack = use "system:slack";
```

For now, we stick to file paths.
