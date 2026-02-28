use crate::db::models::{Entity, Intent, Mcp, TemporalRef};
use crate::error::AppError;

#[derive(Clone)]
pub struct IntentService;

impl IntentService {
    pub fn new() -> Self {
        Self
    }

    pub async fn classify_intent(&self, query: &str) -> Result<Intent, AppError> {
        let q = query.to_lowercase();

        let intent = if q.contains("sql") || q.contains("database") {
            ("Database", 0.93)
        } else if q.contains("code") || q.contains("rust") || q.contains("bug") {
            ("Code", 0.9)
        } else if q.contains("find") || q.contains("search") || q.contains("look up") {
            ("Search", 0.86)
        } else if q.contains("remember") || q.contains("memory") {
            ("Memory", 0.88)
        } else if q.contains("calendar") || q.contains("meeting") {
            ("Calendar", 0.82)
        } else if q.contains("task") || q.contains("todo") {
            ("Task", 0.82)
        } else {
            ("Chat", 0.7)
        };

        Ok(Intent {
            name: intent.0.to_string(),
            confidence: intent.1,
        })
    }

    pub async fn extract_entities(&self, query: &str) -> Result<Vec<Entity>, AppError> {
        let mut entities = Vec::new();

        for token in query.split_whitespace() {
            if token.starts_with("http://") || token.starts_with("https://") {
                entities.push(Entity {
                    kind: "url".to_string(),
                    value: token.to_string(),
                });
            }

            if token.contains(":\\") || token.starts_with("/") {
                entities.push(Entity {
                    kind: "file_path".to_string(),
                    value: token.trim_matches([',', '.', ';']).to_string(),
                });
            }

            if token.chars().all(|ch| ch.is_ascii_digit()) && token.len() >= 4 {
                entities.push(Entity {
                    kind: "number".to_string(),
                    value: token.to_string(),
                });
            }
        }

        Ok(entities)
    }

    pub fn detect_temporal_context(&self, query: &str) -> Option<TemporalRef> {
        let q = query.to_lowercase();

        if q.contains("yesterday") {
            let end = chrono::Utc::now().date_naive() - chrono::Days::new(1);
            let start = end;
            return Some(TemporalRef {
                phrase: "yesterday".to_string(),
                start: Some(start.to_string()),
                end: Some(end.to_string()),
            });
        }

        if q.contains("last week") {
            let end = chrono::Utc::now().date_naive();
            let start = end - chrono::Days::new(7);
            return Some(TemporalRef {
                phrase: "last week".to_string(),
                start: Some(start.to_string()),
                end: Some(end.to_string()),
            });
        }

        if q.contains("today") {
            let today = chrono::Utc::now().date_naive();
            return Some(TemporalRef {
                phrase: "today".to_string(),
                start: Some(today.to_string()),
                end: Some(today.to_string()),
            });
        }

        None
    }

    pub fn predict_needed_mcps(&self, query: &str, user_mcps: &[Mcp]) -> Vec<String> {
        let q = query.to_lowercase();
        let mut chosen = Vec::new();

        for mcp in user_mcps {
            let name = mcp.name.to_lowercase();
            let category = mcp.category.to_lowercase();

            if category == "system" || mcp.is_builtin == 1 {
                chosen.push(mcp.id.clone());
                continue;
            }

            if q.contains("calendar") && (name.contains("calendar") || name.contains("google")) {
                chosen.push(mcp.id.clone());
            } else if q.contains("spotify") && name.contains("spotify") {
                chosen.push(mcp.id.clone());
            } else if q.contains("git") && name.contains("git") {
                chosen.push(mcp.id.clone());
            } else if q.contains("file") && category.contains("system") {
                chosen.push(mcp.id.clone());
            }
        }

        chosen.sort();
        chosen.dedup();
        chosen
    }
}
