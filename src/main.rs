use std::fs;
use std::cell::RefCell;
use std::path::Path;
use std::sync::{Arc, Mutex};


use solidhunter_lib::linter::*;
use solidhunter_lib::types::*;


use tower_lsp::jsonrpc::{Result};
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
        self.client.log_message(MessageType::INFO, "Linting project ...".to_string()).await;
        let mut diagnostics: Vec<Diagnostic> = Vec::new();
        let mut lint_results: Vec<LintResult> = Vec::new();
        let mut root_path = String::new();

        let res = self.client.workspace_folders().await.unwrap_or_else(|err| {
            self.client.log_message(MessageType::ERROR, format!("Error while loading workspace: {:?}", err));
            None
        });

        {
            match res {
                Some(folders) => {
                    let mut linter = self.linter.lock().unwrap();
                    folders.iter().for_each(|folder| {
                        let path = Backend::get_linter_config_path(folder.uri.as_str());
                        if Path::new(&path).exists() {
                            linter.get_mut().initalize(&path);
                            root_path = String::from(folder.uri.clone().as_str());
                            return;
                        }
                    });
                    folders.iter().for_each(|folder| {
                        lint_results.append(&mut linter.get_mut().parse_folder(folder.uri.path().to_string().clone()));
                    });
                }
                None => {
                    self.client.log_message(MessageType::ERROR, format!("No workspace folder found")).await;
                }
            }
        }

        let _ = lint_results.into_iter().for_each(|lint_result| {
            match lint_result {
                Ok(diags) => {
                    diags.into_iter().for_each(|diag| {
                        diagnostics.push(Backend::create_diagnostic(diag));
                    });
                }
                Err(e) => {
                    self.client.log_message(MessageType::ERROR, e);
                }
            }
        });


        self.client.publish_diagnostics(
            root_path.parse().unwrap(),
            diagnostics,
            None,
        ).await;

        self.client
            .log_message(MessageType::INFO, "initialized!")
            .await;
    }
    
    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_delete_files(&self, params: DeleteFilesParams) {
        params.files.iter().for_each(|file| {
            let mut linter = self.linter.lock().unwrap();
            linter.get_mut().delete_file(file.clone().uri);
        });
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let file_content = fs::read_to_string(params.text_document.uri.path());
        
        match file_content {
            Ok(content) => {
                if params.text_document.uri.as_str().ends_with(".sol") {
                    self.on_change(TextDocumentItem {
                        uri: params.text_document.uri,
                        text: content,
                        version: params.text_document.version,
                        language_id: "".to_string(),
                    }).await;
                }
            }
            Err(err) => {
                self.client.log_message(MessageType::ERROR, err.to_string()).await;
            }
        }
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        if params.text_document.uri.as_str().ends_with(".sol") {
            self.on_change(TextDocumentItem {
                uri: params.text_document.clone().uri,
                text: self.recreate_content(Url::to_string(&params.text_document.clone().uri), params.content_changes.clone()),
                version: params.text_document.clone().version,
                language_id: "".to_string(),
            }).await;
        }
    }

    async fn did_save(&self, _: DidSaveTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "file saved!")
            .await;
    }
}



impl Backend {
    
    fn get_linter_config_path(workdir: &str) -> String {
        //TODO: Check if file exists
        String::from(workdir.to_string() + &String::from("/.solidhunter.json"))
    }
    
    fn recreate_content(&self, filepath: String, changes: Vec<TextDocumentContentChangeEvent>) -> String {
        let mut content = fs::read_to_string(filepath).unwrap();
        
        changes.iter().for_each(|change| {
            let start = change.range.unwrap().start;
            let end = change.range.unwrap().end;
            let start_index = content.char_indices().nth(start.line as usize).unwrap().0 + start.character as usize;
            let end_index = content.char_indices().nth(end.line as usize).unwrap().0 + end.character as usize;
            if start_index >= content.len() {
                content.insert_str(start_index, &change.text);
            } else {
                content.replace_range(start_index..end_index, &change.text);
            }
            content.replace_range(start_index..end_index, &change.text);
        });
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
    
    async fn on_change(&self, params: TextDocumentItem) {
        self.client.log_message(MessageType::INFO, "file changed!").await;

        let res : LintResult;
        let mut diagnostics : Vec<Diagnostic> = Vec::new();
        {
            let mut linter = self.linter.lock().unwrap();
            res = linter.get_mut().parse_content(params.uri.as_str().to_string(), &params.text);
        }
        match res {
            Ok(diags) => {
                diags.into_iter().for_each(|diag| {
                    diagnostics.push(Backend::create_diagnostic(diag));
                });
            }
            Err(_) => {

            }
        }
        self.client.publish_diagnostics(
            params.uri.clone(),
            diagnostics,
            None,
        ).await;
    }
}

#[tokio::main]
async fn main() {
    println!("Starting server ...");
    env_logger::init();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::build(|client| Backend {
        client,
        linter: Arc::new(Mutex::from(RefCell::new(SolidLinter::new()))),
    })
    .finish();
    println!("Server started !");
    Server::new(stdin, stdout, socket).serve(service).await;
}

