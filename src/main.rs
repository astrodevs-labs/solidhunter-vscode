use std::borrow::BorrowMut;
use std::fs;
use std::cell::RefCell;
use std::ops::Add;
use glob::glob;
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
        let mut lint_results: Vec<LintResult> = Vec::new();
        let mut root_path = String::new();

        let folders = self.client.workspace_folders().await.unwrap_or(None).unwrap_or(vec![]);

        root_path = self.init_config(&folders).await;
        warn!("Linting started on folder: {}", root_path);
        
        if root_path != "" {
            let mut linter = self.linter.lock().unwrap();
            let mut linter = linter.borrow_mut();
            for entry in glob(&*(root_path.to_string() + "/**/*.sol")) {
                for path in entry {
                    warn!("Linting file: {:?}", path);
                    lint_results.push(linter.get_mut().parse_file(String::from(path.unwrap().into_os_string().into_string().unwrap())));
                }
            }
        } else {
            error!("No root path found");
        }
        for lint_result in lint_results {
            match lint_result {
                Ok(diags) => {
                    let mut diag_vec: Vec<Diagnostic> = Vec::new();
                    let mut f_uri = Url::from_str("file://").unwrap();
                    diags.into_iter().for_each(|diag| { 
                        warn!("Found error: {}", diag.message);
                        if f_uri.as_str() == "file://"{
                            let tmp = String::from("file://") + diag.uri.as_str();
                            f_uri = Url::from_str(tmp.as_str()).unwrap_or(Url::from_str("file://").unwrap());
                        }
                        diag_vec.push(Backend::create_diagnostic(diag));
                    });
                    self.client.publish_diagnostics(f_uri, diag_vec, None).await;
                }
                Err(e) => {
                    error!("Error while linting: {:?}", e);
                }
            }
        }

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
            uri: params.text_document.clone().uri,
            text: self.recreate_content(params.text_document.clone().uri.path().to_string(), params.content_changes.clone()),
            version: params.text_document.clone().version,
            language_id: "".to_string(),
        }).await
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
       warn!("file saved!");
        self.on_change(TextDocumentItem {
            uri: params.text_document.clone().uri,
            text: params.text.unwrap(),
            version: 0,
            language_id: "".to_string(),
        }).await
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
            error!("No workspace folder found.");
            return String::new();
        }

        let path = Backend::get_linter_config_path(folders[0].clone().uri.path()).unwrap_or(String::from(""));
        
        if path != "" {
            let mut linter = self.linter.lock().unwrap();
            let mut linter = linter.borrow_mut();
            linter.get_mut().initalize(&path);
        } else if path == "" {
            error!("No config file found.");
        }
        
        folders[0].clone().uri.path().to_string()
    }

    fn recreate_content(&self, filepath: String, changes: Vec<TextDocumentContentChangeEvent>) -> String {
        warn!("Recreating content for file: {}", filepath);
        let mut content = fs::read_to_string(filepath).unwrap();

        for change in changes {
            return change.text;
        }
        warn!("new file Content: {}", content);
        content
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
                _ => {DiagnosticSeverity::ERROR}
                /*Severity::WARNING => {DiagnosticSeverity::WARNING}
                Severity::INFO => {DiagnosticSeverity::INFORMATION}
                Severity::HINT => {DiagnosticSeverity::HINT}*/
            }),
            code: None,
            code_description: None,
            source: None,
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
        let res : LintResult;
        let mut diagnostics : Vec<Diagnostic> = Vec::new();
        {
            let mut linter = self.linter.lock().unwrap();
            warn!("Linting file: {}\n Content: {}", params.uri.path(), &params.text);
            res = linter.get_mut().parse_file(params.uri.path().to_string());
        }
        match res {
            Ok(diags) => {
                diags.into_iter().for_each(|diag| {
                    warn!("Found error: {}", diag.message);
                    let f_diag = Backend::create_diagnostic(diag);
                    warn!("Found error: {:?}", f_diag);
                    diagnostics.push(f_diag);
                });
                self.client.publish_diagnostics(
                    params.uri.clone(),
                    diagnostics,
                    None,
                ).await;
            }
            Err(e) => {
                error!("Error while linting: {:?}", e);
            }
        }

        /*
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
        }*/
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

