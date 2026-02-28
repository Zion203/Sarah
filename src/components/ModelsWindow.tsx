import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Download, Minus, Pin, RefreshCw, X, AlertCircle, CheckCircle2 } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { Button } from "@/components/ui/button";
import { MAX_QUICK_SWITCH_MODELS, useQuickSwitchModels } from "@/hooks/useQuickSwitchModels";
import { OLLAMA_MODEL_STORAGE_KEY } from "@/hooks/useUIState";

interface ModelsWindowProps {
  embedded?: boolean;
  onRequestClose?: () => void;
}

interface BackendModel {
  id: string;
  name: string;
  displayName: string;
  family: string;
  parameterCount: string | null;
  quantization: string | null;
  fileSizeMb: number | null;
  minRamMb: number;
  recommendedRamMb: number;
  performanceTier: string;
  energyTier: string;
  isDownloaded: number;
  isActive: number;
  isDefault: number;
  isRecommended: number;
}

interface DownloadProgress {
  modelId: string;
  status: "queued" | "downloading" | "completed" | "failed" | "not_started" | "already_downloaded";
  progressPct: number;
  bytesDownloaded: number;
  bytesTotal: number | null;
  errorMessage: string | null;
}

interface CompatibilityInfo {
  modelId: string;
  compatibilityScore: number;
  reason: string;
}

function readSelectedModel() {
  if (typeof window === "undefined") {
    return "";
  }
  return window.localStorage.getItem(OLLAMA_MODEL_STORAGE_KEY)?.trim() ?? "";
}

function toErrorMessage(error: unknown, fallback: string) {
  if (error instanceof Error && error.message.trim()) return error.message;
  if (typeof error === "object" && error !== null && "message" in error && typeof (error as any).message === "string" && (error as any).message.trim()) {
    return (error as any).message;
  }
  if (typeof error === "string" && error.trim()) return error;
  return fallback;
}

