import { getCurrentWindow } from "@tauri-apps/api/window";
import { Clock3, History, Trash2, Search, ArrowLeft, MessageSquare } from "lucide-react";
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "@/components/ui/button";

interface HistoryWindowProps {
  embedded?: boolean;
  onRequestClose?: () => void;
}

interface Session {
  id: string;
  title: string | null;
  messageCount: number;
  lastMessageAt: string | null;
  createdAt: string;
}

interface Message {
  id: string;
  role: string;
  content: string;
  createdAt: string;
}

interface SearchResult {
  id: string;
  sessionId: string;
  role: string;
  content: string;
  createdAt: string;
}

export default function HistoryWindow({ embedded = false, onRequestClose }: HistoryWindowProps) {
  const [sessions, setSessions] = useState<Session[]>([]);
  const [selectedSession, setSelectedSession] = useState<{ id: string; messages: Message[] } | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<SearchResult[] | null>(null);

  const sync = async () => {
    try {
      const user = await invoke<{ id: string }>("get_default_user");
      const dbSessions = await invoke<Session[]>("list_sessions", {
        userId: user.id,
        limit: 50,
      });
      setSessions(dbSessions);
    } catch (e) {
      console.error("Failed to sync sessions:", e);
    }
  };

  useEffect(() => {
    sync();
    // No polling, relying on manual refresh or reopening
  }, []);

  useEffect(() => {
    if (!searchQuery.trim()) {
      setSearchResults(null);
      return;
    }

    const timer = setTimeout(async () => {
      try {
        const user = await invoke<{ id: string }>("get_default_user");
        const results = await invoke<SearchResult[]>("search_conversations", {
          userId: user.id,
          query: searchQuery,
        });
        setSearchResults(results);
      } catch (e) {
        console.error("Search failed:", e);
      }
    }, 300);

    return () => clearTimeout(timer);
  }, [searchQuery]);

  const handleClose = () => {
    if (embedded) {
      onRequestClose?.();
      return;
    }
    void getCurrentWindow().close();
  };

  const handleDeleteSession = async (sessionId: string, e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      if (selectedSession?.id === sessionId) {
        setSelectedSession(null);
      }
      await invoke("archive_session", { sessionId });
      sync();
    } catch (error) {
      console.error("Failed to delete session", error);
    }
  };

  const handleSelectSession = async (sessionId: string) => {
    try {
      const messages = await invoke<Message[]>("get_session_messages", {
        sessionId,
        limit: 200,
      });
      setSelectedSession({ id: sessionId, messages: messages.reverse() });
    } catch (error) {
      console.error("Failed to load session messages", error);
    }
  };

  const handleSelectSearchResult = (sessionId: string) => {
    setSearchQuery("");
    setSearchResults(null);
    handleSelectSession(sessionId);
  };

  return (
    <main className="sarah-history-window" aria-label="Sarah AI chat history">
      <section className="sarah-history-panel flex flex-col h-full bg-background text-foreground supports-[backdrop-filter]:bg-background/80 backdrop-blur-xl border border-border/50 rounded-xl overflow-hidden shadow-2xl">
        <header className="sarah-history-panel__header flex justify-between items-start p-4 border-b border-border/50 shrink-0">
          <div>
            <div className="flex items-center gap-2 mb-1">
              {selectedSession && (
                <Button variant="ghost" size="icon" onClick={() => setSelectedSession(null)} className="h-6 w-6">
                  <ArrowLeft className="h-4 w-4" />
                </Button>
              )}
              <p className="sarah-history-panel__eyebrow text-xs uppercase tracking-widest text-[#d4af37] font-semibold">Sarah AI</p>
            </div>
            <h1 className="sarah-history-panel__title text-lg font-bold">
              {selectedSession ? "Conversation" : "Chat History"}
            </h1>
          </div>
          <div className="sarah-history-panel__actions flex gap-2">
            <Button type="button" variant="outline" size="sm" onClick={handleClose}>
              Close
            </Button>
          </div>
        </header>

        <div className="flex-1 overflow-y-auto p-4 content-visibility-auto">
          {!selectedSession && (
            <div className="mb-4 relative">
              <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
              <input
                type="text"
                placeholder="Search past conversations..."
                className="w-full bg-muted/50 border border-border/50 rounded-md pl-9 pr-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-ring"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
              />
            </div>
          )}

          {searchResults ? (
            <div className="space-y-3">
              <h3 className="text-xs font-semibold text-muted-foreground uppercase tracking-wider mb-2">Search Results</h3>
              {searchResults.length === 0 ? (
                <p className="text-sm text-muted-foreground">No matching messages found.</p>
              ) : (
                searchResults.map((res) => (
                  <article key={res.id} onClick={() => handleSelectSearchResult(res.sessionId)} className="p-3 rounded-lg border border-border/50 bg-card hover:bg-accent/50 cursor-pointer transition-colors group">
                    <div className="flex items-center gap-2 mb-1.5 opacity-70">
                      <MessageSquare className="size-3.5" />
                      <span className="text-xs font-medium capitalize">{res.role}</span>
                      <span className="text-[10px] ml-auto">{new Date(res.createdAt).toLocaleDateString()}</span>
                    </div>
                    <p className="text-sm line-clamp-3 text-card-foreground/90">{res.content}</p>
                  </article>
                ))
              )}
            </div>
          ) : selectedSession ? (
            <div className="space-y-4 pb-4">
              {selectedSession.messages.map((msg) => (
                <div key={msg.id} className={`flex flex-col ${msg.role === 'user' ? 'items-end' : 'items-start'}`}>
                  <div className="flex items-center gap-2 mb-1 px-1 opacity-60">
                    <span className="text-xs font-medium capitalize">{msg.role}</span>
                    <span className="text-[10px]">{new Date(msg.createdAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}</span>
                  </div>
                  <div className={`p-3 rounded-2xl max-w-[85%] text-sm ${msg.role === 'user' ? 'bg-primary text-primary-foreground rounded-tr-sm' : 'bg-muted border border-border/50 rounded-tl-sm text-foreground'}`}>
                    {msg.content}
                  </div>
                </div>
              ))}
            </div>
          ) : sessions.length === 0 ? (
            <div className="h-full flex flex-col items-center justify-center text-muted-foreground opacity-60">
              <History className="h-10 w-10 mb-2" />
              <p>No chat history available.</p>
            </div>
          ) : (
            <div className="space-y-2">
              {sessions.map((session) => (
                <div key={session.id} onClick={() => handleSelectSession(session.id)} className="flex items-center justify-between p-3 rounded-lg border border-border/50 bg-card hover:bg-accent cursor-pointer transition-colors group">
                  <div className="overflow-hidden pr-4">
                    <p className="text-sm font-medium truncate text-card-foreground">
                      {session.title || "New Conversation"}
                    </p>
                    <div className="flex items-center gap-3 mt-1 text-xs text-muted-foreground">
                      <span className="flex items-center gap-1"><MessageSquare className="size-3 h-[10px] w-[10px]" /> {session.messageCount} msgs</span>
                      <span className="flex items-center gap-1"><Clock3 className="size-3 h-[10px] w-[10px]" /> {session.lastMessageAt ? new Date(session.lastMessageAt).toLocaleDateString() : new Date(session.createdAt).toLocaleDateString()}</span>
                    </div>
                  </div>
                  <Button variant="ghost" size="icon" onClick={(e) => handleDeleteSession(session.id, e)} className="opacity-0 group-hover:opacity-100 transition-opacity text-destructive hover:text-destructive hover:bg-destructive/10 shrink-0">
                    <Trash2 className="h-4 w-4" />
                  </Button>
                </div>
              ))}
            </div>
          )}
        </div>
      </section>
    </main>
  );
}
