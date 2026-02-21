import { getCurrentWindow } from "@tauri-apps/api/window";
import { Clock3, History, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { clearChatHistory, readChatHistory, type ChatHistoryItem } from "@/hooks/useUIState";

function HistoryWindow() {
  const [history, setHistory] = useState<ChatHistoryItem[]>([]);

  useEffect(() => {
    const sync = () => setHistory(readChatHistory().slice().reverse());
    sync();

    const timer = window.setInterval(sync, 900);
    return () => window.clearInterval(timer);
  }, []);

  const handleClose = () => {
    void getCurrentWindow().close();
  };

  const handleClear = () => {
    clearChatHistory();
    setHistory([]);
  };

  return (
    <main className="sarah-history-window" aria-label="Sarah AI chat history">
      <section className="sarah-history-panel">
        <header className="sarah-history-panel__header">
          <div>
            <p className="sarah-history-panel__eyebrow">Sarah AI</p>
            <h1 className="sarah-history-panel__title">Chat History</h1>
            <p className="sarah-history-panel__subtitle">
              Type <code>/history</code> from the main input to open this window.
            </p>
          </div>
          <div className="sarah-history-panel__actions">
            <Button type="button" variant="outline" size="sm" onClick={handleClear}>
              <Trash2 className="size-3.5" />
              Clear
            </Button>
            <Button type="button" variant="outline" size="sm" onClick={handleClose}>
              Close
            </Button>
          </div>
        </header>

        <div className="sarah-history-list">
          {history.length === 0 ? (
            <p className="sarah-history-empty">No history yet.</p>
          ) : (
            history.map((item) => (
              <article key={item.id} className="sarah-history-item">
                <div className="sarah-history-item__meta">
                  <History className="size-3.5" />
                  <span>User prompt</span>
                </div>
                <p className="sarah-history-item__prompt">{item.prompt}</p>
                <p className="sarah-history-item__response">{item.response}</p>
                <p className="sarah-history-item__time">
                  <Clock3 className="size-3.5" />
                  {new Date(item.timestamp).toLocaleString()}
                </p>
              </article>
            ))
          )}
        </div>
      </section>
    </main>
  );
}

export default HistoryWindow;
