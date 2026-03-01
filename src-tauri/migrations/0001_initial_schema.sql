CREATE TABLE IF NOT EXISTS system_profile (
  id TEXT PRIMARY KEY,
  cpu_brand TEXT NOT NULL,
  cpu_cores INTEGER NOT NULL,
  cpu_threads INTEGER NOT NULL,
  cpu_frequency_mhz INTEGER,
  total_ram_mb INTEGER NOT NULL,
  available_ram_mb INTEGER,
  gpu_name TEXT,
  gpu_vendor TEXT,
  gpu_vram_mb INTEGER,
  gpu_backend TEXT,
  storage_total_gb INTEGER,
  storage_available_gb INTEGER,
  storage_type TEXT,
  os_name TEXT NOT NULL,
  os_version TEXT NOT NULL,
  os_arch TEXT NOT NULL,
  platform TEXT NOT NULL,
  benchmark_tokens_per_sec REAL,
  benchmark_embed_ms REAL,
  capability_score REAL,
  supports_cuda INTEGER DEFAULT 0,
  supports_metal INTEGER DEFAULT 0,
  supports_vulkan INTEGER DEFAULT 0,
  supports_avx2 INTEGER DEFAULT 0,
  supports_avx512 INTEGER DEFAULT 0,
  last_scan_at TEXT NOT NULL DEFAULT (datetime('now','utc')),
  created_at TEXT NOT NULL DEFAULT (datetime('now','utc')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now','utc'))
);

CREATE TABLE IF NOT EXISTS models (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  display_name TEXT NOT NULL,
  family TEXT NOT NULL,
  version TEXT,
  parameter_count TEXT,
  quantization TEXT,
  file_format TEXT NOT NULL,
  file_path TEXT,
  file_size_mb INTEGER,
  context_length INTEGER NOT NULL DEFAULT 4096,
  embedding_size INTEGER,
  category TEXT NOT NULL,
  capabilities TEXT NOT NULL DEFAULT '[]',
  min_ram_mb INTEGER NOT NULL,
  recommended_ram_mb INTEGER NOT NULL,
  min_vram_mb INTEGER DEFAULT 0,
  performance_tier TEXT NOT NULL,
  energy_tier TEXT NOT NULL,
  compatibility_score REAL,
  is_downloaded INTEGER NOT NULL DEFAULT 0,
  is_active INTEGER NOT NULL DEFAULT 0,
  is_default INTEGER NOT NULL DEFAULT 0,
  is_recommended INTEGER NOT NULL DEFAULT 0,
  download_url TEXT,
  sha256_checksum TEXT,
  tags TEXT NOT NULL DEFAULT '[]',
  metadata TEXT NOT NULL DEFAULT '{}',
  last_used_at TEXT,
  tokens_generated INTEGER NOT NULL DEFAULT 0,
  avg_tokens_per_sec REAL,
  created_at TEXT NOT NULL DEFAULT (datetime('now','utc')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now','utc'))
);
CREATE INDEX IF NOT EXISTS idx_models_category ON models(category);
CREATE INDEX IF NOT EXISTS idx_models_is_downloaded ON models(is_downloaded);
CREATE INDEX IF NOT EXISTS idx_models_performance_tier ON models(performance_tier);
CREATE INDEX IF NOT EXISTS idx_models_is_default ON models(is_default);

CREATE TABLE IF NOT EXISTS model_downloads (
  id TEXT PRIMARY KEY,
  model_id TEXT NOT NULL REFERENCES models(id) ON DELETE CASCADE,
  status TEXT NOT NULL,
  bytes_downloaded INTEGER NOT NULL DEFAULT 0,
  bytes_total INTEGER,
  progress_pct REAL DEFAULT 0.0,
  started_at TEXT,
  completed_at TEXT,
  error_message TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now','utc')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now','utc'))
);
CREATE INDEX IF NOT EXISTS idx_model_downloads_model_id ON model_downloads(model_id);
CREATE INDEX IF NOT EXISTS idx_model_downloads_status ON model_downloads(status);

CREATE TABLE IF NOT EXISTS users (
  id TEXT PRIMARY KEY,
  username TEXT NOT NULL UNIQUE,
  display_name TEXT NOT NULL,
  avatar_path TEXT,
  bio TEXT,
  locale TEXT NOT NULL DEFAULT 'en',
  timezone TEXT NOT NULL DEFAULT 'UTC',
  is_active INTEGER NOT NULL DEFAULT 1,
  last_active_at TEXT,
  metadata TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL DEFAULT (datetime('now','utc')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now','utc'))
);

CREATE TABLE IF NOT EXISTS settings (
  id TEXT PRIMARY KEY,
  user_id TEXT REFERENCES users(id) ON DELETE CASCADE,
  namespace TEXT NOT NULL,
  key TEXT NOT NULL,
  value TEXT NOT NULL,
  value_type TEXT NOT NULL,
  is_encrypted INTEGER NOT NULL DEFAULT 0,
  description TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now','utc')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now','utc')),
  UNIQUE(user_id, namespace, key)
);
CREATE INDEX IF NOT EXISTS idx_settings_namespace ON settings(namespace);
CREATE INDEX IF NOT EXISTS idx_settings_user_id ON settings(user_id);

CREATE TABLE IF NOT EXISTS feature_flags (
  id TEXT PRIMARY KEY,
  user_id TEXT REFERENCES users(id) ON DELETE CASCADE,
  flag_name TEXT NOT NULL,
  is_enabled INTEGER NOT NULL DEFAULT 0,
  rollout_percentage INTEGER DEFAULT 100,
  conditions TEXT DEFAULT '{}',
  created_at TEXT NOT NULL DEFAULT (datetime('now','utc')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now','utc')),
  UNIQUE(user_id, flag_name)
);

CREATE TRIGGER IF NOT EXISTS trg_system_profile_updated_at
AFTER UPDATE ON system_profile
FOR EACH ROW WHEN NEW.updated_at = OLD.updated_at
BEGIN
  UPDATE system_profile SET updated_at = datetime('now','utc') WHERE id = OLD.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_models_updated_at
AFTER UPDATE ON models
FOR EACH ROW WHEN NEW.updated_at = OLD.updated_at
BEGIN
  UPDATE models SET updated_at = datetime('now','utc') WHERE id = OLD.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_model_downloads_updated_at
AFTER UPDATE ON model_downloads
FOR EACH ROW WHEN NEW.updated_at = OLD.updated_at
BEGIN
  UPDATE model_downloads SET updated_at = datetime('now','utc') WHERE id = OLD.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_users_updated_at
AFTER UPDATE ON users
FOR EACH ROW WHEN NEW.updated_at = OLD.updated_at
BEGIN
  UPDATE users SET updated_at = datetime('now','utc') WHERE id = OLD.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_settings_updated_at
AFTER UPDATE ON settings
FOR EACH ROW WHEN NEW.updated_at = OLD.updated_at
BEGIN
  UPDATE settings SET updated_at = datetime('now','utc') WHERE id = OLD.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_feature_flags_updated_at
AFTER UPDATE ON feature_flags
FOR EACH ROW WHEN NEW.updated_at = OLD.updated_at
BEGIN
  UPDATE feature_flags SET updated_at = datetime('now','utc') WHERE id = OLD.id;
END;
