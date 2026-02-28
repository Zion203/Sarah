import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { join } from "@tauri-apps/api/path";
import { listen } from "@tauri-apps/api/event";
import { useSession } from "./useSession";

export type UIVisualState = "idle" | "listening" | "thinking" | "speaking";
export type ConversationStatus = "thinking" | "completed";
export type ModelSelectionMode = "auto" | "manual";
export const CHAT_HISTORY_STORAGE_KEY = "sarah_chat_history_v1";
export const OLLAMA_MODEL_STORAGE_KEY = "sarah_ollama_model_v1";
export const MODEL_SELECTION_MODE_STORAGE_KEY = "sarah_model_selection_mode_v1";

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
const DEFAULT_OLLAMA_MODEL = "llama3.1:8b";
const AUDIO_DECISION_MODEL = "qwen2.5-coder:7b";
const MCP_STORAGE_KEY = "sarah_spotify_mcp_config_v1";
const SPOTIFY_WEB_DEVICE_STORAGE_KEY = "sarah_spotify_web_sdk_device_id_v1";
const MCP_LEGACY_ROOT =
  "C:\\Users\\jesud\\OneDrive\\Desktop\\personal\\Sarah\\mcp\\spotify-mcp-server";
const MCP_DEFAULT_ROOT = "F:\\Sarah\\mcp\\spotify-mcp-server";

function readStoredModel() {
  if (typeof window === "undefined") {
    return DEFAULT_OLLAMA_MODEL;
  }

  const value = window.localStorage.getItem(OLLAMA_MODEL_STORAGE_KEY);
  const normalized = value?.trim();
  if (!normalized) {
    return DEFAULT_OLLAMA_MODEL;
  }

  return normalized;
}

function readStoredModelSelectionMode(): ModelSelectionMode {
  if (typeof window === "undefined") {
    return "auto";
  }

  const value = window.localStorage.getItem(MODEL_SELECTION_MODE_STORAGE_KEY);
  return value === "manual" ? "manual" : "auto";
}

const AUDIO_COMMAND_PATTERNS: Array<{
  intent: "play" | "pause" | "stop" | "next" | "prev";
  match: RegExp;
}> = [
    { intent: "play", match: /\b(play|resume|start|unpause)\b/i },
    { intent: "pause", match: /\b(pause|hold|freeze)\b/i },
    { intent: "stop", match: /\b(stop|silence|mute)\b/i },
    { intent: "next", match: /\b(next|skip|forward)\b/i },
    { intent: "prev", match: /\b(previous|prev|back|rewind)\b/i },
  ];

type AudioIntent =
  | { action: "play"; explicit: boolean; query?: string; type?: "track" | "album" | "artist" | "playlist" }
  | { action: "queue"; explicit: boolean; query: string }
  | { action: "volume"; explicit: boolean; value?: number; adjustment?: number }
  | { action: "pause" | "stop" | "next" | "prev"; explicit: boolean };

type AudioDecision =
  | { action: "play"; type?: "track" | "album" | "artist" | "playlist"; query?: string }
  | { action: "pause" | "stop" | "next" | "prev" }
  | { action: "queue"; query: string }
  | { action: "volume_set"; value: number }
  | { action: "volume_adjust"; adjustment: number }
  | { action: "none" };

type AudioPlayType = "track" | "album" | "artist" | "playlist";

