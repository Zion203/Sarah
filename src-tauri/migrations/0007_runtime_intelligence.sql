CREATE TABLE IF NOT EXISTS runtime_policy_overrides (
  id TEXT PRIMARY KEY,
  user_id TEXT REFERENCES users(id) ON DELETE CASCADE,
  policy_json TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1,
  source TEXT NOT NULL DEFAULT 'system',
  created_at TEXT NOT NULL DEFAULT (datetime('now','utc')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now','utc'))
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_runtime_policy_scope
  ON runtime_policy_overrides(COALESCE(user_id, '__global__'));

CREATE TABLE IF NOT EXISTS model_benchmarks (
  id TEXT PRIMARY KEY,
  model_id TEXT NOT NULL REFERENCES models(id) ON DELETE CASCADE,
  system_profile_id TEXT REFERENCES system_profile(id),
  context_tokens INTEGER NOT NULL DEFAULT 0,
  prompt_tokens INTEGER NOT NULL DEFAULT 0,
  output_tokens INTEGER NOT NULL DEFAULT 0,
  load_time_ms INTEGER,
  first_token_ms INTEGER,
  total_latency_ms INTEGER NOT NULL,
  tokens_per_sec REAL,
  memory_used_mb INTEGER,
  cpu_usage_pct REAL,
  success INTEGER NOT NULL DEFAULT 1,
  metadata TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL DEFAULT (datetime('now','utc'))
);
CREATE INDEX IF NOT EXISTS idx_model_benchmarks_model
  ON model_benchmarks(model_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_model_benchmarks_profile
  ON model_benchmarks(system_profile_id, created_at DESC);

CREATE TABLE IF NOT EXISTS routing_events (
  id TEXT PRIMARY KEY,
  user_id TEXT REFERENCES users(id) ON DELETE CASCADE,
  session_id TEXT REFERENCES sessions(id) ON DELETE CASCADE,
  requested_task_type TEXT,
  requested_qos TEXT,
  selected_model_id TEXT REFERENCES models(id),
  fallback_chain TEXT NOT NULL DEFAULT '[]',
  pressure_level TEXT NOT NULL DEFAULT 'normal',
  max_tokens INTEGER NOT NULL DEFAULT 512,
  reason TEXT,
  latency_ms INTEGER,
  success INTEGER NOT NULL DEFAULT 1,
  error_code TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now','utc'))
);
CREATE INDEX IF NOT EXISTS idx_routing_events_created
  ON routing_events(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_routing_events_session
  ON routing_events(session_id, created_at DESC);

CREATE TABLE IF NOT EXISTS background_job_runs (
  id TEXT PRIMARY KEY,
  job_type TEXT NOT NULL,
  status TEXT NOT NULL,
  deferred_reason TEXT,
  started_at TEXT NOT NULL DEFAULT (datetime('now','utc')),
  completed_at TEXT,
  latency_ms INTEGER,
  metadata TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL DEFAULT (datetime('now','utc'))
);
CREATE INDEX IF NOT EXISTS idx_background_job_runs_type
  ON background_job_runs(job_type, created_at DESC);

CREATE TABLE IF NOT EXISTS setup_state (
  id TEXT PRIMARY KEY,
  user_id TEXT REFERENCES users(id) ON DELETE CASCADE,
  status TEXT NOT NULL,
  current_stage TEXT NOT NULL,
  progress_pct REAL NOT NULL DEFAULT 0,
  selected_bundle TEXT,
  hardware_profile_id TEXT REFERENCES system_profile(id),
  last_error TEXT,
  metadata TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL DEFAULT (datetime('now','utc')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now','utc'))
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_setup_state_scope
  ON setup_state(COALESCE(user_id, '__global__'));

CREATE INDEX IF NOT EXISTS idx_perf_logs_event_created_success
  ON perf_logs(event_type, created_at DESC, success);
CREATE INDEX IF NOT EXISTS idx_embeddings_user_namespace_entity
  ON embeddings(user_id, namespace, entity_type);
CREATE INDEX IF NOT EXISTS idx_document_chunks_user_doc_index
  ON document_chunks(user_id, document_id, chunk_index);
CREATE INDEX IF NOT EXISTS idx_messages_session_created
  ON messages(session_id, created_at DESC);

CREATE TRIGGER IF NOT EXISTS trg_runtime_policy_updated_at
AFTER UPDATE ON runtime_policy_overrides
FOR EACH ROW WHEN NEW.updated_at = OLD.updated_at
BEGIN
  UPDATE runtime_policy_overrides
  SET updated_at = datetime('now','utc')
  WHERE id = OLD.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_setup_state_updated_at
AFTER UPDATE ON setup_state
FOR EACH ROW WHEN NEW.updated_at = OLD.updated_at
BEGIN
  UPDATE setup_state
  SET updated_at = datetime('now','utc')
  WHERE id = OLD.id;
END;
