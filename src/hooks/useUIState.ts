import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export type UIVisualState = "idle" | "listening" | "thinking" | "speaking";
export type ConversationStatus = "thinking" | "completed";
export const CHAT_HISTORY_STORAGE_KEY = "sarah_chat_history_v1";

export interface ConversationItem {
  id: string;
  prompt: string;
  status: ConversationStatus;
  response: string;
}

export interface ChatHistoryItem {
  id: string;
  prompt: string;
  response: string;
  timestamp: string;
}

const STATE_FLOW: UIVisualState[] = ["idle", "listening", "thinking", "speaking"];
const HISTORY_LIMIT = 120;
const OLLAMA_MODEL = "llama3.1:8b";

export function readChatHistory(): ChatHistoryItem[] {
  if (typeof window === "undefined") {
    return [];
  }

  try {
    const raw = window.localStorage.getItem(CHAT_HISTORY_STORAGE_KEY);
    if (!raw) {
      return [];
    }

    const parsed = JSON.parse(raw) as ChatHistoryItem[];
    if (!Array.isArray(parsed)) {
      return [];
    }

    return parsed.filter(
      (item) =>
        typeof item?.id === "string" &&
        typeof item?.prompt === "string" &&
        typeof item?.response === "string" &&
        typeof item?.timestamp === "string",
    );
  } catch {
    return [];
  }
}

export function writeChatHistory(items: ChatHistoryItem[]) {
  if (typeof window === "undefined") {
    return;
  }

  window.localStorage.setItem(CHAT_HISTORY_STORAGE_KEY, JSON.stringify(items.slice(-HISTORY_LIMIT)));
}

export function clearChatHistory() {
  if (typeof window === "undefined") {
    return;
  }

  window.localStorage.removeItem(CHAT_HISTORY_STORAGE_KEY);
}

function amplitudeTargetByState(state: UIVisualState) {
  switch (state) {
    case "idle":
      return 0.08 + Math.random() * 0.06;
    case "listening":
      return 0.32 + Math.random() * 0.3;
    case "thinking":
      return 0.18 + Math.random() * 0.14;
    case "speaking":
      return 0.3 + Math.random() * 0.34;
    default:
      return 0.1;
  }
}

export function useUIState() {
  const [state, setState] = useState<UIVisualState>("idle");
  const [prompt, setPrompt] = useState("");
  const [amplitude, setAmplitude] = useState(0.09);
  const [conversations, setConversations] = useState<ConversationItem[]>([]);
  const [isPromptLocked, setIsPromptLocked] = useState(false);
  const completionTimerRef = useRef<number | null>(null);
  const activeRequestIdRef = useRef(0);

  const clearPending = useCallback(() => {
    if (completionTimerRef.current !== null) {
      window.clearTimeout(completionTimerRef.current);
      completionTimerRef.current = null;
    }
    activeRequestIdRef.current += 1;
  }, []);

  useEffect(() => {
    const timer = window.setInterval(() => {
      const target = amplitudeTargetByState(state);
      setAmplitude((current) => current + (target - current) * 0.4);
    }, 70);

    return () => window.clearInterval(timer);
  }, [state]);

  useEffect(() => clearPending, [clearPending]);

  const cycleState = useCallback(() => {
    setState((current) => {
      const index = STATE_FLOW.indexOf(current);
      const nextIndex = index === STATE_FLOW.length - 1 ? 0 : index + 1;
      return STATE_FLOW[nextIndex];
    });
  }, []);

  const clearPrompt = useCallback(() => {
    clearPending();
    setIsPromptLocked(false);
    setPrompt("");
  }, [clearPending]);

  const submitPrompt = useCallback(() => {
    if (isPromptLocked) {
      return;
    }

    const value = prompt.trim();
    if (!value) {
      return;
    }

    const conversationId = `${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;
    const nextItem: ConversationItem = {
      id: conversationId,
      prompt: value,
      status: "thinking",
      response: "",
    };

    clearPending();
    const requestId = activeRequestIdRef.current;

    console.log(`[Sarah AI] ${value}`);
    setPrompt(value);
    setIsPromptLocked(true);
    setAmplitude(0.6);
    setState("thinking");
    setConversations([nextItem]);

    void (async () => {
      try {
        const response = await invoke<string>("generate_ollama_response", {
          prompt: value,
          model: OLLAMA_MODEL,
        });

        if (activeRequestIdRef.current !== requestId) {
          return;
        }

        const safeResponse = response.trim() || "No response from model.";
        setAmplitude(0.74);
        setState("speaking");

        setConversations((current) =>
          current.map((item) =>
            item.id === conversationId
              ? {
                  ...item,
                  status: "completed",
                  response: safeResponse,
                }
              : item,
          ),
        );

        const existing = readChatHistory();
        writeChatHistory([
          ...existing,
          {
            id: conversationId,
            prompt: value,
            response: safeResponse,
            timestamp: new Date().toISOString(),
          },
        ]);

        completionTimerRef.current = window.setTimeout(() => {
          if (activeRequestIdRef.current !== requestId) {
            return;
          }
          setIsPromptLocked(false);
          setState("idle");
          completionTimerRef.current = null;
        }, 320);
      } catch (error) {
        if (activeRequestIdRef.current !== requestId) {
          return;
        }

        const message =
          error instanceof Error
            ? error.message
            : typeof error === "object" &&
                error !== null &&
                "message" in error &&
                typeof (error as { message: unknown }).message === "string"
              ? (error as { message: string }).message
            : typeof error === "string"
              ? error
              : "Failed to get response from Ollama.";

        setConversations((current) =>
          current.map((item) =>
            item.id === conversationId
              ? {
                  ...item,
                  status: "completed",
                  response: message,
                }
              : item,
          ),
        );
        setIsPromptLocked(false);
        setState("idle");
      }
    })();
  }, [clearPending, isPromptLocked, prompt]);

  const stopResponse = useCallback(() => {
    clearPending();
    setIsPromptLocked(false);
    setState("idle");
    setConversations((current) =>
      current.map((item) =>
        item.status === "thinking"
          ? { ...item, status: "completed", response: "Response stopped." }
          : item,
      ),
    );
  }, [clearPending]);

  return {
    amplitude,
    clearPrompt,
    conversations,
    cycleState,
    isPromptLocked,
    prompt,
    setPrompt,
    setState,
    state,
    stopResponse,
    submitPrompt,
  };
}
