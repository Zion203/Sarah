import { useCallback, useEffect, useState } from "react";

export const QUICK_SWITCH_MODELS_STORAGE_KEY = "sarah_quick_switch_models_v1";
export const MAX_QUICK_SWITCH_MODELS = 5;

export function normalizeQuickSwitchModels(value: unknown, limit = MAX_QUICK_SWITCH_MODELS) {
  if (!Array.isArray(value)) {
    return [];
  }

  const unique = new Set<string>();
  for (const item of value) {
    if (typeof item !== "string") {
      continue;
    }

    const normalized = item.trim();
    if (!normalized || unique.has(normalized)) {
      continue;
    }

    unique.add(normalized);
    if (unique.size >= limit) {
      break;
    }
  }

  return Array.from(unique);
}

export function readQuickSwitchModels() {
  if (typeof window === "undefined") {
    return [];
  }

  const raw = window.localStorage.getItem(QUICK_SWITCH_MODELS_STORAGE_KEY);
  if (!raw) {
    return [];
  }

  try {
    return normalizeQuickSwitchModels(JSON.parse(raw));
  } catch {
    return [];
  }
}

export function writeQuickSwitchModels(models: string[]) {
  if (typeof window === "undefined") {
    return;
  }

  window.localStorage.setItem(
    QUICK_SWITCH_MODELS_STORAGE_KEY,
    JSON.stringify(normalizeQuickSwitchModels(models)),
  );
}

type QuickSwitchUpdater = string[] | ((current: string[]) => string[]);

export function useQuickSwitchModels() {
  const [quickSwitchModels, setQuickSwitchModelsState] = useState<string[]>(readQuickSwitchModels);

  useEffect(() => {
    const onStorage = (event: StorageEvent) => {
      if (event.key !== QUICK_SWITCH_MODELS_STORAGE_KEY) {
        return;
      }

      setQuickSwitchModelsState(readQuickSwitchModels());
    };

    window.addEventListener("storage", onStorage);
    return () => window.removeEventListener("storage", onStorage);
  }, []);

  const setQuickSwitchModels = useCallback((updater: QuickSwitchUpdater) => {
    setQuickSwitchModelsState((current) => {
      const next =
        typeof updater === "function"
          ? (updater as (state: string[]) => string[])(current)
          : updater;
      const normalized = normalizeQuickSwitchModels(next);
      writeQuickSwitchModels(normalized);
      return normalized;
    });
  }, []);

  return {
    quickSwitchModels,
    setQuickSwitchModels,
  };
}
