use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use tokio::process::Command;

use crate::db::models::{Intent, Mcp, McpHealthStatus, ToolResult};
use crate::error::AppError;
use crate::repositories::mcp_repo::McpRepo;
use crate::services::crypto_service::CryptoService;
use crate::services::intent_service::IntentService;

#[derive(Clone)]
struct McpClient {
    mcp: Mcp,
    last_used_at: Instant,
}

#[derive(Clone)]
pub struct McpService {
    repo: McpRepo,
    crypto: CryptoService,
    intent: IntentService,
    pool: Arc<DashMap<String, McpClient>>,
    breaker: Arc<DashMap<String, (u32, Instant, String)>>,
}

impl McpService {
    pub fn new(repo: McpRepo, crypto: CryptoService, intent: IntentService) -> Self {
        Self {
            repo,
            crypto,
            intent,
            pool: Arc::new(DashMap::new()),
            breaker: Arc::new(DashMap::new()),
        }
    }

    pub async fn ensure_connected(&self, mcp_id: &str) -> Result<Mcp, AppError> {
        if let Some(state) = self.breaker.get(mcp_id) {
            let (errors, opened_at, breaker_state) = state.value();
            if breaker_state == "open" && *errors >= 5 {
                if opened_at.elapsed() < Duration::from_secs(30) {
                    return Err(AppError::McpError {
                        mcp_id: mcp_id.to_string(),
                        message: "Circuit breaker open".to_string(),
                    });
                }
                self.breaker.insert(
                    mcp_id.to_string(),
                    (*errors, Instant::now(), "half-open".to_string()),
                );
            }
        }

        if let Some(mut client) = self.pool.get_mut(mcp_id) {
            client.last_used_at = Instant::now();
            return Ok(client.mcp.clone());
        }

        let mcp = self
            .repo
            .get_mcp(mcp_id)
            .await?
            .ok_or_else(|| AppError::NotFound {
                entity: "mcp".to_string(),
                id: mcp_id.to_string(),
            })?;

        let client = McpClient {
            mcp: mcp.clone(),
            last_used_at: Instant::now(),
        };

        self.pool.insert(mcp_id.to_string(), client);
        self.repo
            .upsert_connection_state(mcp_id, "default", "connected", "closed", None, true)
            .await?;

        Ok(mcp)
    }

    pub async fn call_tool(
        &self,
        mcp_id: &str,
        tool_name: &str,
        args: serde_json::Value,
        user_id: &str,
    ) -> Result<ToolResult, AppError> {
        let mcp = self.ensure_connected(mcp_id).await?;
        let started = Instant::now();

        let output = match mcp.mcp_type.as_str() {
            "builtin" => serde_json::json!({
                "tool": tool_name,
                "status": "ok",
                "message": "builtin tool executed",
                "args": args,
            })
            .to_string(),
            "stdio" => {
                let command = mcp.command.clone().ok_or_else(|| AppError::McpError {
                    mcp_id: mcp_id.to_string(),
                    message: "stdio MCP missing command".to_string(),
                })?;

                let mut cmd = Command::new(command);

                let args_json = args.to_string();
                if let Ok(parsed_args) = serde_json::from_str::<Vec<String>>(&mcp.args) {
                    for arg in parsed_args {
                        cmd.arg(arg);
                    }
                }

                cmd.arg(tool_name).arg(args_json);

                let timed = tokio::time::timeout(Duration::from_secs(30), cmd.output()).await;
                let result = match timed {
                    Ok(Ok(output)) => output,
                    Ok(Err(err)) => {
                        return Err(AppError::McpError {
                            mcp_id: mcp_id.to_string(),
                            message: err.to_string(),
                        })
                    }
                    Err(_) => {
                        return Err(AppError::McpError {
                            mcp_id: mcp_id.to_string(),
                            message: "Tool call timed out after 30 seconds".to_string(),
                        })
                    }
                };

                if !result.status.success() {
                    let stderr = String::from_utf8_lossy(&result.stderr).to_string();
                    return Err(AppError::McpError {
                        mcp_id: mcp_id.to_string(),
                        message: format!("Tool failed: {stderr}"),
                    });
                }

                String::from_utf8_lossy(&result.stdout).to_string()
            }
            _ => {
                return Err(AppError::McpError {
                    mcp_id: mcp_id.to_string(),
                    message: format!("Unsupported MCP type: {}", mcp.mcp_type),
                })
            }
        };

        let latency_ms = started.elapsed().as_millis() as i64;

        self.repo
            .record_usage(mcp_id, user_id, tool_name, latency_ms, true)
            .await?;

        self.repo
            .upsert_connection_state(
                mcp_id,
                user_id,
                "idle",
                "closed",
                Some(latency_ms as f64),
                true,
            )
            .await?;

        self.breaker.insert(
            mcp_id.to_string(),
            (0, Instant::now(), "closed".to_string()),
        );

        Ok(ToolResult {
            mcp_id: mcp_id.to_string(),
            tool_name: tool_name.to_string(),
            output,
            latency_ms,
            success: true,
            error: None,
        })
    }

