use std::fs;
use std::cell::RefCell;
use std::path::Path;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use log::{error, warn};

use solidhunter_lib::linter::*;
use solidhunter_lib::types::*;

use ropey::Rope;

use tower_lsp::jsonrpc::{Error, ErrorCode, Result};
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

struct Backend {
    client: Client,
    linter: Arc<Mutex<RefCell<SolidLinter>>>
}

#[tower_lsp::async_trait]

impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
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
        warn!("Linting project ...");
        let mut diagnostics: Vec<Diagnostic> = Vec::new();
        let mut lint_results: Vec<LintResult> = Vec::new();
        let mut root_path = String::new();

        let folders = self.client.workspace_folders().await.unwrap_or(None).unwrap_or(vec![]);

        root_path = self.init_config(&folders).await;
        warn!("Linting started on folder: {}", root_path);
        
        if root_path != "" {
            let linter = self.linter.lock().unwrap();
            lint_results = linter.borrow_mut().parse_folder(root_path.to_string());
        } else {
            error!("No root path found");
        }
        warn!("Linting finished");
        for lint_result in lint_results {
            match lint_result {
                Ok(diags) => {
                    diags.into_iter().for_each(|diag| { 
                        warn!("Found error: {}", diag.message);
                        diagnostics.push(Backend::create_diagnostic(diag));
                    });
                }
                Err(e) => {
                    self.client
                        .log_message(MessageType::ERROR, e)
                        .await;
                }
            }
        }


        self.client
            .publish_diagnostics(
            Url::from_str(root_path.as_str()).unwrap_or(Url::parse("file:///").unwrap()),
            diagnostics,
            None)
            .await;

       warn!("finished initialization");
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        warn!("file opened!");
        self.on_change(TextDocumentItem {
            uri: params.text_document.uri,
            text: params.text_document.text,
            version: params.text_document.version,
            language_id: "".to_string(),
        })
            .await
    }

    async fn did_change(&self, mut params: DidChangeTextDocumentParams) {
        warn!("file changed!");
        self.on_change(TextDocumentItem {
            uri: params.text_document.uri,
            text: std::mem::take(&mut params.content_changes[0].text),
            version: params.text_document.version,
            language_id: "".to_string(),
        }).await
    }

    async fn did_save(&self, _: DidSaveTextDocumentParams) {
       warn!("file saved!");
    }

    async fn completion(&self, _: CompletionParams) -> Result<Option<CompletionResponse>> {
        Ok(None)
    }
    
    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        warn!("file closed!");
    }
}


impl Backend {
    
    pub async fn init_config(&self, folders : &Vec<WorkspaceFolder>) -> String {

        if folders.len() <= 0 {
            self.client.log_message(MessageType::ERROR, "No workspace folder found").await;
            return String::new();
        }

        let path = Backend::get_linter_config_path(folders[0].clone().uri.path()).unwrap_or(String::from(""));
        
        if path != "" {
            let mut linter = self.linter.lock().unwrap();
            linter.borrow_mut().initalize(&path);
        } else if path == "" {
            self.client.log_message(MessageType::ERROR, "No config file found").await;
        }
        
        warn!("Config file found at: {}", path);
        folders[0].clone().uri.path().to_string()
    }
    
    fn create_diagnostic(diag : LintDiag) -> Diagnostic
    {
        return Diagnostic {
            range: tower_lsp::lsp_types::Range {
                start: tower_lsp::lsp_types::Position {
                    line: diag.range.start.line as u32,
                    character: diag.range.start.character as u32,
                },
                end: tower_lsp::lsp_types::Position {
                    line: diag.range.end.line as u32,
                    character: diag.range.end.character as u32,
                },
            },
            severity: Some(match diag.severity.unwrap() {
                Severity::ERROR => {DiagnosticSeverity::ERROR}
                Severity::WARNING => {DiagnosticSeverity::WARNING}
                Severity::INFO => {DiagnosticSeverity::INFORMATION}
                Severity::HINT => {DiagnosticSeverity::HINT}
            }),
            code: None,
            code_description: None,
            source: diag.source.clone(),
            message: diag.message.to_string(),
            related_information: None,
            tags: None,
            data: None,
        };
    }

    fn get_linter_config_path(workdir: &str) -> Result<String> {
        let test = String::from(workdir.to_string() + &String::from("/.solidhunter.json"));
        if Path::new(&test).exists() {
            return Ok(test);
        }
        return Err(Error { code: ErrorCode::ParseError, message: test, data: None });
    }
    
    async fn on_change(&self, params: TextDocumentItem) {
        // TODO: use solidhunter to generate hunter-diagnostics
        // and send them to the client
        
        // Exemple: 
        let rope = Rope::from_str(&params.text);
        for i in 0..rope.len_lines() {
            let line = rope.line(i);
            let line_str = line.to_string();
            if line_str.contains("dummy") {
                self.client.publish_diagnostics(
                   params.uri.clone(),
                    vec![Diagnostic {
                        range: tower_lsp::lsp_types::Range {
                            start: tower_lsp::lsp_types::Position {
                                line: i as u64 as u32,
                                character: 0,
                            },
                            end: tower_lsp::lsp_types::Position {
                                line: i as u64 as u32,
                                character: line.len_chars() as u64 as u32,
                            },
                        },
                        severity: Some(DiagnosticSeverity::ERROR),
                        code: None,
                        code_description: None,
                        source: None,
                        message: "dummy found!".to_string(),
                        related_information: None,
                        tags: None,
                        data: None,
                    }],
                    None,
                ).await;
            }
        }
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::build(|client| Backend {
        client,
        linter: Arc::new(Mutex::new(RefCell::new(SolidLinter::new()))),
    })
    .finish();
    Server::new(stdin, stdout, socket).serve(service).await;
}

