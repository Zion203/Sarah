use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct SystemProfile {
    pub id: String,
    pub cpu_brand: String,
    pub cpu_cores: i64,
    pub cpu_threads: i64,
    pub cpu_frequency_mhz: Option<i64>,
    pub total_ram_mb: i64,
    pub available_ram_mb: Option<i64>,
    pub gpu_name: Option<String>,
    pub gpu_vendor: Option<String>,
    pub gpu_vram_mb: Option<i64>,
    pub gpu_backend: Option<String>,
    pub storage_total_gb: Option<i64>,
    pub storage_available_gb: Option<i64>,
    pub storage_type: Option<String>,
    pub os_name: String,
    pub os_version: String,
    pub os_arch: String,
    pub platform: String,
    pub benchmark_tokens_per_sec: Option<f64>,
    pub benchmark_embed_ms: Option<f64>,
    pub capability_score: Option<f64>,
    pub supports_cuda: i64,
    pub supports_metal: i64,
    pub supports_vulkan: i64,
    pub supports_avx2: i64,
    pub supports_avx512: i64,
    pub last_scan_at: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkResult {
    pub profile_id: String,
    pub tokens_per_sec: f64,
    pub embed_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub family: String,
    pub version: Option<String>,
    pub parameter_count: Option<String>,
    pub quantization: Option<String>,
    pub file_format: String,
    pub file_path: Option<String>,
    pub file_size_mb: Option<i64>,
    pub context_length: i64,
    pub embedding_size: Option<i64>,
    pub category: String,
    pub capabilities: String,
    pub min_ram_mb: i64,
    pub recommended_ram_mb: i64,
    pub min_vram_mb: i64,
    pub performance_tier: String,
    pub energy_tier: String,
    pub compatibility_score: Option<f64>,
    pub is_downloaded: i64,
    pub is_active: i64,
    pub is_default: i64,
    pub is_recommended: i64,
    pub download_url: Option<String>,
    pub sha256_checksum: Option<String>,
    pub tags: String,
    pub metadata: String,
    pub last_used_at: Option<String>,
    pub tokens_generated: i64,
    pub avg_tokens_per_sec: Option<f64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewModel {
    pub name: String,
    pub display_name: String,
    pub family: String,
    pub version: Option<String>,
    pub parameter_count: Option<String>,
    pub quantization: Option<String>,
    pub file_format: String,
    pub file_path: Option<String>,
    pub file_size_mb: Option<i64>,
    pub context_length: i64,
    pub embedding_size: Option<i64>,
    pub category: String,
    pub capabilities: String,
    pub min_ram_mb: i64,
    pub recommended_ram_mb: i64,
    pub min_vram_mb: i64,
    pub performance_tier: String,
    pub energy_tier: String,
    pub download_url: Option<String>,
    pub sha256_checksum: Option<String>,
    pub tags: String,
    pub metadata: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ModelWithScore {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub recommendation_tier: String,
    pub score: f64,
    pub reasoning: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub user_id: String,
    pub title: Option<String>,
    pub model_id: Option<String>,
    pub system_prompt: Option<String>,
    pub context_window_used: Option<i64>,
    pub token_count: i64,
    pub message_count: i64,
    pub status: String,
    pub summary: Option<String>,
    pub tags: String,
    pub pinned: i64,
    pub forked_from_session_id: Option<String>,
    pub forked_at_message_id: Option<String>,
    pub metadata: String,
    pub last_message_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub content_type: String,
    pub thinking: Option<String>,
    pub token_count: Option<i64>,
    pub model_id: Option<String>,
    pub latency_ms: Option<i64>,
    pub tokens_per_sec: Option<f64>,
    pub finish_reason: Option<String>,
    pub is_error: i64,
    pub error_message: Option<String>,
    pub parent_message_id: Option<String>,
    pub edited_at: Option<String>,
    pub original_content: Option<String>,
    pub metadata: String,
    pub position: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewMessage {
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub content_type: String,
    pub token_count: Option<i64>,
    pub model_id: Option<String>,
    pub metadata: String,
    pub position: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct MessageSearchResult {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub position: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ToolCall {
    pub id: String,
    pub message_id: String,
    pub session_id: String,
    pub mcp_id: Option<String>,
    pub tool_name: String,
    pub tool_input: String,
    pub tool_output: Option<String>,
    pub status: String,
    pub error_message: Option<String>,
    pub latency_ms: Option<i64>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewToolCall {
    pub message_id: String,
    pub session_id: String,
    pub mcp_id: Option<String>,
    pub tool_name: String,
    pub tool_input: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Memory {
    pub id: String,
    pub user_id: String,
    pub memory_type: String,
    pub category: Option<String>,
    pub subject: Option<String>,
    pub predicate: Option<String>,
    pub object: Option<String>,
    pub content: String,
    pub summary: Option<String>,
    pub source: String,
    pub source_id: Option<String>,
    pub session_id: Option<String>,
    pub confidence: f64,
    pub importance: f64,
    pub decay_rate: f64,
    pub access_count: i64,
    pub last_accessed_at: Option<String>,
    pub privacy_level: String,
    pub is_verified: i64,
    pub is_archived: i64,
    pub is_pinned: i64,
    pub expires_at: Option<String>,
    pub tags: String,
    pub embedding_id: Option<String>,
    pub metadata: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewMemory {
    pub user_id: String,
    pub memory_type: String,
    pub category: Option<String>,
    pub subject: Option<String>,
    pub predicate: Option<String>,
    pub object: Option<String>,
    pub content: String,
    pub summary: Option<String>,
    pub source: String,
    pub source_id: Option<String>,
    pub session_id: Option<String>,
    pub confidence: f64,
    pub importance: f64,
    pub decay_rate: f64,
    pub privacy_level: String,
    pub tags: String,
    pub metadata: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct MemoryRelation {
    pub id: String,
    pub user_id: String,
    pub source_memory_id: String,
    pub target_memory_id: String,
    pub relation_type: String,
    pub strength: f64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryGraph {
    pub root_memory_id: String,
    pub nodes: Vec<Memory>,
    pub edges: Vec<MemoryRelation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    pub id: String,
    pub user_id: String,
    pub title: String,
    pub file_path: Option<String>,
    pub source_url: Option<String>,
    pub source_type: String,
    pub mime_type: Option<String>,
    pub file_size_bytes: Option<i64>,
    pub language: Option<String>,
    pub namespace: String,
    pub index_status: String,
    pub chunk_count: Option<i64>,
    pub token_count: Option<i64>,
    pub checksum: Option<String>,
    pub access_level: String,
    pub version: i64,
    pub parent_document_id: Option<String>,
    pub is_deleted: i64,
    pub last_indexed_at: Option<String>,
    pub metadata: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewDocument {
    pub user_id: String,
    pub title: String,
    pub file_path: Option<String>,
    pub source_url: Option<String>,
    pub source_type: String,
    pub mime_type: Option<String>,
    pub file_size_bytes: Option<i64>,
    pub namespace: String,
    pub checksum: Option<String>,
    pub metadata: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Chunk {
    pub id: String,
    pub document_id: String,
    pub user_id: String,
    pub chunk_index: i64,
    pub content: String,
    pub token_count: i64,
    pub start_char: Option<i64>,
    pub end_char: Option<i64>,
    pub page_number: Option<i64>,
    pub section_title: Option<String>,
    pub heading_path: Option<String>,
    pub embedding_id: Option<String>,
    pub metadata: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewChunk {
    pub document_id: String,
    pub user_id: String,
    pub chunk_index: i64,
    pub content: String,
    pub token_count: i64,
    pub start_char: Option<i64>,
    pub end_char: Option<i64>,
    pub page_number: Option<i64>,
    pub section_title: Option<String>,
    pub heading_path: Option<String>,
    pub metadata: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ChunkResult {
    pub id: String,
    pub document_id: String,
    pub chunk_index: i64,
    pub content: String,
    pub section_title: Option<String>,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingRow {
    pub id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub user_id: String,
    pub namespace: String,
    pub model_name: String,
    pub vector: Vec<u8>,
    pub dimensions: i64,
    pub norm: Option<f64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Mcp {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub version: Option<String>,
    pub author: Option<String>,
    pub icon_path: Option<String>,
    pub category: String,
    pub mcp_type: String,
    pub command: Option<String>,
    pub args: String,
    pub env_vars: String,
    pub url: Option<String>,
    pub tool_schemas: String,
    pub resource_schemas: String,
    pub prompt_schemas: String,
    pub is_installed: i64,
    pub is_active: i64,
    pub is_builtin: i64,
    pub is_default: i64,
    pub health_status: String,
    pub last_health_check_at: Option<String>,
    pub last_error: Option<String>,
    pub metadata: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct McpUsageStat {
    pub id: String,
    pub mcp_id: String,
    pub user_id: String,
    pub tool_name: String,
    pub call_count: i64,
    pub success_count: i64,
    pub error_count: i64,
    pub total_latency_ms: i64,
    pub last_called_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ModelRecommendation {
    pub id: String,
    pub system_profile_id: String,
    pub model_id: String,
    pub recommendation_tier: String,
    pub score: f64,
    pub reasoning: String,
    pub performance_estimate: Option<String>,
    pub energy_rating: Option<String>,
    pub is_primary_recommendation: i64,
    pub computed_at: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct PerfLog {
    pub id: String,
    pub event_type: String,
    pub session_id: Option<String>,
    pub model_id: Option<String>,
    pub mcp_id: Option<String>,
    pub latency_ms: i64,
    pub tokens_in: Option<i64>,
    pub tokens_out: Option<i64>,
    pub tokens_per_sec: Option<f64>,
    pub cpu_usage_pct: Option<f64>,
    pub ram_usage_mb: Option<i64>,
    pub gpu_usage_pct: Option<f64>,
    pub success: i64,
    pub error_code: Option<String>,
    pub metadata: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ModelBenchmark {
    pub id: String,
    pub model_id: String,
    pub system_profile_id: Option<String>,
    pub context_tokens: i64,
    pub prompt_tokens: i64,
    pub output_tokens: i64,
    pub load_time_ms: Option<i64>,
    pub first_token_ms: Option<i64>,
    pub total_latency_ms: i64,
    pub tokens_per_sec: Option<f64>,
    pub memory_used_mb: Option<i64>,
    pub cpu_usage_pct: Option<f64>,
    pub success: i64,
    pub metadata: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct SetupState {
    pub id: String,
    pub user_id: Option<String>,
    pub status: String,
    pub current_stage: String,
    pub progress_pct: f64,
    pub selected_bundle: Option<String>,
    pub hardware_profile_id: Option<String>,
    pub last_error: Option<String>,
    pub metadata: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePolicy {
    pub pressure_cpu_pct: f64,
    pub pressure_memory_pct: f64,
    pub interactive_max_tokens: usize,
    pub background_max_tokens: usize,
    pub interactive_max_concurrency: usize,
    pub background_max_concurrency: usize,
    pub retrieval_candidate_limit: usize,
    pub defer_background_under_pressure: bool,
}

impl Default for RuntimePolicy {
    fn default() -> Self {
        Self {
            pressure_cpu_pct: 82.0,
            pressure_memory_pct: 85.0,
            interactive_max_tokens: 640,
            background_max_tokens: 256,
            interactive_max_concurrency: 1,
            background_max_concurrency: 1,
            retrieval_candidate_limit: 36,
            defer_background_under_pressure: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePolicyPatch {
    pub pressure_cpu_pct: Option<f64>,
    pub pressure_memory_pct: Option<f64>,
    pub interactive_max_tokens: Option<usize>,
    pub background_max_tokens: Option<usize>,
    pub interactive_max_concurrency: Option<usize>,
    pub background_max_concurrency: Option<usize>,
    pub retrieval_candidate_limit: Option<usize>,
    pub defer_background_under_pressure: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutingDecision {
    pub task_type: String,
    pub qos: String,
    pub selected_model_id: Option<String>,
    pub selected_model_name: Option<String>,
    pub max_tokens: usize,
    pub pressure_level: String,
    pub reason: String,
    pub fallback_chain: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutingPreviewRequest {
    pub user_id: String,
    pub session_id: Option<String>,
    pub content: String,
    pub task_type: Option<String>,
    pub qos: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceSummary {
    pub window_hours: i64,
    pub total_events: i64,
    pub success_rate: f64,
    pub p50_latency_ms: Option<f64>,
    pub p95_latency_ms: Option<f64>,
    pub avg_tokens_per_sec: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LiveSystemStats {
    pub cpu_usage_pct: f32,
    pub memory_used_mb: u64,
    pub memory_total_mb: u64,
    pub process_count: usize,
    pub gpu_name: Option<String>,
    pub gpu_usage_pct: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetrievedChunk {
    pub chunk: Chunk,
    pub vector_score: Option<f32>,
    pub bm25_score: Option<f32>,
    pub rerank_score: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolResult {
    pub mcp_id: String,
    pub tool_name: String,
    pub output: String,
    pub latency_ms: i64,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RankCandidate {
    pub id: String,
    pub text: String,
    pub metadata: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RankedResult {
    pub id: String,
    pub score: f32,
    pub metadata: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Intent {
    pub name: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entity {
    pub kind: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemporalRef {
    pub phrase: String,
    pub start: Option<String>,
    pub end: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssembledContext {
    pub system_prompt: String,
    pub messages: Vec<Message>,
    pub tools: Vec<Mcp>,
    pub memory_refs: Vec<Memory>,
    pub doc_refs: Vec<RetrievedChunk>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationOptions {
    pub temperature: f32,
    pub top_p: f32,
    pub max_tokens: usize,
}

impl Default for GenerationOptions {
    fn default() -> Self {
        Self {
            temperature: 0.2,
            top_p: 0.95,
            max_tokens: 512,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationResult {
    pub text: String,
    pub tokens_generated: usize,
    pub finish_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageStreamChunk {
    pub session_id: String,
    pub token: String,
    pub done: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct McpHealthStatus {
    pub mcp_id: String,
    pub health_status: String,
    pub last_error: Option<String>,
}
