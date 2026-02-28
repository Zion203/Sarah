CREATE TABLE IF NOT EXISTS memories (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  memory_type TEXT NOT NULL,
  category TEXT,
  subject TEXT,
  predicate TEXT,
  object TEXT,
  content TEXT NOT NULL,
  summary TEXT,
  source TEXT NOT NULL,
  source_id TEXT,
  session_id TEXT REFERENCES sessions(id),
  confidence REAL NOT NULL DEFAULT 0.8,
  importance REAL NOT NULL DEFAULT 0.5,
  decay_rate REAL NOT NULL DEFAULT 0.001,
  access_count INTEGER NOT NULL DEFAULT 0,
  last_accessed_at TEXT,
  privacy_level TEXT NOT NULL DEFAULT 'private',
  is_verified INTEGER NOT NULL DEFAULT 0,
  is_archived INTEGER NOT NULL DEFAULT 0,
  is_pinned INTEGER NOT NULL DEFAULT 0,
  expires_at TEXT,
  tags TEXT NOT NULL DEFAULT '[]',
  embedding_id TEXT,
  metadata TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL DEFAULT (datetime('now','utc')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now','utc'))
);
CREATE INDEX IF NOT EXISTS idx_memories_user_id ON memories(user_id);
CREATE INDEX IF NOT EXISTS idx_memories_type ON memories(memory_type);
CREATE INDEX IF NOT EXISTS idx_memories_importance ON memories(importance DESC);
CREATE INDEX IF NOT EXISTS idx_memories_created_at ON memories(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_memories_source ON memories(source, source_id);
CREATE INDEX IF NOT EXISTS idx_memories_is_archived ON memories(is_archived);

CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
  content,
  subject,
  object,
  tags,
  memory_id UNINDEXED,
  user_id UNINDEXED,
  tokenize='porter unicode61'
);

CREATE TABLE IF NOT EXISTS memory_relations (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  source_memory_id TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
  target_memory_id TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
  relation_type TEXT NOT NULL,
  strength REAL NOT NULL DEFAULT 0.5,
  created_at TEXT NOT NULL DEFAULT (datetime('now','utc')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now','utc'))
);
CREATE INDEX IF NOT EXISTS idx_memory_relations_source ON memory_relations(source_memory_id);
CREATE INDEX IF NOT EXISTS idx_memory_relations_target ON memory_relations(target_memory_id);

CREATE TRIGGER IF NOT EXISTS trg_memories_fts_insert
AFTER INSERT ON memories
BEGIN
  INSERT INTO memories_fts (rowid, content, subject, object, tags, memory_id, user_id)
  VALUES (new.rowid, new.content, new.subject, new.object, new.tags, new.id, new.user_id);
END;

CREATE TRIGGER IF NOT EXISTS trg_memories_fts_delete
AFTER DELETE ON memories
BEGIN
  INSERT INTO memories_fts(memories_fts, rowid, content, subject, object, tags, memory_id, user_id)
  VALUES('delete', old.rowid, old.content, old.subject, old.object, old.tags, old.id, old.user_id);
END;

CREATE TRIGGER IF NOT EXISTS trg_memories_fts_update
AFTER UPDATE ON memories
BEGIN
  INSERT INTO memories_fts(memories_fts, rowid, content, subject, object, tags, memory_id, user_id)
  VALUES('delete', old.rowid, old.content, old.subject, old.object, old.tags, old.id, old.user_id);
  INSERT INTO memories_fts(rowid, content, subject, object, tags, memory_id, user_id)
  VALUES (new.rowid, new.content, new.subject, new.object, new.tags, new.id, new.user_id);
END;

CREATE TRIGGER IF NOT EXISTS trg_memories_updated_at
AFTER UPDATE ON memories
FOR EACH ROW WHEN NEW.updated_at = OLD.updated_at
BEGIN
  UPDATE memories SET updated_at = datetime('now','utc') WHERE id = OLD.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_memory_relations_updated_at
AFTER UPDATE ON memory_relations
FOR EACH ROW WHEN NEW.updated_at = OLD.updated_at
BEGIN
  UPDATE memory_relations SET updated_at = datetime('now','utc') WHERE id = OLD.id;
END;
