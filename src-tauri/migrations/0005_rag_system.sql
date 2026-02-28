CREATE TABLE IF NOT EXISTS documents (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  title TEXT NOT NULL,
  file_path TEXT,
  source_url TEXT,
  source_type TEXT NOT NULL,
  mime_type TEXT,
  file_size_bytes INTEGER,
  language TEXT DEFAULT 'en',
  namespace TEXT NOT NULL DEFAULT 'personal',
  index_status TEXT NOT NULL DEFAULT 'pending',
  chunk_count INTEGER DEFAULT 0,
  token_count INTEGER DEFAULT 0,
  checksum TEXT,
  access_level TEXT NOT NULL DEFAULT 'private',
  version INTEGER NOT NULL DEFAULT 1,
  parent_document_id TEXT REFERENCES documents(id),
  is_deleted INTEGER NOT NULL DEFAULT 0,
  last_indexed_at TEXT,
  metadata TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL DEFAULT (datetime('now','utc')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now','utc'))
);
CREATE INDEX IF NOT EXISTS idx_documents_user_id ON documents(user_id);
CREATE INDEX IF NOT EXISTS idx_documents_namespace ON documents(namespace);
CREATE INDEX IF NOT EXISTS idx_documents_index_status ON documents(index_status);
CREATE INDEX IF NOT EXISTS idx_documents_source_type ON documents(source_type);

CREATE TABLE IF NOT EXISTS document_chunks (
  id TEXT PRIMARY KEY,
  document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  chunk_index INTEGER NOT NULL,
  content TEXT NOT NULL,
  token_count INTEGER NOT NULL,
  start_char INTEGER,
  end_char INTEGER,
  page_number INTEGER,
  section_title TEXT,
  heading_path TEXT,
  embedding_id TEXT,
  metadata TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL DEFAULT (datetime('now','utc')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now','utc'))
);
CREATE INDEX IF NOT EXISTS idx_chunks_document_id ON document_chunks(document_id, chunk_index);
CREATE INDEX IF NOT EXISTS idx_chunks_user_id ON document_chunks(user_id);

CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
  content,
  section_title,
  chunk_id UNINDEXED,
  document_id UNINDEXED,
  user_id UNINDEXED,
  tokenize='porter unicode61'
);

CREATE TABLE IF NOT EXISTS embeddings (
  id TEXT PRIMARY KEY,
  entity_type TEXT NOT NULL,
  entity_id TEXT NOT NULL,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  namespace TEXT NOT NULL DEFAULT 'default',
  model_name TEXT NOT NULL,
  vector BLOB NOT NULL,
  dimensions INTEGER NOT NULL,
  norm REAL,
  created_at TEXT NOT NULL DEFAULT (datetime('now','utc')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now','utc')),
  UNIQUE(entity_type, entity_id)
);
CREATE INDEX IF NOT EXISTS idx_embeddings_entity ON embeddings(entity_type, entity_id);
CREATE INDEX IF NOT EXISTS idx_embeddings_namespace ON embeddings(namespace);
CREATE INDEX IF NOT EXISTS idx_embeddings_user_id ON embeddings(user_id);

CREATE TABLE IF NOT EXISTS rag_retrievals (
  id TEXT PRIMARY KEY,
  session_id TEXT REFERENCES sessions(id),
  query_text TEXT NOT NULL,
  query_embedding_id TEXT REFERENCES embeddings(id),
  retrieved_chunk_ids TEXT NOT NULL DEFAULT '[]',
  reranked_chunk_ids TEXT NOT NULL DEFAULT '[]',
  strategy TEXT NOT NULL,
  latency_ms INTEGER,
  feedback_score REAL,
  created_at TEXT NOT NULL DEFAULT (datetime('now','utc'))
);

CREATE TRIGGER IF NOT EXISTS trg_chunks_fts_insert
AFTER INSERT ON document_chunks
BEGIN
  INSERT INTO chunks_fts (rowid, content, section_title, chunk_id, document_id, user_id)
  VALUES (new.rowid, new.content, new.section_title, new.id, new.document_id, new.user_id);
END;

CREATE TRIGGER IF NOT EXISTS trg_chunks_fts_delete
AFTER DELETE ON document_chunks
BEGIN
  INSERT INTO chunks_fts(chunks_fts, rowid, content, section_title, chunk_id, document_id, user_id)
  VALUES('delete', old.rowid, old.content, old.section_title, old.id, old.document_id, old.user_id);
END;

CREATE TRIGGER IF NOT EXISTS trg_chunks_fts_update
AFTER UPDATE ON document_chunks
BEGIN
  INSERT INTO chunks_fts(chunks_fts, rowid, content, section_title, chunk_id, document_id, user_id)
  VALUES('delete', old.rowid, old.content, old.section_title, old.id, old.document_id, old.user_id);
  INSERT INTO chunks_fts(rowid, content, section_title, chunk_id, document_id, user_id)
  VALUES (new.rowid, new.content, new.section_title, new.id, new.document_id, new.user_id);
END;

CREATE TRIGGER IF NOT EXISTS trg_documents_updated_at
AFTER UPDATE ON documents
FOR EACH ROW WHEN NEW.updated_at = OLD.updated_at
BEGIN
  UPDATE documents SET updated_at = datetime('now','utc') WHERE id = OLD.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_document_chunks_updated_at
AFTER UPDATE ON document_chunks
FOR EACH ROW WHEN NEW.updated_at = OLD.updated_at
BEGIN
  UPDATE document_chunks SET updated_at = datetime('now','utc') WHERE id = OLD.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_embeddings_updated_at
AFTER UPDATE ON embeddings
FOR EACH ROW WHEN NEW.updated_at = OLD.updated_at
BEGIN
  UPDATE embeddings SET updated_at = datetime('now','utc') WHERE id = OLD.id;
END;
