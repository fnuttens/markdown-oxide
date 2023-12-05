use std::ops::Deref;
use std::path::Path;

use tokio::sync::RwLock;

use gotodef::goto_definition;
use tower_lsp::jsonrpc::{Result, Error, ErrorCode};
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use vault::{Vault, construct_vault, reconstruct_vault};

mod vault;
mod gotodef;


#[derive(Debug)]
struct Backend {
    client: Client,
    vault: RwLock<Option<Vault>>
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, i: InitializeParams) -> Result<InitializeResult> {
        let Some(root_uri) = i.root_uri else {
            return Err(Error::new(ErrorCode::InvalidParams));
        };
        let root_dir = Path::new(root_uri.path());
        let Ok(vault) = construct_vault(root_dir) else {
            return Err(Error::new(ErrorCode::ServerError(0)))
        };
        let mut value = self.vault.write().await;
        *value = Some(vault);

        return Ok(InitializeResult {
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
                    completion_item: None,
                }),
                // definition: Some(GotoCapability::default()),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                rename_provider: Some(OneOf::Left(true)),
                ..ServerCapabilities::default()

            }
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Obsidian_ls initialized")
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let Some(ref mut vault) = *self.vault.write().await else {
            self.client.log_message(MessageType::ERROR, "Vault is not initialized").await;
            return;
        };

        let Ok(path) = params.text_document.uri.to_file_path() else {
            self.client.log_message(MessageType::ERROR, "Failed to parse URI path").await;
            return;
        };
        let text = &params.content_changes[0].text;
        reconstruct_vault(vault, (&path, text));
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {

        self.client.log_message(MessageType::INFO, format!( "Got goto def request: {:?}", params )).await;


        let position = params.text_document_position_params.position;

        let vault_option = self.vault.read().await;
        let Some(vault) = vault_option.deref() else {
            self.client.log_message(MessageType::ERROR, "Vault is not initialized").await;
            return Err(Error::new(ErrorCode::ServerError(0)));
        };
        let Ok(path) = params.text_document_position_params.text_document.uri.to_file_path() else {
            self.client.log_message(MessageType::ERROR, "Failed to parse URI path").await;
            return Err(Error::new(ErrorCode::ServerError(0)));
        };
        self.client.log_message(MessageType::INFO, format!( "Path: {:?}", path )).await;
        let result = goto_definition(&vault, position, &path);

        self.client.log_message(MessageType::INFO, format!("Result {:?}", result)).await;

        return Ok(result.map(|l| GotoDefinitionResponse::Scalar(l)))
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend { client, vault: None.into() });
    Server::new(stdin, stdout, socket).serve(service).await;
}
