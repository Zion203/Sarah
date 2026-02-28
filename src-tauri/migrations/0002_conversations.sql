CREATE TABLE IF NOT EXISTS sessions (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  title TEXT,
  model_id TEXT REFERENCES models(id),
  system_prompt TEXT,
  context_window_used INTEGER DEFAULT 0,
  token_count INTEGER NOT NULL DEFAULT 0,
  message_count INTEGER NOT NULL DEFAULT 0,
  status TEXT NOT NULL DEFAULT 'active',
  summary TEXT,
  tags TEXT NOT NULL DEFAULT '[]',
  pinned INTEGER NOT NULL DEFAULT 0,
  forked_from_session_id TEXT REFERENCES sessions(id),
  forked_at_message_id TEXT,
  metadata TEXT NOT NULL DEFAULT '{}',
  last_message_at TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now','utc')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now','utc'))
);
CREATE INDEX IF NOT EXISTS idx_sessions_user_id ON sessions(user_id);
CREATE INDEX IF NOT EXISTS idx_sessions_status ON sessions(status);
CREATE INDEX IF NOT EXISTS idx_sessions_last_message_at ON sessions(last_message_at DESC);
CREATE INDEX IF NOT EXISTS idx_sessions_model_id ON sessions(model_id);

CREATE TABLE IF NOT EXISTS messages (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
  role TEXT NOT NULL,
  content TEXT NOT NULL,
  content_type TEXT NOT NULL DEFAULT 'text',
  thinking TEXT,
  token_count INTEGER,
  model_id TEXT REFERENCES models(id),
  latency_ms INTEGER,
  tokens_per_sec REAL,
  finish_reason TEXT,
  is_error INTEGER NOT NULL DEFAULT 0,
  error_message TEXT,
  parent_message_id TEXT REFERENCES messages(id),
  edited_at TEXT,
  original_content TEXT,
  metadata TEXT NOT NULL DEFAULT '{}',
  position INTEGER NOT NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now','utc')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now','utc'))
);
CREATE INDEX IF NOT EXISTS idx_messages_session_id ON messages(session_id, position);
CREATE INDEX IF NOT EXISTS idx_messages_role ON messages(role);
CREATE INDEX IF NOT EXISTS idx_messages_created_at ON messages(created_at DESC);

CREATE TABLE IF NOT EXISTS attachments (
  id TEXT PRIMARY KEY,
  message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
  session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
  file_name TEXT NOT NULL,
  file_path TEXT,
  mime_type TEXT NOT NULL,
  file_size_bytes INTEGER NOT NULL,
  content TEXT,
  checksum TEXT,
  is_processed INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (datetime('now','utc')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now','utc'))
);
CREATE INDEX IF NOT EXISTS idx_attachments_message_id ON attachments(message_id);
CREATE INDEX IF NOT EXISTS idx_attachments_session_id ON attachments(session_id);

CREATE TABLE IF NOT EXISTS tool_calls (
  id TEXT PRIMARY KEY,
  message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
  session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
  mcp_id TEXT,
  tool_name TEXT NOT NULL,
  tool_input TEXT NOT NULL,
  tool_output TEXT,
  status TEXT NOT NULL DEFAULT 'pending',
  error_message TEXT,
  latency_ms INTEGER,
  started_at TEXT,
  completed_at TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now','utc')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now','utc'))
);
CREATE INDEX IF NOT EXISTS idx_tool_calls_message_id ON tool_calls(message_id);
CREATE INDEX IF NOT EXISTS idx_tool_calls_mcp_id ON tool_calls(mcp_id);
CREATE INDEX IF NOT EXISTS idx_tool_calls_tool_name ON tool_calls(tool_name);

CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
  content,
  session_id UNINDEXED,
  message_id UNINDEXED,
  tokenize='porter unicode61'
);

CREATE TRIGGER IF NOT EXISTS trg_messages_fts_insert
AFTER INSERT ON messages
BEGIN
  INSERT INTO messages_fts (rowid, content, session_id, message_id)
  VALUES (new.rowid, new.content, new.session_id, new.id);
END;

CREATE TRIGGER IF NOT EXISTS trg_messages_fts_delete
AFTER DELETE ON messages
BEGIN
  INSERT INTO messages_fts(messages_fts, rowid, content, session_id, message_id)
  VALUES('delete', old.rowid, old.content, old.session_id, old.id);
END;

CREATE TRIGGER IF NOT EXISTS trg_messages_fts_update
AFTER UPDATE ON messages
BEGIN
  INSERT INTO messages_fts(messages_fts, rowid, content, session_id, message_id)
  VALUES('delete', old.rowid, old.content, old.session_id, old.id);
  INSERT INTO messages_fts(rowid, content, session_id, message_id)
  VALUES (new.rowid, new.content, new.session_id, new.id);
END;

CREATE TRIGGER IF NOT EXISTS trg_sessions_updated_at
AFTER UPDATE ON sessions
FOR EACH ROW WHEN NEW.updated_at = OLD.updated_at
BEGIN
  UPDATE sessions SET updated_at = datetime('now','utc') WHERE id = OLD.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_messages_updated_at
AFTER UPDATE ON messages
FOR EACH ROW WHEN NEW.updated_at = OLD.updated_at
BEGIN
  UPDATE messages SET updated_at = datetime('now','utc') WHERE id = OLD.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_attachments_updated_at
AFTER UPDATE ON attachments
FOR EACH ROW WHEN NEW.updated_at = OLD.updated_at
BEGIN
  UPDATE attachments SET updated_at = datetime('now','utc') WHERE id = OLD.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_tool_calls_updated_at
AFTER UPDATE ON tool_calls
FOR EACH ROW WHEN NEW.updated_at = OLD.updated_at
BEGIN
  UPDATE tool_calls SET updated_at = datetime('now','utc') WHERE id = OLD.id;
END;
