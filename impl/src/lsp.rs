use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::offset_to_line_col;

#[derive(Debug)]
pub struct Backend {
    pub client: Client,
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
        self.validate_document(params.text_document.uri, &params.text_document.text).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        // We use Full sync, so content_changes[0] has the full text
        if let Some(change) = params.content_changes.first() {
            self.validate_document(params.text_document.uri, &change.text).await;
        }
    }
    
    async fn completion(&self, _params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let keywords = vec![
            "turn", "let", "use", "context", "remember", "recall", "call", "return",
            "if", "else", "while", "try", "catch", "throw", "true", "false", "null"
        ];
        
        let items: Vec<CompletionItem> = keywords.into_iter().map(|k| {
            CompletionItem {
                label: k.to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                ..Default::default()
            }
        }).collect();
        
        Ok(Some(CompletionResponse::Array(items)))
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
                        start: Position { line, character: col },
                        end: Position { line, character: col + 1 },
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
                self.client.publish_diagnostics(uri, diagnostics, None).await;
                return;
            }
        };

        // 2. Parsing
        if let Err(e) = Parser::new(tokens).parse() {
            let offset = e.offset();
            let (line, col) = offset_to_line_col(text, offset);
            let line = (line - 1) as u32;
            let col = (col - 1) as u32;

            let diagnostic = Diagnostic {
                range: Range {
                    start: Position { line, character: col },
                    end: Position { line, character: col + 1 }, // TODO: better span end
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
        }

        self.client.publish_diagnostics(uri, diagnostics, None).await;
    }
}
