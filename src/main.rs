use std::collections::{HashMap, HashSet};

use dashmap::DashMap;

use ropey::Rope;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use tower_lsp::jsonrpc::{ErrorCode, Result};
use tower_lsp::lsp_types::notification::Notification;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

#[derive(Debug)]
struct Backend {
    client: Client,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        let mut diagnostics : Vec<Diagnostic> = Vec::new();

        self.client.workspace_folders().iter().for_each(|workspace| {
            let res : LintResult = lint_folder(&workspace.uri.as_str());

            res.errors.iter().for_each(|diag| {
            diagnostics.push(createDiagnostic(diag));
            });
            res.warnings.iter().for_each(|diag| {
                diagnostics.push(createDiagnostic(diag));
            });
            res.infos.iter().for_each(|diag| {
                diagnostics.push(createDiagnostic(diag));
            });
            res.hints.iter().for_each(|diag| {
                diagnostics.push(createDiagnostic(diag));
            });
            self.client.publish_diagnostics(
                params.uri.clone(),
                diagnostics,
                None,
            ).await;
        });

        Ok(InitializeResult {
            server_info: None,
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),

                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![".".to_string()]),
                    work_done_progress_options: Default::default(),
                    all_commit_characters: None,
                }),

                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec!["dummy.do_something".to_string()],
                    work_done_progress_options: Default::default(),
                }),

                workspace: Some(WorkspaceServerCapabilities {
                    workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                        supported: Some(true),
                        change_notifications: Some(OneOf::Left(false)),
                    }),
                    file_operations: None,
                }),
                ..ServerCapabilities::default()
            },
        })
    }
    
    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "initialized!")
            .await;
    }
    
    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        if params.text_document.uri.as_str().ends_with(".sol") {
            self.on_change(TextDocumentItem {
                uri: params.text_document.uri,
                text: std::mem::take(&mut params.content_changes[0].text),
                version: params.text_document.version,
                language_id: "".to_string(),
            }).await
        }
    }

    async fn did_change(&self, mut params: DidChangeTextDocumentParams) {
        if params.text_document.uri.as_str().ends_with(".sol") {
            self.on_change(TextDocumentItem {
                uri: params.text_document.uri,
                text: std::mem::take(&mut params.content_changes[0].text),
                version: params.text_document.version,
                language_id: "".to_string(),
            }).await
        }
    }

    async fn did_save(&self, _: DidSaveTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "file saved!")
            .await;
    }
}



impl Backend {
    
    fn createDiagnostic(diag : lint_result::ResultElem) -> Diagnostic
    {
        return Diagnostic {
            range: tower_lsp::Range {
                start: tower_lsp::Position {
                    line: diag.range.start.line as u32,
                    character: diag.range.start.character as u32,
                },
                end: tower_lsp::Position {
                    line: diag.range.end.line as u32,
                    character: diag.range.end.character as u32,
                },
            },
            severity: Some(DiagnosticSeverity(diag.severity.into())),
            code: diag.code.into(),
            code_description: None,
            source: diag.source.clone(),
            message: diag.message.to_string(),
            related_information: None,
            tags: None,
            data: None,
        };
    }
    
    async fn on_change(&self, params: TextDocumentItem) {
        self.client.log_message(MessageType::INFO, "file changed!").await;
        // TODO: use solidhunter to generate hunter-d      iagnostics
        // and send them to the client
        
        // Exemple: 
        let mut diagnostics : Vec<Diagnostic> = Vec::new();
        let res : LintResult = lint_file(&params.uri.as_str(), &params.text);
        res.errors.iter().for_each(|diag| {
            diagnostics.push(createDiagnostic(diag));
        });
        res.warnings.iter().for_each(|diag| {
            diagnostics.push(createDiagnostic(diag));
        });
        res.infos.iter().for_each(|diag| {
            diagnostics.push(createDiagnostic(diag));
        });
        res.hints.iter().for_each(|diag| {
            diagnostics.push(createDiagnostic(diag));
        });
        self.client.publish_diagnostics(
            params.uri.clone(),
            diagnostics,
            None,
        ).await;
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::build(|client| Backend {
        client,
    })
    .finish();
    Server::new(stdin, stdout, socket).serve(service).await;
}

fn offset_to_position(offset: usize, rope: &Rope) -> Option<Position> {
    let line = rope.try_char_to_line(offset).ok()?;
    let first_char = rope.try_line_to_char(line).ok()?;
    let column = offset - first_char;
    Some(Position::new(line as u32, column as u32))
}