function parseAudioIntent(input: string): AudioIntent | null {
  const text = input.trim();
  if (!text) return null;
  const hasAudioKeyword = /\b(spotify|song|track|audio|music|player)\b/i.test(text);

  const playlistPhraseMatch = text.match(
    /\b(play|start|resume|put on|turn on)\b[\s\S]*?\bplaylist\b\s*(.+)?$/i,
  );
  if (playlistPhraseMatch) {
    const query = (playlistPhraseMatch[2] ?? "").trim() || text.replace(/.*playlist/i, "").trim();
    if (query.length > 0) {
      return { action: "play", query, type: "playlist", explicit: true };
    }
  }

  const albumPhraseMatch = text.match(
    /\b(play|start|resume|put on|turn on)\b[\s\S]*?\balbum\b\s*(.+)?$/i,
  );
  if (albumPhraseMatch) {
    const query = (albumPhraseMatch[2] ?? "").trim() || text.replace(/.*album/i, "").trim();
    if (query.length > 0) {
      return { action: "play", query, type: "album", explicit: true };
    }
  }

  const artistPhraseMatch = text.match(
    /\b(play|start|resume|put on|turn on)\b[\s\S]*?\bartist\b\s*(.+)?$/i,
  );
  if (artistPhraseMatch) {
    const query = (artistPhraseMatch[2] ?? "").trim() || text.replace(/.*artist/i, "").trim();
    if (query.length > 0) {
      return { action: "play", query, type: "artist", explicit: true };
    }
  }

  const playTypedMatch = text.match(
    /^(?:play|start|resume)\s+(playlist|album|artist|track)\s+(.+)$/i,
  );
  if (playTypedMatch) {
    const type = playTypedMatch[1].toLowerCase() as "track" | "album" | "artist" | "playlist";
    const query = playTypedMatch[2].trim();
    if (query.length > 0) {
      return { action: "play", query, type, explicit: true };
    }
  }

  const playQueryMatch = text.match(
    /^(?:play|start|resume)\s+(?:spotify|song|track|music)?\s*(.+)$/i,
  );
  if (playQueryMatch && playQueryMatch[1]) {
    const query = playQueryMatch[1].trim();
    if (query.length > 0 && !/\b(next|previous|prev|back)\b/i.test(query)) {
      return { action: "play", query, type: "track", explicit: true };
    }
  }

  const queueMatch = text.match(/^(?:queue|add)\s+(.+?)\s*(?:to\s+queue)?$/i);
  if (queueMatch && queueMatch[1]) {
    const query = queueMatch[1].trim();
    if (query.length > 0) {
      return { action: "queue", query, explicit: hasAudioKeyword };
    }
  }

  const implicitPlaylistMatch = text.match(/\bplaylist\b/i);
  if (implicitPlaylistMatch && hasAudioKeyword) {
    const query = text.replace(/.*playlist/i, "").trim();
    if (query.length > 0) {
      return { action: "play", query, type: "playlist", explicit: true };
    }
  }

  const volumeSetMatch = text.match(/(?:set\s+volume\s+to|volume)\s+(\d{1,3})/i);
  if (volumeSetMatch) {
    const value = Math.min(100, Math.max(0, Number(volumeSetMatch[1])));
    return { action: "volume", value, explicit: hasAudioKeyword };
  }

  const volumeAdjustMatch = text.match(/\b(volume|sound)\s+(up|down)\b/i);
  if (volumeAdjustMatch) {
    const adjustment = volumeAdjustMatch[2].toLowerCase() === "up" ? 10 : -10;
    return { action: "volume", adjustment, explicit: hasAudioKeyword };
  }

  for (const pattern of AUDIO_COMMAND_PATTERNS) {
    if (pattern.match.test(text)) {
      return { action: pattern.intent, explicit: hasAudioKeyword };
    }
  }
  return null;
}

type SpotifyToolResponse = {
  content?: Array<{ type?: string; text?: string; isError?: boolean }>;
};

type SpotifyToolResult = {
  isError: boolean;
  text: string;
};

