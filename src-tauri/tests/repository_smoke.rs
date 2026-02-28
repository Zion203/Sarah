use sarah_lib::db::models::NewMessage;
use sarah_lib::repositories::conversation_repo::ConversationRepo;

#[tokio::test]
async fn conversation_repo_roundtrip() {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:")
        .await
        .expect("memory sqlite");

    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await
        .expect("pragma");

    sqlx::query(
        r#"
        CREATE TABLE users (
          id TEXT PRIMARY KEY,
          username TEXT NOT NULL UNIQUE,
          display_name TEXT NOT NULL,
          locale TEXT NOT NULL DEFAULT 'en',
          timezone TEXT NOT NULL DEFAULT 'UTC',
          is_active INTEGER NOT NULL DEFAULT 1,
          metadata TEXT NOT NULL DEFAULT '{}',
          created_at TEXT NOT NULL DEFAULT (datetime('now','utc')),
          updated_at TEXT NOT NULL DEFAULT (datetime('now','utc'))
        );

        CREATE TABLE sessions (
          id TEXT PRIMARY KEY,
          user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
          title TEXT,
          model_id TEXT,
          system_prompt TEXT,
          context_window_used INTEGER DEFAULT 0,
          token_count INTEGER NOT NULL DEFAULT 0,
          message_count INTEGER NOT NULL DEFAULT 0,
          status TEXT NOT NULL DEFAULT 'active',
          summary TEXT,
          tags TEXT NOT NULL DEFAULT '[]',
          pinned INTEGER NOT NULL DEFAULT 0,
          forked_from_session_id TEXT,
          forked_at_message_id TEXT,
          metadata TEXT NOT NULL DEFAULT '{}',
          last_message_at TEXT,
          created_at TEXT NOT NULL DEFAULT (datetime('now','utc')),
          updated_at TEXT NOT NULL DEFAULT (datetime('now','utc'))
        );

        CREATE TABLE messages (
          id TEXT PRIMARY KEY,
          session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
          role TEXT NOT NULL,
          content TEXT NOT NULL,
          content_type TEXT NOT NULL DEFAULT 'text',
          thinking TEXT,
          token_count INTEGER,
          model_id TEXT,
          latency_ms INTEGER,
          tokens_per_sec REAL,
          finish_reason TEXT,
          is_error INTEGER NOT NULL DEFAULT 0,
          error_message TEXT,
          parent_message_id TEXT,
          edited_at TEXT,
          original_content TEXT,
          metadata TEXT NOT NULL DEFAULT '{}',
          position INTEGER NOT NULL,
          created_at TEXT NOT NULL DEFAULT (datetime('now','utc')),
          updated_at TEXT NOT NULL DEFAULT (datetime('now','utc'))
        );

        CREATE TABLE tool_calls (
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

        CREATE VIRTUAL TABLE messages_fts USING fts5(content, session_id UNINDEXED, message_id UNINDEXED);
        "#,
    )
    .execute(&pool)
    .await
    .expect("schema");

    sqlx::query(
        "INSERT INTO users (id, username, display_name) VALUES ('u1', 'default', 'Default User')",
    )
    .execute(&pool)
    .await
    .expect("user");

    let repo = ConversationRepo::new(pool.clone());

    let session = repo
        .create_session("u1", None)
        .await
        .expect("create session");

    let message = repo
        .insert_message(NewMessage {
            session_id: session.id.clone(),
            role: "user".to_string(),
            content: "hello world".to_string(),
            content_type: "text".to_string(),
            token_count: Some(3),
            model_id: None,
            metadata: "{}".to_string(),
            position: 0,
        })
        .await
        .expect("insert message");

    assert_eq!(message.session_id, session.id);

    let list = repo
        .get_messages(&session.id, 20, 0)
        .await
        .expect("get messages");
    assert_eq!(list.len(), 1);
}
