# Turn Language Server Protocol (LSP) (Planned)

**Status:** Draft.

## 1. Philosophy: The Agent's Nervous System

A modern language is defined by its tooling. The LSP acts as the "nervous system" connecting the code (static text) to the developer's intent (dynamic editing).

For Turn, the LSP provides:
*   **Instant Feedback:** Syntax errors and lints as you type.
*   **Navigation:** "Go to Definition" for modules and variables.
*   **Insight:** Hover over a variable to see its scope or value type (if inferable).

## 2. Architecture

We will embed the LSP server directly into the `turn` binary.
Command: `turn lsp`

This simplifies distribution. The VSCode extension simply spawns `turn lsp` and communicates via Stdio.

### Tech Stack
*   **Protocol:** LSP 3.17
*   **Transport:** Stdio (Standard Input/Output)
*   **Library:** `tower-lsp` (Rust)

## 3. Features (planned)

### 3.1 Diagnostics (Linting)
*   **Trigger:** `textDocument/didOpen`, `textDocument/didChange`, `textDocument/didSave`.
*   **Action:**
    1.  Run `Lexer`. If error -> Report Diagnostic.
    2.  Run `Parser`. If error -> Report Diagnostic.
    3.  (Future) Run `Compiler`. If error -> Report Diagnostic.
*   **Output:** Red squiggles at the exact line/column of the error.

### 3.2 Completion (Basic)
*   **Trigger:** `textDocument/completion`.
*   **Action:** Suggest keywords (`turn`, `let`, `try`, `catch`, `call`, `use`).

## 4. Future Roadmap
*   **Go to Definition:** Jump to where a variable was `let` defined.
*   **Hover:** Show documentation for Standard Library tools.
*   **Module Resolution:** Resolve `use "./module.tn"` and provide completion for exported keys.