function parseSearchResult(text: string) {
  const idMatch = text.match(/ID:\s*([A-Za-z0-9]+)/);
  const titleMatch = text.match(/1\.\s+"(.+?)"\s+by\s+(.+?)\s+\(/);
  return {
    id: idMatch?.[1],
    title: titleMatch?.[1],
    artist: titleMatch?.[2],
  };
}

function safeParseJson<T>(raw: string): T | null {
  try {
    return JSON.parse(raw) as T;
  } catch {
    return null;
  }
}

function parseSpotifyToolResult(raw: string): SpotifyToolResult {
  const parsed = safeParseJson<SpotifyToolResponse>(raw);
  const blocks = Array.isArray(parsed?.content) ? parsed.content : [];
  const text = blocks
    .map((block) => (typeof block.text === "string" ? block.text.trim() : ""))
    .filter(Boolean)
    .join("\n\n")
    .trim();

  const inferredError =
    /^error\b/i.test(text) ||
    /\b(failed|unauthorized|forbidden|not found)\b/i.test(text);

  return {
    isError: blocks.some((block) => block.isError === true) || inferredError,
    text: text || raw.trim(),
  };
}

function clearPreferredSpotifyDeviceId() {
  if (typeof window === "undefined") {
    return;
  }
  window.localStorage.removeItem(SPOTIFY_WEB_DEVICE_STORAGE_KEY);
}

async function invokeSpotifyToolWithDeviceRecovery(
  serverRoot: string,
  tool: string,
  args: Record<string, unknown>,
) {
  const invokeTool = async (nextArgs: Record<string, unknown>) =>
    invoke<string>("run_spotify_tool", { serverRoot, tool, args: nextArgs });

  let raw = await invokeTool(args);
  let parsed = parseSpotifyToolResult(raw);
  const deviceId = typeof args.deviceId === "string" ? args.deviceId.trim() : "";
  const shouldRetryWithoutDevice =
    Boolean(deviceId) && parsed.isError && /\bdevice not found\b/i.test(parsed.text);

  if (shouldRetryWithoutDevice) {
    clearPreferredSpotifyDeviceId();
    const retryArgs: Record<string, unknown> = { ...args };
    delete retryArgs.deviceId;
    raw = await invokeTool(retryArgs);
    parsed = parseSpotifyToolResult(raw);
  }

  if (parsed.isError) {
    throw new Error(parsed.text || "Spotify MCP command failed.");
  }

  return raw;
}

function extractJsonBlock(raw: string) {
  const start = raw.indexOf("{");
  const end = raw.lastIndexOf("}");
  if (start === -1 || end === -1 || end <= start) return null;
  return raw.slice(start, end + 1);
}

function looksLikeAudioCommand(text: string) {
  return /\b(spotify|song|track|audio|music|player|playlist|album|artist|play|pause|stop|next|previous|skip|queue|volume)\b/i.test(
    text,
  );
}

function resolvePlaySearchType(prompt: string, modelType?: AudioPlayType): AudioPlayType {
  const normalizedPrompt = prompt.toLowerCase();
  if (/\bplaylist\b/.test(normalizedPrompt)) {
    return "playlist";
  }
  if (/\balbum\b/.test(normalizedPrompt)) {
    return "album";
  }
  if (/\bartist\b/.test(normalizedPrompt)) {
    return "artist";
  }
  // Default to track/song-name lookup unless user explicitly requests another type.
  return modelType === "track" ? "track" : "track";
}

function normalizeMcpServerRoot(value: unknown) {
  if (typeof value !== "string") {
    return MCP_DEFAULT_ROOT;
  }

  const normalized = value.trim();
  if (!normalized || normalized === MCP_LEGACY_ROOT) {
    return MCP_DEFAULT_ROOT;
  }

  return normalized;
}

function withPreferredSpotifyDevice<T extends Record<string, unknown>>(args: T): T & { deviceId?: string } {
  if (typeof window === "undefined") {
    return args;
  }

  const raw = window.localStorage.getItem(SPOTIFY_WEB_DEVICE_STORAGE_KEY);
  const deviceId = raw?.trim();
  if (!deviceId) {
    return args;
  }

  return {
    ...args,
    deviceId,
  };
}

async function requestAudioDecision(prompt: string): Promise<AudioDecision | null> {
  const system = [
    "You are an assistant that outputs ONLY valid JSON.",
    "Choose an audio action if the user asks to control Spotify music.",
    "Schema:",
    '{ "action": "play|pause|stop|next|prev|queue|volume_set|volume_adjust|none", "type": "track|album|artist|playlist", "query": "string", "value": number, "adjustment": number }',
    "Rules:",
    "- If playing a playlist/album/artist/song name, set action=play and include query.",
    "- If queuing a song, set action=queue with query.",
    "- If user says volume 40 -> action=volume_set, value=40.",
    "- If volume up/down -> action=volume_adjust, adjustment=10 or -10.",
    "- If no audio intent, action=none.",
    "Return JSON only.",
  ].join("\n");

  const response = await invoke<string>("generate_ollama_response", {
    prompt: `${system}\n\nUser: ${prompt}`,
    model: AUDIO_DECISION_MODEL,
  });

  const jsonBlock = extractJsonBlock(response.trim());
  if (!jsonBlock) return null;
  return safeParseJson<AudioDecision>(jsonBlock);
}

async function requestAudioDecisionWithTimeout(prompt: string, timeoutMs: number) {
  const timeout = new Promise<null>((resolve) => {
    const timer = window.setTimeout(() => {
      window.clearTimeout(timer);
      resolve(null);
    }, timeoutMs);
  });

  return Promise.race([requestAudioDecision(prompt), timeout]);
}

async function ensureSpotifyMcpRunning(serverRoot: string) {
  const isRunning = await invoke<boolean>("spotify_mcp_status");
  if (isRunning) {
    return { ok: true as const };
  }

  const entryPath = await join(serverRoot, "build", "index.js");
  await invoke("start_spotify_mcp", { entryPath });
  return { ok: true as const };
}

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

interface UseUIStateOptions {
  animate?: boolean;
}

export function useUIState(options: UseUIStateOptions = {}) {
  const { animate = true } = options;
  const [state, setState] = useState<UIVisualState>("idle");
  const [prompt, setPrompt] = useState("");
  const [amplitude, setAmplitude] = useState(0.09);
  const [conversations, setConversations] = useState<ConversationItem[]>([]);
  const [selectedModel, setSelectedModelState] = useState(readStoredModel);
  const [modelSelectionMode, setModelSelectionModeState] =
    useState<ModelSelectionMode>(readStoredModelSelectionMode);
  const [isPromptLocked, setIsPromptLocked] = useState(false);
  const completionTimerRef = useRef<number | null>(null);
  const activeRequestIdRef = useRef(0);
  const { currentSessionId, createNewSession } = useSession();

  const clearPending = useCallback(() => {
    if (completionTimerRef.current !== null) {
      window.clearTimeout(completionTimerRef.current);
      completionTimerRef.current = null;
    }
    activeRequestIdRef.current += 1;
  }, []);

  useEffect(() => {
    if (!animate) {
      return;
    }

    const timer = window.setInterval(() => {
      const target = amplitudeTargetByState(state);
      setAmplitude((current) => current + (target - current) * 0.4);
    }, 70);

    return () => window.clearInterval(timer);
  }, [animate, state]);

  useEffect(() => clearPending, [clearPending]);

  useEffect(() => {
    const onStorage = (event: StorageEvent) => {
      if (event.key === OLLAMA_MODEL_STORAGE_KEY) {
        setSelectedModelState(readStoredModel());
      } else if (event.key === MODEL_SELECTION_MODE_STORAGE_KEY) {
        setModelSelectionModeState(readStoredModelSelectionMode());
      }
    };

    window.addEventListener("storage", onStorage);
    return () => window.removeEventListener("storage", onStorage);
  }, []);

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

  const clearConversation = useCallback(async () => {
    clearPending();
    setIsPromptLocked(false);
    setPrompt("");
    setState("idle");
    setConversations([]);
    await createNewSession();
  }, [clearPending, createNewSession]);

  const setSelectedModel = useCallback((model: string) => {
    const normalized = model.trim();
    if (!normalized) {
      return;
    }

    setSelectedModelState(normalized);
    if (typeof window !== "undefined") {
      window.localStorage.setItem(OLLAMA_MODEL_STORAGE_KEY, normalized);
    }
  }, []);

  const setModelSelectionMode = useCallback((mode: ModelSelectionMode) => {
    setModelSelectionModeState(mode);
    if (typeof window !== "undefined") {
      window.localStorage.setItem(MODEL_SELECTION_MODE_STORAGE_KEY, mode);
    }
  }, []);

  const submitPrompt = useCallback(() => {
    if (isPromptLocked) {
      return;
    }

    const value = prompt.trim();
    if (!value) {
      return;
    }

    if (looksLikeAudioCommand(value)) {
      const conversationId = `${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;
      clearPending();
      setPrompt(value);
      setIsPromptLocked(false);
      setState("idle");
      setConversations([
        {
          id: conversationId,
          prompt: value,
          status: "thinking",
          response: "Connecting Spotify MCP...",
        },
      ]);

      void (async () => {
        try {
          const decision =
            (await requestAudioDecisionWithTimeout(value, 4500)) ??
            (() => {
              const fallback = parseAudioIntent(value);
              if (!fallback) return null;
              if (fallback.action === "volume") {
                return fallback.value !== undefined
                  ? { action: "volume_set", value: fallback.value }
                  : fallback.adjustment !== undefined
                    ? { action: "volume_adjust", adjustment: fallback.adjustment }
                    : { action: "none" };
              }
              if (fallback.action === "queue") {
                return { action: "queue", query: fallback.query };
              }
              if (fallback.action === "play") {
                return { action: "play", query: fallback.query, type: fallback.type };
              }
              return { action: fallback.action };
            })();

          if (!decision || decision.action === "none") {
            setConversations([
              {
                id: conversationId,
                prompt: value,
                status: "completed",
                response:
                  "Tell me which song, playlist, or artist to play (or say play/pause/next).",
              },
            ]);
            return;
          }

          const stored = window.localStorage.getItem(MCP_STORAGE_KEY);
          const parsedServerRoot = stored
            ? (() => {
              try {
                const parsed = JSON.parse(stored) as { serverRoot?: unknown };
                return parsed.serverRoot;
              } catch {
                return undefined;
              }
            })()
            : undefined;
          const serverRoot = normalizeMcpServerRoot(parsedServerRoot);

          await ensureSpotifyMcpRunning(serverRoot);

          if (decision.action === "play") {
            if (!decision.query) {
              setConversations([
                {
                  id: conversationId,
                  prompt: value,
                  status: "thinking",
                  response: "Resuming playback...",
                },
              ]);
              await invokeSpotifyToolWithDeviceRecovery(
                serverRoot,
                "resumePlayback",
                withPreferredSpotifyDevice({}),
              );
              setConversations([
                {
                  id: conversationId,
                  prompt: value,
                  status: "completed",
                  response: "Resuming Spotify playback.",
                },
              ]);
              return;
            }

            const searchType = resolvePlaySearchType(value, decision.type);
            setConversations([
              {
                id: conversationId,
                prompt: value,
                status: "thinking",
                response: "Searching Spotify...",
              },
            ]);
            const searchRaw = await invoke<string>("run_spotify_tool", {
              serverRoot,
              tool: "searchSpotify",
              args: { query: decision.query, type: searchType, limit: 5 },
            });
            const searchResult = safeParseJson<SpotifyToolResponse>(searchRaw);
            if (!searchResult) {
              setConversations([
                {
                  id: conversationId,
                  prompt: value,
                  status: "completed",
                  response: searchRaw,
                },
              ]);
              return;
            }

            const searchText = searchResult.content?.[0]?.text ?? "";
            const { id, title, artist } = parseSearchResult(searchText);

            if (!id) {
              setConversations([
                {
                  id: conversationId,
                  prompt: value,
                  status: "completed",
                  response: `No Spotify results found for \"${decision.query}\".`,
                },
              ]);
              return;
            }

            await invokeSpotifyToolWithDeviceRecovery(
              serverRoot,
              "playMusic",
              withPreferredSpotifyDevice({ type: searchType, id }),
            );

            setConversations([
              {
                id: conversationId,
                prompt: value,
                status: "completed",
                response: title
                  ? `Playing ${title}${artist ? `, ${artist}` : ""}`
                  : "Playing",
              },
            ]);
            return;
          }

          if (decision.action === "queue") {
            const searchRaw = await invoke<string>("run_spotify_tool", {
              serverRoot,
              tool: "searchSpotify",
              args: { query: decision.query, type: "track", limit: 5 },
            });
            const searchResult = safeParseJson<SpotifyToolResponse>(searchRaw);
            if (!searchResult) {
              setConversations([
                {
                  id: conversationId,
                  prompt: value,
                  status: "completed",
                  response: searchRaw,
                },
              ]);
              return;
            }
            const searchText = searchResult.content?.[0]?.text ?? "";
            const { id, title, artist } = parseSearchResult(searchText);
            if (!id) {
              setConversations([
                {
                  id: conversationId,
                  prompt: value,
                  status: "completed",
                  response: `No Spotify results found for \"${decision.query}\".`,
                },
              ]);
              return;
            }
            await invokeSpotifyToolWithDeviceRecovery(
              serverRoot,
              "addToQueue",
              withPreferredSpotifyDevice({ type: "track", id }),
            );
            setConversations([
              {
                id: conversationId,
                prompt: value,
                status: "completed",
                response: title
                  ? `Queued \"${title}\"${artist ? ` by ${artist}` : ""}.`
                  : "Added track to queue.",
              },
            ]);
            return;
          }

          if (decision.action === "volume_set") {
            await invokeSpotifyToolWithDeviceRecovery(
              serverRoot,
              "setVolume",
              withPreferredSpotifyDevice({ volumePercent: decision.value }),
            );
            setConversations([
              {
                id: conversationId,
                prompt: value,
                status: "completed",
                response: `Volume set to ${decision.value}%.`,
              },
            ]);
            return;
          }

          if (decision.action === "volume_adjust") {
            await invokeSpotifyToolWithDeviceRecovery(
              serverRoot,
              "adjustVolume",
              withPreferredSpotifyDevice({ adjustment: decision.adjustment }),
            );
            setConversations([
              {
                id: conversationId,
                prompt: value,
                status: "completed",
                response:
                  decision.adjustment > 0 ? "Volume increased." : "Volume decreased.",
              },
            ]);
            return;
          }

          const toolMap: Record<string, string> = {
            play: "resumePlayback",
            pause: "pausePlayback",
            stop: "pausePlayback",
            next: "skipToNext",
            prev: "skipToPrevious",
          };
          const tool = toolMap[decision.action];
          if (tool) {
            await invokeSpotifyToolWithDeviceRecovery(
              serverRoot,
              tool,
              withPreferredSpotifyDevice({}),
            );
          }

          const responseMap: Record<string, string> = {
            play: "Resuming Spotify playback.",
            pause: "Pausing Spotify playback.",
            stop: "Stopping Spotify playback.",
            next: "Skipping to the next track on Spotify.",
            prev: "Going back to the previous track on Spotify.",
          };
          setConversations([
            {
              id: conversationId,
              prompt: value,
              status: "completed",
              response: responseMap[decision.action] ?? "Spotify command sent.",
            },
          ]);
        } catch (error) {
          const message =
            error instanceof Error
              ? error.message
              : typeof error === "string"
                ? error
                : "Spotify MCP command failed.";
          setConversations([
            {
              id: conversationId,
              prompt: value,
              status: "completed",
              response: message,
            },
          ]);
        }
      })();

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

    setPrompt(value);
    setIsPromptLocked(true);
    setAmplitude(0.6);
    setState("thinking");
    setConversations([nextItem]);

    void (async () => {
      try {
        if (!currentSessionId) {
          throw new Error("No active session initialized.");
        }

        const unlistenToken = await listen<{
          sessionId: string;
          token: string;
          done: boolean;
        }>("ai:token", (event) => {
          if (event.payload.sessionId !== currentSessionId) return;
          if (activeRequestIdRef.current !== requestId) return;

          setConversations((current) =>
            current.map((item) =>
              item.id === conversationId
                ? {
                  ...item,
                  response: item.response + event.payload.token,
                }
                : item,
            ),
          );
        });

        const unlistenDone = await listen<{ sessionId: string }>("ai:done", (event) => {
          if (event.payload.sessionId !== currentSessionId) return;
          if (activeRequestIdRef.current !== requestId) return;

          setAmplitude(0.74);
          setState("speaking");

          setConversations((current) =>
            current.map((item) =>
              item.id === conversationId
                ? {
                  ...item,
                  status: "completed",
                }
                : item,
            ),
          );

          completionTimerRef.current = window.setTimeout(() => {
            if (activeRequestIdRef.current !== requestId) {
              return;
            }
            setIsPromptLocked(false);
            setState("idle");
            completionTimerRef.current = null;
          }, 320);

          unlistenToken();
          unlistenDone();
        });

        const user = await invoke<{ id: string }>("get_default_user");
        await invoke("send_message", {
          request: {
            userId: user.id,
            sessionId: currentSessionId,
            content: value,
            attachments: [],
            modelSelectionMode,
            selectedModel: modelSelectionMode === "manual" ? selectedModel : null,
          },
        });

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
  }, [clearPending, isPromptLocked, modelSelectionMode, prompt, selectedModel]);

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

  const setSystemConversation = useCallback(
    (promptLabel: string, responseText: string, persist = false) => {
      const safePrompt = promptLabel.trim() || "System";
      const safeResponse = responseText.trim() || "Done.";
      const id = `${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;

      clearPending();
      setIsPromptLocked(false);
      setPrompt("");
      setState("idle");
      setConversations([
        {
          id,
          prompt: safePrompt,
          status: "completed",
          response: safeResponse,
        },
      ]);

      if (persist) {
        const existing = readChatHistory();
        writeChatHistory([
          ...existing,
          {
            id,
            prompt: safePrompt,
            response: safeResponse,
            timestamp: new Date().toISOString(),
          },
        ]);
      }
    },
    [clearPending],
  );

  return {
    amplitude,
    clearConversation,
    clearPrompt,
    conversations,
    cycleState,
    isPromptLocked,
    prompt,
    modelSelectionMode,
    selectedModel,
    setModelSelectionMode,
    setPrompt,
    setSelectedModel,
    setSystemConversation,
    setState,
    state,
    stopResponse,
    submitPrompt,
  };
}
