CREATE TABLE IF NOT EXISTS mcps (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  display_name TEXT NOT NULL,
  description TEXT,
  version TEXT,
  author TEXT,
  icon_path TEXT,
  category TEXT NOT NULL,
  mcp_type TEXT NOT NULL,
  command TEXT,
  args TEXT NOT NULL DEFAULT '[]',
  env_vars TEXT NOT NULL DEFAULT '{}',
  url TEXT,
  tool_schemas TEXT NOT NULL DEFAULT '[]',
  resource_schemas TEXT NOT NULL DEFAULT '[]',
  prompt_schemas TEXT NOT NULL DEFAULT '[]',
  is_installed INTEGER NOT NULL DEFAULT 0,
  is_active INTEGER NOT NULL DEFAULT 0,
  is_builtin INTEGER NOT NULL DEFAULT 0,
  is_default INTEGER NOT NULL DEFAULT 0,
  health_status TEXT NOT NULL DEFAULT 'unknown',
  last_health_check_at TEXT,
  last_error TEXT,
  metadata TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL DEFAULT (datetime('now','utc')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now','utc'))
);
CREATE INDEX IF NOT EXISTS idx_mcps_is_active ON mcps(is_active);
CREATE INDEX IF NOT EXISTS idx_mcps_category ON mcps(category);

CREATE TABLE IF NOT EXISTS mcp_secrets (
  id TEXT PRIMARY KEY,
  mcp_id TEXT NOT NULL REFERENCES mcps(id) ON DELETE CASCADE,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  key_name TEXT NOT NULL,
  encrypted_value TEXT NOT NULL,
  nonce TEXT NOT NULL,
  key_hint TEXT,
  expires_at TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now','utc')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now','utc')),
  UNIQUE(mcp_id, user_id, key_name)
);

CREATE TABLE IF NOT EXISTS mcp_configs (
  id TEXT PRIMARY KEY,
  mcp_id TEXT NOT NULL REFERENCES mcps(id) ON DELETE CASCADE,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  config_json TEXT NOT NULL DEFAULT '{}',
  is_enabled INTEGER NOT NULL DEFAULT 1,
  priority INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (datetime('now','utc')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now','utc')),
  UNIQUE(mcp_id, user_id)
);

CREATE TABLE IF NOT EXISTS mcp_connection_states (
  id TEXT PRIMARY KEY,
  mcp_id TEXT NOT NULL REFERENCES mcps(id) ON DELETE CASCADE,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  status TEXT NOT NULL DEFAULT 'disconnected',
  connected_at TEXT,
  disconnected_at TEXT,
  last_used_at TEXT,
  error_count INTEGER NOT NULL DEFAULT 0,
  success_count INTEGER NOT NULL DEFAULT 0,
  avg_latency_ms REAL,
  circuit_breaker_state TEXT NOT NULL DEFAULT 'closed',
  created_at TEXT NOT NULL DEFAULT (datetime('now','utc')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now','utc')),
  UNIQUE(mcp_id, user_id)
);

CREATE TABLE IF NOT EXISTS mcp_usage_stats (
  id TEXT PRIMARY KEY,
  mcp_id TEXT NOT NULL REFERENCES mcps(id) ON DELETE CASCADE,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  tool_name TEXT NOT NULL,
  call_count INTEGER NOT NULL DEFAULT 0,
  success_count INTEGER NOT NULL DEFAULT 0,
  error_count INTEGER NOT NULL DEFAULT 0,
  total_latency_ms INTEGER NOT NULL DEFAULT 0,
  last_called_at TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now','utc')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now','utc')),
  UNIQUE(mcp_id, user_id, tool_name)
);

CREATE TRIGGER IF NOT EXISTS trg_mcps_updated_at
AFTER UPDATE ON mcps
FOR EACH ROW WHEN NEW.updated_at = OLD.updated_at
BEGIN
  UPDATE mcps SET updated_at = datetime('now','utc') WHERE id = OLD.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_mcp_secrets_updated_at
AFTER UPDATE ON mcp_secrets
FOR EACH ROW WHEN NEW.updated_at = OLD.updated_at
BEGIN
  UPDATE mcp_secrets SET updated_at = datetime('now','utc') WHERE id = OLD.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_mcp_configs_updated_at
AFTER UPDATE ON mcp_configs
FOR EACH ROW WHEN NEW.updated_at = OLD.updated_at
BEGIN
  UPDATE mcp_configs SET updated_at = datetime('now','utc') WHERE id = OLD.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_mcp_connection_states_updated_at
AFTER UPDATE ON mcp_connection_states
FOR EACH ROW WHEN NEW.updated_at = OLD.updated_at
BEGIN
  UPDATE mcp_connection_states SET updated_at = datetime('now','utc') WHERE id = OLD.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_mcp_usage_stats_updated_at
AFTER UPDATE ON mcp_usage_stats
FOR EACH ROW WHEN NEW.updated_at = OLD.updated_at
BEGIN
  UPDATE mcp_usage_stats SET updated_at = datetime('now','utc') WHERE id = OLD.id;
END;
