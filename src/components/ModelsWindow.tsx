import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Download, Minus, Pin, RefreshCw, X } from "lucide-react";
import {
  useCallback,
  useEffect,
  useMemo,
  useState,
} from "react";
import { Button } from "@/components/ui/button";
import {
  MAX_QUICK_SWITCH_MODELS,
  useQuickSwitchModels,
} from "@/hooks/useQuickSwitchModels";
import { OLLAMA_MODEL_STORAGE_KEY } from "@/hooks/useUIState";

interface ModelsWindowProps {
  embedded?: boolean;
  onRequestClose?: () => void;
}

interface DetailedOllamaModel {
  digestShort: string;
  family: string;
  modifiedAt: null | string;
  name: string;
  parameterSize: string;
  quantizationLevel: string;
  sizeBytes: number;
  sizeLabel: string;
}

interface ModelLibraryEntry {
  description: string;
  name: string;
}

const MODEL_LIBRARY: ModelLibraryEntry[] = [
  { name: "llama3.1:8b", description: "Balanced, fast local assistant model." },
  { name: "llama3.1:70b", description: "Higher quality reasoning with heavier RAM/GPU usage." },
  { name: "qwen2.5:7b", description: "Strong multilingual and coding capability." },
  { name: "qwen2.5-coder:7b", description: "Code-focused model for development tasks." },
  { name: "mistral:7b", description: "General-purpose lightweight model." },
  { name: "phi4:latest", description: "Compact model with good instruction following." },
  { name: "deepseek-r1:8b", description: "Reasoning-focused distilled model." },
  { name: "gemma2:9b", description: "Reliable instruction model with strong baseline quality." },
];

function normalizeDetailedModel(value: unknown): DetailedOllamaModel | null {
  if (typeof value !== "object" || value === null) {
    return null;
  }

  const row = value as Record<string, unknown>;
  const name = typeof row.name === "string" ? row.name.trim() : "";
  if (!name) {
    return null;
  }

  return {
    digestShort: typeof row.digestShort === "string" ? row.digestShort : "",
    family: typeof row.family === "string" ? row.family : "Unknown",
    modifiedAt: typeof row.modifiedAt === "string" ? row.modifiedAt : null,
    name,
    parameterSize: typeof row.parameterSize === "string" ? row.parameterSize : "Unknown",
    quantizationLevel:
      typeof row.quantizationLevel === "string" ? row.quantizationLevel : "Unknown",
    sizeBytes: typeof row.sizeBytes === "number" ? row.sizeBytes : 0,
    sizeLabel: typeof row.sizeLabel === "string" ? row.sizeLabel : "Unknown size",
  };
}

function normalizeDetailedModels(value: unknown) {
  if (!Array.isArray(value)) {
    return [];
  }

  const unique = new Map<string, DetailedOllamaModel>();
  for (const item of value) {
    const normalized = normalizeDetailedModel(item);
    if (!normalized) {
      continue;
    }
    unique.set(normalized.name, normalized);
  }

  return Array.from(unique.values()).sort((left, right) =>
    left.name.toLowerCase().localeCompare(right.name.toLowerCase()),
  );
}

function readSelectedModel() {
  if (typeof window === "undefined") {
    return "";
  }

  return window.localStorage.getItem(OLLAMA_MODEL_STORAGE_KEY)?.trim() ?? "";
}

function toErrorMessage(error: unknown, fallback: string) {
  if (error instanceof Error && error.message.trim()) {
    return error.message;
  }

  if (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof (error as { message: unknown }).message === "string" &&
    (error as { message: string }).message.trim()
  ) {
    return (error as { message: string }).message;
  }

  if (typeof error === "string" && error.trim()) {
    return error;
  }

  return fallback;
}

function formatModifiedAt(value: null | string) {
  if (!value) {
    return "Unknown";
  }

  const parsed = new Date(value);
  if (Number.isNaN(parsed.valueOf())) {
    return value;
  }

  return parsed.toLocaleString();
}