    pub async fn health_check_all(&self) -> Result<Vec<McpHealthStatus>, AppError> {
        let mcps = self.repo.list_mcps(true).await?;
        let mut statuses = Vec::new();

        for mcp in mcps {
            if mcp.is_active == 0 {
                continue;
            }

            let health = if mcp.mcp_type == "builtin" {
                McpHealthStatus {
                    mcp_id: mcp.id.clone(),
                    health_status: "healthy".to_string(),
                    last_error: None,
                }
            } else if mcp.mcp_type == "stdio" {
                if mcp.command.is_some() {
                    McpHealthStatus {
                        mcp_id: mcp.id.clone(),
                        health_status: "healthy".to_string(),
                        last_error: None,
                    }
                } else {
                    McpHealthStatus {
                        mcp_id: mcp.id.clone(),
                        health_status: "down".to_string(),
                        last_error: Some("Missing command".to_string()),
                    }
                }
            } else {
                McpHealthStatus {
                    mcp_id: mcp.id.clone(),
                    health_status: "unknown".to_string(),
                    last_error: None,
                }
            };

            self.repo
                .update_health(&mcp.id, &health.health_status, health.last_error.as_deref())
                .await?;
            statuses.push(health);
        }

        Ok(statuses)
    }

    pub async fn route_mcps_for_query(
        &self,
        query: &str,
        intent: &Intent,
        _user_id: &str,
    ) -> Result<Vec<String>, AppError> {
        let active_mcps: Vec<Mcp> = self
            .repo
            .list_mcps(true)
            .await?
            .into_iter()
            .filter(|mcp| mcp.is_active == 1)
            .collect();

        let mut selected = self.intent.predict_needed_mcps(query, &active_mcps);

        for mcp in &active_mcps {
            if mcp.is_builtin == 1 {
                selected.push(mcp.id.clone());
            }
            if intent.name == "Search" && mcp.category == "data" {
                selected.push(mcp.id.clone());
            }
        }

        selected.sort();
        selected.dedup();
        Ok(selected)
    }

    pub async fn save_mcp_secret(
        &self,
        mcp_id: &str,
        user_id: &str,
        key: &str,
        value: &str,
    ) -> Result<(), AppError> {
        let encrypted = self.crypto.encrypt_to_compact(value.as_bytes())?;
        let mut split = encrypted.splitn(2, ':');
        let nonce = split.next().unwrap_or_default();
        let payload = split.next().unwrap_or_default();

        self.repo
            .save_secret(mcp_id, user_id, key, payload, nonce, Some("configured"))
            .await
    }

    pub async fn decrypt_mcp_secret(
        &self,
        mcp_id: &str,
        user_id: &str,
        key: &str,
    ) -> Result<Option<String>, AppError> {
        let Some(secret) = self.repo.get_secret(mcp_id, user_id, key).await? else {
            return Ok(None);
        };

        let compact = format!("{}:{}", secret.nonce, secret.encrypted_value);
        let mut plaintext = self.crypto.decrypt(&compact)?;
        let output =
            String::from_utf8(plaintext.clone()).map_err(|e| AppError::Crypto(e.to_string()))?;
        CryptoService::zeroize_after_use(&mut plaintext);
        Ok(Some(output))
    }

    pub async fn cleanup_idle_connections(&self, idle_ttl: Duration) -> Result<(), AppError> {
        let mut remove_ids = Vec::new();
        for entry in self.pool.iter() {
            if entry.value().last_used_at.elapsed() > idle_ttl {
                remove_ids.push(entry.key().clone());
            }
        }

        for id in remove_ids {
            self.pool.remove(&id);
        }

        Ok(())
    }

    pub async fn get_stats(
        &self,
        mcp_id: &str,
    ) -> Result<Vec<crate::db::models::McpUsageStat>, AppError> {
        self.repo.get_usage_stats(mcp_id).await
    }
}
