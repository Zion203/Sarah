CREATE TABLE IF NOT EXISTS perf_logs (
  id TEXT PRIMARY KEY,
  event_type TEXT NOT NULL,
  session_id TEXT,
  model_id TEXT,
  mcp_id TEXT,
  latency_ms INTEGER NOT NULL,
  tokens_in INTEGER,
  tokens_out INTEGER,
  tokens_per_sec REAL,
  cpu_usage_pct REAL,
  ram_usage_mb INTEGER,
  gpu_usage_pct REAL,
  success INTEGER NOT NULL DEFAULT 1,
  error_code TEXT,
  metadata TEXT DEFAULT '{}',
  created_at TEXT NOT NULL DEFAULT (datetime('now','utc'))
);
CREATE INDEX IF NOT EXISTS idx_perf_logs_event_type ON perf_logs(event_type, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_perf_logs_model_id ON perf_logs(model_id);
CREATE INDEX IF NOT EXISTS idx_perf_logs_created_at ON perf_logs(created_at DESC);

CREATE TABLE IF NOT EXISTS error_reports (
  id TEXT PRIMARY KEY,
  error_code TEXT NOT NULL,
  error_message TEXT NOT NULL,
  stack_trace TEXT,
  component TEXT NOT NULL,
  severity TEXT NOT NULL DEFAULT 'error',
  is_resolved INTEGER NOT NULL DEFAULT 0,
  metadata TEXT DEFAULT '{}',
  created_at TEXT NOT NULL DEFAULT (datetime('now','utc'))
);
CREATE INDEX IF NOT EXISTS idx_error_reports_severity ON error_reports(severity, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_error_reports_component ON error_reports(component);

CREATE TABLE IF NOT EXISTS model_recommendations (
  id TEXT PRIMARY KEY,
  system_profile_id TEXT NOT NULL REFERENCES system_profile(id),
  model_id TEXT NOT NULL REFERENCES models(id),
  recommendation_tier TEXT NOT NULL,
  score REAL NOT NULL,
  reasoning TEXT NOT NULL,
  performance_estimate TEXT,
  energy_rating TEXT,
  is_primary_recommendation INTEGER NOT NULL DEFAULT 0,
  computed_at TEXT NOT NULL DEFAULT (datetime('now','utc')),
  created_at TEXT NOT NULL DEFAULT (datetime('now','utc'))
);
CREATE INDEX IF NOT EXISTS idx_recommendations_profile ON model_recommendations(system_profile_id);
CREATE INDEX IF NOT EXISTS idx_recommendations_tier ON model_recommendations(recommendation_tier);
