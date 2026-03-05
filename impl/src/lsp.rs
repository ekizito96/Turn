use crate::analysis::Analysis;
use crate::lexer::{Lexer, Span, Token};
use crate::parser::Parser;
use crate::{line_col_to_offset, offset_to_line_col};
use std::collections::HashMap;
use std::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

#[derive(Debug)]
pub struct Backend {
    pub client: Client,
    pub analysis: RwLock<Analysis>,
    pub documents: RwLock<HashMap<Url, String>>,
}

impl Backend {
    fn find_identifier(&self, source: &str, offset: usize) -> Option<(String, Span)> {
        let mut lexer = Lexer::new(source);
        while let Ok(t) = lexer.next_token() {
            if let Token::Eof = t.token {
                break;
            }
            if let Token::Id(name) = t.token {
                if t.span.start <= offset && offset <= t.span.end {
                    return Some((name, t.span));
                }
            }
        }
        None
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![".".to_string()]),
                    ..Default::default()
                }),
                definition_provider: Some(OneOf::Left(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Turn LSP initialized!")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.documents.write().unwrap().insert(
            params.text_document.uri.clone(),
            params.text_document.text.clone(),
        );
        self.validate_document(params.text_document.uri, &params.text_document.text)
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        // We use Full sync, so content_changes[0] has the full text
        if let Some(change) = params.content_changes.first() {
            self.documents
                .write()
                .unwrap()
                .insert(params.text_document.uri.clone(), change.text.clone());
            self.validate_document(params.text_document.uri, &change.text)
                .await;
        }
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let keywords = vec![
            "turn", "let", "use", "context", "remember", "recall", "call", "return", "if", "else",
            "while", "try", "catch", "throw", "true", "false", "null",
        ];

        let mut items: Vec<CompletionItem> = keywords
            .into_iter()
            .map(|k| CompletionItem {
                label: k.to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                ..Default::default()
            })
            .collect();

        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        // Try to get scope-aware completions
        let source_opt = self.documents.read().unwrap().get(&uri).cloned();

        if let Some(source) = source_opt {
            if let Some(offset) = line_col_to_offset(
                &source,
                (position.line + 1) as usize,
                (position.character + 1) as usize,
            ) {
                if let Ok(analysis) = self.analysis.read() {
                    let vars = analysis.completion_items(offset);
                    let mut seen = std::collections::HashSet::new();
                    for var in vars {
                        if seen.insert(var.clone()) {
                            items.push(CompletionItem {
                                label: var,
                                kind: Some(CompletionItemKind::VARIABLE),
                                ..Default::default()
                            });
                        }
                    }
                }
            }
        }

        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        // Get source text
        let source = {
            let docs = self.documents.read().unwrap();
            match docs.get(&uri) {
                Some(s) => s.clone(),
                None => return Ok(None),
            }
        };

        // Convert position to offset
        let offset = match line_col_to_offset(
            &source,
            (position.line + 1) as usize,
            (position.character + 1) as usize,
        ) {
            Some(o) => o,
            None => return Ok(None),
        };

        // Find identifier at offset
        let span = match self.find_identifier(&source, offset) {
            Some((_, s)) => s,
            None => return Ok(None),
        };

        // Query analysis
        if let Ok(analysis) = self.analysis.read() {
            if let Some(def_span) = analysis.usages.get(&span.start) {
                let (line, col) = offset_to_line_col(&source, def_span.start);
                return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                    uri,
                    range: Range {
                        start: Position {
                            line: (line - 1) as u32,
                            character: (col - 1) as u32,
                        },
                        end: Position {
                            line: (line - 1) as u32,
                            character: (col - 1) as u32,
                        },
                    },
                })));
            }
        }

        Ok(None)
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let source = {
            let docs = self.documents.read().unwrap();
            match docs.get(&uri) {
                Some(s) => s.clone(),
                None => return Ok(None),
            }
        };

        let offset = match line_col_to_offset(
            &source,
            (position.line + 1) as usize,
            (position.character + 1) as usize,
        ) {
            Some(o) => o,
            None => return Ok(None),
        };

        // Find identifier at offset
        let (name, span) = match self.find_identifier(&source, offset) {
            Some(x) => x,
            None => return Ok(None),
        };

        let mut def_span = None;

        if let Ok(analysis) = self.analysis.read() {
            // Check usages
            if let Some(ds) = analysis.usages.get(&span.start) {
                def_span = Some(*ds);
            } else {
                // Check if it's a definition itself
                for scope in &analysis.scopes {
                    if scope.span.start <= span.start && span.end <= scope.span.end {
                        if let Some((d_span, _)) = scope.definitions.get(&name) {
                            if *d_span == span {
                                def_span = Some(span);
                                break;
                            }
                        }
                    }
                }
            }
        }

        if let Some(target_span) = def_span {
            let (line, _) = offset_to_line_col(&source, target_span.start);
            let line_content = source.lines().nth(line - 1).unwrap_or("").trim();

            return Ok(Some(Hover {
                contents: HoverContents::Scalar(MarkedString::LanguageString(LanguageString {
                    language: "turn".to_string(),
                    value: format!("// Defined on line {}\n{}", line, line_content),
                })),
                range: None,
            }));
        }

        Ok(None)
    }
}