function ModelsWindow({ embedded = false, onRequestClose }: ModelsWindowProps) {
  const { quickSwitchModels, setQuickSwitchModels } = useQuickSwitchModels();
  const [installedModels, setInstalledModels] = useState<DetailedOllamaModel[]>([]);
  const [isLoadingInstalled, setIsLoadingInstalled] = useState(false);
  const [installedError, setInstalledError] = useState<null | string>(null);
  const [selectedModel, setSelectedModel] = useState(readSelectedModel);
  const [statusMessage, setStatusMessage] = useState<null | string>(null);
  const [downloadStateByModel, setDownloadStateByModel] = useState<
    Record<string, "done" | "error" | "loading">
  >({});

  const installedSet = useMemo(
    () => new Set(installedModels.map((model) => model.name)),
    [installedModels],
  );
  const quickSwitchSet = useMemo(() => new Set(quickSwitchModels), [quickSwitchModels]);

  useEffect(() => {
    const onStorage = (event: StorageEvent) => {
      if (event.key === OLLAMA_MODEL_STORAGE_KEY) {
        setSelectedModel(readSelectedModel());
      }
    };

    window.addEventListener("storage", onStorage);
    return () => window.removeEventListener("storage", onStorage);
  }, []);

  const loadInstalledModels = useCallback(async () => {
    setIsLoadingInstalled(true);
    setInstalledError(null);
    try {
      const response = await invoke<unknown>("list_ollama_models_detailed");
      const normalized = normalizeDetailedModels(response);
      setInstalledModels(normalized);
    } catch (error) {
      setInstalledError(toErrorMessage(error, "Failed to load local models from Ollama."));
      setInstalledModels([]);
    } finally {
      setIsLoadingInstalled(false);
    }
  }, []);

  useEffect(() => {
    void loadInstalledModels();
  }, [loadInstalledModels]);

  const handleClose = async () => {
    if (embedded) {
      onRequestClose?.();
      return;
    }

    await getCurrentWindow().close();
  };

  const handleMinimize = async () => {
    await getCurrentWindow().minimize();
  };

  const handleSetActiveModel = (model: string) => {
    const normalized = model.trim();
    if (!normalized) {
      return;
    }

    window.localStorage.setItem(OLLAMA_MODEL_STORAGE_KEY, normalized);
    setSelectedModel(normalized);
    setStatusMessage(`Active model switched to ${normalized}.`);
  };

  const handleToggleQuickSwitch = (model: string) => {
    const normalized = model.trim();
    if (!normalized) {
      return;
    }

    if (!quickSwitchSet.has(normalized) && quickSwitchModels.length >= MAX_QUICK_SWITCH_MODELS) {
      setStatusMessage(`Quick switch supports up to ${MAX_QUICK_SWITCH_MODELS} models.`);
      return;
    }

    setQuickSwitchModels((current) => {
      if (current.includes(normalized)) {
        return current.filter((item) => item !== normalized);
      }
      return [normalized, ...current].slice(0, MAX_QUICK_SWITCH_MODELS);
    });
    setStatusMessage(
      quickSwitchSet.has(normalized)
        ? `${normalized} removed from quick switch.`
        : `${normalized} added to quick switch.`,
    );
  };

  const handleDownloadModel = async (model: string) => {
    const normalized = model.trim();
    if (!normalized) {
      return;
    }

    setDownloadStateByModel((current) => ({ ...current, [normalized]: "loading" }));
    setStatusMessage(`Downloading ${normalized}...`);

    try {
      const status = await invoke<string>("pull_ollama_model", { model: normalized });
      setDownloadStateByModel((current) => ({ ...current, [normalized]: "done" }));
      setStatusMessage(`${normalized}: ${status}`);
      await loadInstalledModels();
    } catch (error) {
      setDownloadStateByModel((current) => ({ ...current, [normalized]: "error" }));
      setStatusMessage(toErrorMessage(error, `Failed to download ${normalized}.`));
    }
  };

  return (
    <main className="sarah-models-window" aria-label="Sarah AI models window">
      <section className="sarah-models-shell">
        <header className="sarah-models-titlebar">
          <div className="sarah-models-titlebar__meta">Models</div>
          <p className="sarah-models-titlebar__title">Sarah AI Models</p>
          <div className="sarah-models-titlebar__window-controls" data-tauri-disable-drag-region="true">
            <button
              type="button"
              className="sarah-models-titlebar__window-btn"
              aria-label="Minimize models window"
              style={{ display: embedded ? "none" : undefined }}
              onClick={() => void handleMinimize()}
            >
              <Minus className="size-3.5" />
            </button>
            <button
              type="button"
              className="sarah-models-titlebar__window-btn sarah-models-titlebar__window-btn--close"
              aria-label="Close models window"
              onClick={() => void handleClose()}
            >
              <X className="size-3.5" />
            </button>
          </div>
        </header>

        <div className="sarah-models-layout">
          <section className="sarah-models-column">
            <article className="sarah-models-card sarah-models-card--quick">
              <header className="sarah-models-card__header">
                <div>
                  <p className="sarah-models-card__eyebrow">Quick switch</p>
                  <h2 className="sarah-models-card__title">
                    Models ({quickSwitchModels.length}/{MAX_QUICK_SWITCH_MODELS})
                  </h2>
                </div>
              </header>
              <p className="sarah-models-card__note">
                These models appear when you click the bot icon in the main input.
              </p>
              <div className="sarah-models-quick-grid">
                {Array.from({ length: MAX_QUICK_SWITCH_MODELS }).map((_, index) => {
                  const model = quickSwitchModels[index] ?? "";
                  if (!model) {
                    return (
                      <div key={`slot-${index + 1}`} className="sarah-models-quick-slot" data-filled="false">
                        Slot {index + 1}
                      </div>
                    );
                  }

                  return (
                    <button
                      key={model}
                      type="button"
                      className="sarah-models-quick-slot"
                      data-filled="true"
                      onClick={() => handleToggleQuickSwitch(model)}
                    >
                      <span>{model}</span>
                    </button>
                  );
                })}
              </div>
            </article>

            <article className="sarah-models-card sarah-models-card--library">
              <header className="sarah-models-card__header">
                <div>
                  <p className="sarah-models-card__eyebrow">Discover</p>
                  <h2 className="sarah-models-card__title">Model library</h2>
                </div>
              </header>
              <div className="sarah-models-library">
                {MODEL_LIBRARY.map((model) => {
                  const isInstalled = installedSet.has(model.name);
                  const downloadState = downloadStateByModel[model.name];

                  return (
                    <article key={model.name} className="sarah-models-library-item">
                      <div className="sarah-models-library-item__copy">
                        <p className="sarah-models-library-item__name">{model.name}</p>
                        <p className="sarah-models-library-item__description">{model.description}</p>
                      </div>
                      <div className="sarah-models-library-item__actions">
                        {isInstalled ? <span className="sarah-models-chip">Installed</span> : null}
                        <Button
                          type="button"
                          size="sm"
                          variant="outline"
                          disabled={downloadState === "loading"}
                          onClick={() => void handleDownloadModel(model.name)}
                        >
                          <Download className="size-3.5" />
                          {downloadState === "loading"
                            ? "Downloading..."
                            : downloadState === "done"
                              ? "Downloaded"
                              : downloadState === "error"
                                ? "Retry"
                                : "Download"}
                        </Button>
                      </div>
                    </article>
                  );
                })}
              </div>
            </article>
          </section>

          <section className="sarah-models-column">
            <article className="sarah-models-card sarah-models-card--grow">
              <header className="sarah-models-card__header">
                <div>
                  <p className="sarah-models-card__eyebrow">Installed</p>
                  <h2 className="sarah-models-card__title">Local models</h2>
                </div>
                <Button type="button" size="sm" variant="outline" onClick={() => void loadInstalledModels()}>
                  <RefreshCw className="size-3.5" />
                  Refresh
                </Button>
              </header>

              {isLoadingInstalled ? (
                <p className="sarah-models-state">Loading local models...</p>
              ) : installedError ? (
                <p className="sarah-models-state">{installedError}</p>
              ) : installedModels.length === 0 ? (
                <p className="sarah-models-state">
                  No local models found. Download one from the model library.
                </p>
              ) : (
                <div className="sarah-models-installed-list">
                  {installedModels.map((model) => {
                    const isPinned = quickSwitchSet.has(model.name);
                    const isActive = selectedModel === model.name;
                    const cannotPinMore = !isPinned && quickSwitchModels.length >= MAX_QUICK_SWITCH_MODELS;

                    return (
                      <article key={model.name} className="sarah-models-installed-item">
                        <div className="sarah-models-installed-item__copy">
                          <p className="sarah-models-installed-item__name">{model.name}</p>
                          <p className="sarah-models-installed-item__meta">
                            {model.family} • {model.parameterSize} • {model.quantizationLevel}
                          </p>
                          <p className="sarah-models-installed-item__meta">
                            {model.sizeLabel} • Updated {formatModifiedAt(model.modifiedAt)}
                            {model.digestShort ? ` • ${model.digestShort}` : ""}
                          </p>
                        </div>
                        <div className="sarah-models-installed-item__actions">
                          <Button
                            type="button"
                            size="sm"
                            variant={isActive ? "secondary" : "outline"}
                            onClick={() => handleSetActiveModel(model.name)}
                          >
                            {isActive ? "Active" : "Use model"}
                          </Button>
                          <Button
                            type="button"
                            size="sm"
                            variant={isPinned ? "secondary" : "ghost"}
                            disabled={cannotPinMore}
                            onClick={() => handleToggleQuickSwitch(model.name)}
                          >
                            <Pin className="size-3.5" />
                            {isPinned ? "Pinned" : "Pin quick switch"}
                          </Button>
                        </div>
                      </article>
                    );
                  })}
                </div>
              )}
            </article>
          </section>
        </div>

        <footer className="sarah-models-footer">
          <span>{statusMessage ?? "Manage downloads and quick-switch models here."}</span>
        </footer>
      </section>
    </main>
  );
}

export default ModelsWindow;