export default function ModelsWindow({ embedded = false, onRequestClose }: ModelsWindowProps) {
  const { quickSwitchModels, setQuickSwitchModels } = useQuickSwitchModels();
  const [catalogModels, setCatalogModels] = useState<BackendModel[]>([]);
  const [installedModels, setInstalledModels] = useState<BackendModel[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [errorStatus, setErrorStatus] = useState<null | string>(null);
  const [selectedModel, setSelectedModel] = useState(readSelectedModel);
  const [statusMessage, setStatusMessage] = useState<null | string>(null);
  const [downloadStates, setDownloadStates] = useState<Record<string, DownloadProgress>>({});
  const [compatScores, setCompatScores] = useState<Record<string, CompatibilityInfo>>({});

  const installedSet = useMemo(() => new Set(installedModels.map((m) => m.name)), [installedModels]);
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

  const loadData = useCallback(async () => {
    setIsLoading(true);
    setErrorStatus(null);
    try {
      const dbCatalog = await invoke<BackendModel[]>("get_model_catalog");
      const dbInstalled = await invoke<BackendModel[]>("get_installed_models");

      setCatalogModels(dbCatalog);
      setInstalledModels(dbInstalled);

      // const dbRecommended = await invoke<any[]>("get_recommended_models");
      // Could merge recommendation info here

    } catch (error) {
      setErrorStatus(toErrorMessage(error, "Failed to load models."));
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadData();
  }, [loadData]);

  const loadCompatScore = async (modelId: string) => {
    if (compatScores[modelId]) return;
    try {
      const score = await invoke<CompatibilityInfo>("get_model_compatibility_score", { modelId });
      setCompatScores(prev => ({ ...prev, [modelId]: score }));
    } catch {
      // ignore
    }
  };

  useEffect(() => {
    catalogModels.forEach(m => loadCompatScore(m.id));
  }, [catalogModels]);

  // Polling for downloads
  useEffect(() => {
    const activeDownloads = Object.values(downloadStates).filter(
      (d) => d.status === "queued" || d.status === "downloading"
    );
    if (activeDownloads.length === 0) return;

    const interval = setInterval(async () => {
      for (const d of activeDownloads) {
        try {
          const progress = await invoke<DownloadProgress>("get_download_progress", { modelId: d.modelId });
          setDownloadStates((prev) => ({ ...prev, [d.modelId]: progress }));
          if (progress.status === "completed" || progress.status === "failed") {
            if (progress.status === "completed") {
              loadData(); // refresh installed list
            }
          }
        } catch (e) {
          console.error("Polled progress failed", e);
        }
      }
    }, 1500);

    return () => clearInterval(interval);
  }, [downloadStates, loadData]);

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

  const handleSetActiveModel = async (modelName: string, modelId: string) => {
    const normalized = modelName.trim();
    if (!normalized) return;

    try {
      await invoke("set_default_model", { modelId });
      window.localStorage.setItem(OLLAMA_MODEL_STORAGE_KEY, normalized);
      setSelectedModel(normalized);
      setStatusMessage(`Active model switched to ${normalized}.`);
      loadData();
    } catch (e) {
      setStatusMessage(toErrorMessage(e, "Failed to set default model."));
    }
  };

  const handleToggleQuickSwitch = (model: string) => {
    const normalized = model.trim();
    if (!normalized) return;

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
    setStatusMessage(quickSwitchSet.has(normalized) ? `${normalized} removed from quick switch.` : `${normalized} added to quick switch.`);
  };

  const handleDownloadModel = async (modelId: string) => {
    setStatusMessage(`Starting download...`);
    try {
      await invoke("start_model_download", { modelId });
      // trigger poll
      const progress = await invoke<DownloadProgress>("get_download_progress", { modelId });
      setDownloadStates(prev => ({ ...prev, [modelId]: progress }));
      setStatusMessage(`Started downloading.`);
    } catch (error) {
      setStatusMessage(toErrorMessage(error, `Failed to download model.`));
    }
  };

  const renderCompatBadge = (modelId: string) => {
    const compat = compatScores[modelId];
    if (!compat) return null;
    let color = "text-yellow-600 bg-yellow-500/10 border-yellow-500/20";
    let icon = <AlertCircle className="size-3 mr-1" />;
    let text = "Fair";
    if (compat.compatibilityScore >= 0.8) {
      color = "text-green-600 bg-green-500/10 border-green-500/20";
      icon = <CheckCircle2 className="size-3 mr-1" />;
      text = "Good";
    } else if (compat.compatibilityScore <= 0.4) {
      color = "text-red-600 bg-red-500/10 border-red-500/20";
      text = "Poor";
    }

    return (
      <span className={`inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-medium border ${color}`} title={compat.reason}>
        {icon} {text} hardware match
      </span>
    );
  };

  return (
    <main className="sarah-models-window" aria-label="Sarah AI models window">
      <section className="sarah-models-shell">
        <header className="sarah-models-titlebar">
          <div className="sarah-models-titlebar__meta">Models</div>
          <p className="sarah-models-titlebar__title">Sarah AI Models</p>
          <div className="sarah-models-titlebar__window-controls" data-tauri-disable-drag-region="true">
            <button type="button" className="sarah-models-titlebar__window-btn" style={{ display: embedded ? "none" : undefined }} onClick={() => void handleMinimize()}>
              <Minus className="size-3.5" />
            </button>
            <button type="button" className="sarah-models-titlebar__window-btn sarah-models-titlebar__window-btn--close" onClick={() => void handleClose()}>
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
                    <button key={model} type="button" className="sarah-models-quick-slot" data-filled="true" onClick={() => handleToggleQuickSwitch(model)}>
                      <span>{model}</span>
                    </button>
                  );
                })}
              </div>
            </article>

            <article className="sarah-models-card sarah-models-card--library flex-1 min-h-0 flex flex-col">
              <header className="sarah-models-card__header shrink-0">
                <div>
                  <p className="sarah-models-card__eyebrow">Discover</p>
                  <h2 className="sarah-models-card__title">Model library</h2>
                </div>
              </header>
              <div className="sarah-models-library flex-1 overflow-y-auto min-h-0">
                {catalogModels.map((model) => {
                  const isInstalled = installedSet.has(model.name);
                  const dState = downloadStates[model.id];
                  const isDownloading = dState && (dState.status === "queued" || dState.status === "downloading");

                  return (
                    <article key={model.id} className="sarah-models-library-item">
                      <div className="sarah-models-library-item__copy">
                        <div className="flex items-center gap-2">
                          <p className="sarah-models-library-item__name">{model.displayName}</p>
                          {renderCompatBadge(model.id)}
                        </div>
                        <p className="sarah-models-library-item__description mt-1 text-xs opacity-70">
                          {model.family} • {model.parameterCount} • {model.quantization} • {model.performanceTier}
                        </p>
                        {isDownloading && (
                          <div className="mt-2 w-full bg-secondary/30 rounded-full h-1.5 overflow-hidden">
                            <div className="bg-primary h-full transition-all duration-300" style={{ width: `${dState.progressPct}%` }} />
                          </div>
                        )}
                      </div>
                      <div className="sarah-models-library-item__actions">
                        {isInstalled ? <span className="sarah-models-chip">Installed</span> : null}
                        <Button
                          type="button"
                          size="sm"
                          variant="outline"
                          disabled={isDownloading || isInstalled}
                          onClick={() => void handleDownloadModel(model.id)}
                        >
                          <Download className="size-3.5" />
                          {isDownloading ? `${Math.round(dState.progressPct)}%` : isInstalled ? "Ready" : "Download"}
                        </Button>
                      </div>
                    </article>
                  );
                })}
              </div>
            </article>
          </section>

          <section className="sarah-models-column">
            <article className="sarah-models-card sarah-models-card--grow flex-1 min-h-0 flex flex-col">
              <header className="sarah-models-card__header shrink-0">
                <div>
                  <p className="sarah-models-card__eyebrow">Installed</p>
                  <h2 className="sarah-models-card__title">Local models</h2>
                </div>
                <Button type="button" size="sm" variant="outline" onClick={() => void loadData()}>
                  <RefreshCw className="size-3.5" />
                  Refresh
                </Button>
              </header>

              {isLoading ? (
                <p className="sarah-models-state">Loading local models...</p>
              ) : errorStatus ? (
                <p className="sarah-models-state">{errorStatus}</p>
              ) : installedModels.length === 0 ? (
                <p className="sarah-models-state">
                  No local models found. Download one from the model library.
                </p>
              ) : (
                <div className="sarah-models-installed-list flex-1 overflow-y-auto min-h-0">
                  {installedModels.map((model) => {
                    const isPinned = quickSwitchSet.has(model.name);
                    const isActive = selectedModel === model.name || model.isDefault === 1;
                    const cannotPinMore = !isPinned && quickSwitchModels.length >= MAX_QUICK_SWITCH_MODELS;

                    return (
                      <article key={model.id} className="sarah-models-installed-item">
                        <div className="sarah-models-installed-item__copy">
                          <p className="sarah-models-installed-item__name">{model.displayName}</p>
                          <p className="sarah-models-installed-item__meta">
                            {model.family} • {model.parameterCount} • {model.quantization}
                          </p>
                          <p className="sarah-models-installed-item__meta">
                            {model.fileSizeMb ? `${(model.fileSizeMb / 1024).toFixed(2)} GB` : "Unknown Size"}
                          </p>
                        </div>
                        <div className="sarah-models-installed-item__actions">
                          <Button
                            type="button"
                            size="sm"
                            variant={isActive ? "secondary" : "outline"}
                            onClick={() => handleSetActiveModel(model.name, model.id)}
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
