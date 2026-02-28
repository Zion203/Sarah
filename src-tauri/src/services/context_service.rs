use std::sync::Arc;

use crate::db::models::{AssembledContext, Mcp, Message};
use crate::error::AppError;
use crate::repositories::conversation_repo::ConversationRepo;
use crate::repositories::model_repo::ModelRepo;
use crate::services::intent_service::IntentService;
use crate::services::mcp_service::McpService;
use crate::services::memory_service::MemoryService;
use crate::services::rag_service::RagService;

#[derive(Clone)]
pub struct ContextService {
    memory_service: MemoryService,
    rag_service: Option<Arc<RagService>>,
    intent_service: IntentService,
    mcp_service: McpService,
    conversation_repo: ConversationRepo,
    model_repo: ModelRepo,
}

impl ContextService {
    pub fn new(
        memory_service: MemoryService,
        rag_service: Option<Arc<RagService>>,
        intent_service: IntentService,
        mcp_service: McpService,
        conversation_repo: ConversationRepo,
        model_repo: ModelRepo,
    ) -> Self {
        Self {
            memory_service,
            rag_service,
            intent_service,
            mcp_service,
            conversation_repo,
            model_repo,
        }
    }

    pub async fn build_context(
        &self,
        user_id: &str,
        session_id: &str,
        query: &str,
    ) -> Result<AssembledContext, AppError> {
        let memory_fut = self.memory_service.retrieve_relevant(user_id, query, 10);

        let rag_fut = async {
            if let Some(rag) = self.rag_service.as_ref() {
                rag.retrieve(user_id, query, "personal", 8).await.ok()
            } else {
                Some(Vec::new())
            }
        };

        let intent_fut = self.intent_service.classify_intent(query);
        let conv_fut = self.conversation_repo.get_context_window(session_id, 2000);

        let (memories, docs, intent, messages) =
            tokio::join!(memory_fut, rag_fut, intent_fut, conv_fut);

        let memories = memories?;
        let docs = docs.unwrap_or_default();
        let intent = intent?;
        let mut messages = messages?;

        let mcp_ids = self
            .mcp_service
            .route_mcps_for_query(query, &intent, user_id)
            .await?;

        let tools: Vec<Mcp> = self
            .mcp_service
            .health_check_all()
            .await
            .unwrap_or_default()
            .into_iter()
            .filter_map(|status| {
                if mcp_ids.contains(&status.mcp_id) {
                    Some(Mcp {
                        id: status.mcp_id,
                        name: "mcp".to_string(),
                        display_name: "MCP".to_string(),
                        description: None,
                        version: None,
                        author: None,
                        icon_path: None,
                        category: "tool".to_string(),
                        mcp_type: "builtin".to_string(),
                        command: None,
                        args: "[]".to_string(),
                        env_vars: "{}".to_string(),
                        url: None,
                        tool_schemas: "[]".to_string(),
                        resource_schemas: "[]".to_string(),
                        prompt_schemas: "[]".to_string(),
                        is_installed: 1,
                        is_active: 1,
                        is_builtin: 1,
                        is_default: 0,
                        health_status: status.health_status,
                        last_health_check_at: None,
                        last_error: status.last_error,
                        metadata: "{}".to_string(),
                        created_at: String::new(),
                        updated_at: String::new(),
                    })
                } else {
                    None
                }
            })
            .collect();

        if messages.len() > 24 {
            messages = messages.split_off(messages.len().saturating_sub(24));
        }

        let mut installed_models = self.model_repo.list_installed().await?;
        let active_model = installed_models
            .iter()
            .find(|model| model.is_default == 1)
            .cloned()
            .or_else(|| installed_models.pop());

        let model_line = active_model
            .map(|m| format!("Active model: {} ({})", m.display_name, m.name))
            .unwrap_or_else(|| "Active model: none selected".to_string());

        let memory_block = if memories.is_empty() {
            "(none)".to_string()
        } else {
            memories
                .iter()
                .map(|m| {
                    format!(
                        "[Memory:{}] {}",
                        m.subject.as_deref().unwrap_or("fact"),
                        m.content
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        };

        let doc_block = if docs.is_empty() {
            "(none)".to_string()
        } else {
            docs.iter()
                .enumerate()
                .map(|(idx, row)| format!("[Doc {}] {}", idx + 1, row.chunk.content))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let tool_block = if tools.is_empty() {
            "(none)".to_string()
        } else {
            tools
                .iter()
                .map(|t| format!("{} ({})", t.display_name, t.health_status))
                .collect::<Vec<_>>()
                .join(", ")
        };

        let mut system_prompt = format!(
            "You are Sarah, a highly capable local AI assistant.\n\n{}\n\nUSER MEMORY:\n{}\n\nRELEVANT KNOWLEDGE:\n{}\n\nACTIVE TOOLS: {}\n\nGUIDELINES:\n- Personalize using memory facts\n- Cite sources as [Doc N] or [Memory: subject]\n- Extract new facts to memory when user shares information\n- Be concise, intelligent, and premium quality",
            model_line, memory_block, doc_block, tool_block
        );

        trim_context(&mut system_prompt, &mut messages, 3500);

        Ok(AssembledContext {
            system_prompt,
            messages,
            tools,
            memory_refs: memories,
            doc_refs: docs,
        })
    }
}

fn trim_context(system_prompt: &mut String, messages: &mut Vec<Message>, max_tokens: usize) {
    let estimate_tokens = |text: &str| text.len() / 4;

    while estimate_tokens(system_prompt)
        + messages
            .iter()
            .map(|m| estimate_tokens(&m.content))
            .sum::<usize>()
        > max_tokens
    {
        if messages.len() > 4 {
            messages.remove(0);
        } else if system_prompt.len() > 400 {
            let trimmed = system_prompt
                .chars()
                .skip(system_prompt.len().saturating_sub(400))
                .collect::<String>();
            *system_prompt = trimmed;
            break;
        } else {
            break;
        }
    }
}
