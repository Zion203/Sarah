import { useEffect, useRef } from "react";
import type { ConversationItem } from "@/hooks/useUIState";

interface ConversationFeedProps {
  items: ConversationItem[];
}

function ConversationFeed({ items }: ConversationFeedProps) {
  const scrollRef = useRef<HTMLElement | null>(null);
  const currentItem = items.length > 0 ? items[items.length - 1] : undefined;
  const isThinking = currentItem?.status === "thinking";
  const isEmpty = !currentItem;

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [currentItem]);

  return (
    <section
      ref={scrollRef}
      className={`sarah-chat-thread ${isEmpty ? "sarah-chat-thread--empty" : ""}`}
      aria-label="Current response panel"
    >
      {isEmpty ? (
        <div className="sarah-chat-empty">
          <p className="sarah-chat-empty__badge">
            <span className="sarah-chat-empty__badge-dot" />
            Ready for your prompt
          </p>
          <p className="sarah-chat-empty__title">Response panel is waiting</p>
          <p className="sarah-chat-empty__subtitle">
            Ask anything and Sarah will answer right here.
          </p>
          <div className="sarah-chat-empty__hints">
            <span className="sarah-chat-empty__hint">Try: Summarize this code</span>
            <span className="sarah-chat-empty__hint">Shortcut: Ctrl + Space</span>
          </div>
        </div>
      ) : (
        <>
          <p
            className={`sarah-response-status ${isThinking ? "sarah-response-status--processing" : ""}`}
          >
            {isThinking ? (
              <>
                <span className="sarah-response-status__base">Thinking...</span>
                <span className="sarah-response-status__scan">Thinking...</span>
              </>
            ) : (
              "Response ready"
            )}
          </p>
          {currentItem.status === "thinking" ? (
            <div className="sarah-response-skeleton" aria-label="Response is loading">
              <span className="sarah-response-skeleton__line" />
              <span className="sarah-response-skeleton__line sarah-response-skeleton__line--wide" />
              <span className="sarah-response-skeleton__line sarah-response-skeleton__line--mid" />
            </div>
          ) : (
            <p className="sarah-response-text">{currentItem.response}</p>
          )}
        </>
      )}
    </section>
  );
}

export default ConversationFeed;