impl Backend {
    async fn validate_document(&self, uri: Url, text: &str) {
        let mut diagnostics = Vec::new();

        // 1. Lexing
        let tokens = match Lexer::new(text).tokenize() {
            Ok(t) => t,
            Err(e) => {
                let offset = e.offset().unwrap_or(0);
                let (line, col) = offset_to_line_col(text, offset);
                // LSP uses 0-based indexing
                let line = (line - 1) as u32;
                let col = (col - 1) as u32;

                let diagnostic = Diagnostic {
                    range: Range {
                        start: Position {
                            line,
                            character: col,
                        },
                        end: Position {
                            line,
                            character: col + 1,
                        },
                    },
                    severity: Some(DiagnosticSeverity::ERROR),
                    code: None,
                    code_description: None,
                    source: Some("turn-lexer".to_string()),
                    message: e.to_string(),
                    related_information: None,
                    tags: None,
                    data: None,
                };
                diagnostics.push(diagnostic);
                self.client
                    .publish_diagnostics(uri, diagnostics, None)
                    .await;
                return;
            }
        };

        // 2. Parsing
        let program = match Parser::new(tokens).parse() {
            Ok(p) => p,
            Err(e) => {
                let offset = e.offset();
                let (line, col) = offset_to_line_col(text, offset);
                let line = (line - 1) as u32;
                let col = (col - 1) as u32;

                let diagnostic = Diagnostic {
                    range: Range {
                        start: Position {
                            line,
                            character: col,
                        },
                        end: Position {
                            line,
                            character: col + 1,
                        }, // span end is approximate; single-token width
                    },
                    severity: Some(DiagnosticSeverity::ERROR),
                    code: None,
                    code_description: None,
                    source: Some("turn-parser".to_string()),
                    message: e.to_string(),
                    related_information: None,
                    tags: None,
                    data: None,
                };
                diagnostics.push(diagnostic);
                self.client
                    .publish_diagnostics(uri, diagnostics, None)
                    .await;
                return;
            }
        };

        // 3. Analysis
        let mut analysis = Analysis::new();
        analysis.analyze(&program);

        // Process analysis diagnostics
        for (span, msg) in &analysis.diagnostics {
            let (start_line, start_col) = offset_to_line_col(text, span.start);
            let (end_line, end_col) = offset_to_line_col(text, span.end);

            let diagnostic = Diagnostic {
                range: Range {
                    start: Position {
                        line: (start_line - 1) as u32,
                        character: (start_col - 1) as u32,
                    },
                    end: Position {
                        line: (end_line - 1) as u32,
                        character: (end_col - 1) as u32,
                    },
                },
                severity: Some(DiagnosticSeverity::ERROR),
                code: None,
                code_description: None,
                source: Some("turn-analysis".to_string()),
                message: msg.clone(),
                related_information: None,
                tags: None,
                data: None,
            };
            diagnostics.push(diagnostic);
        }

        // Store analysis result
        if let Ok(mut guard) = self.analysis.write() {
            *guard = analysis;
        }

        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }
}
