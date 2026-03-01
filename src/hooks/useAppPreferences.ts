import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export type ScreenCaptureSurface = "screen" | "window";

export interface AppPreferences {
  allowScreenRecording: boolean;
  captureOutputDirectory: null | string;
  screenCaptureSurface: ScreenCaptureSurface;
  screenPermissions: Record<ScreenCaptureSurface, boolean>;
  screenPermissionGrantedAt: Record<ScreenCaptureSurface, null | string>;
}

interface Setting {
  key: string;
  value: string;
  valueType: string;
}

const APP_PREFERENCES_STORAGE_KEY = "sarah_app_preferences_v1";

const DEFAULT_APP_PREFERENCES: AppPreferences = {
  allowScreenRecording: false,
  captureOutputDirectory: null,
  screenCaptureSurface: "window",
  screenPermissions: {
    screen: false,
    window: false,
  },
  screenPermissionGrantedAt: {
    screen: null,
    window: null,
  },
};

function parseSurface(value: unknown): ScreenCaptureSurface {
  return value === "screen" ? "screen" : "window";
}

export function readAppPreferences(): AppPreferences {
  if (typeof window === "undefined") {
    return DEFAULT_APP_PREFERENCES;
  }

  try {
    const raw = window.localStorage.getItem(APP_PREFERENCES_STORAGE_KEY);
    if (!raw) {
      return DEFAULT_APP_PREFERENCES;
    }

    const parsed = JSON.parse(raw) as Partial<AppPreferences>;
    const surface = parseSurface(parsed.screenCaptureSurface);

    return {
      allowScreenRecording:
        typeof parsed.allowScreenRecording === "boolean"
          ? parsed.allowScreenRecording
          : DEFAULT_APP_PREFERENCES.allowScreenRecording,
      captureOutputDirectory:
        typeof parsed.captureOutputDirectory === "string"
          ? parsed.captureOutputDirectory
          : null,
      screenCaptureSurface: surface,
      screenPermissions: {
        window:
          typeof parsed.screenPermissions?.window === "boolean"
            ? parsed.screenPermissions.window
            : DEFAULT_APP_PREFERENCES.screenPermissions.window,
        screen:
          typeof parsed.screenPermissions?.screen === "boolean"
            ? parsed.screenPermissions.screen
            : DEFAULT_APP_PREFERENCES.screenPermissions.screen,
      },
      screenPermissionGrantedAt: {
        window:
          typeof parsed.screenPermissionGrantedAt?.window === "string"
            ? parsed.screenPermissionGrantedAt.window
            : null,
        screen:
          typeof parsed.screenPermissionGrantedAt?.screen === "string"
            ? parsed.screenPermissionGrantedAt.screen
            : null,
      },
    };
  } catch {
    return DEFAULT_APP_PREFERENCES;
  }
}

export function writeAppPreferences(preferences: AppPreferences) {
  if (typeof window === "undefined") {
    return;
  }

  window.localStorage.setItem(APP_PREFERENCES_STORAGE_KEY, JSON.stringify(preferences));
}

type PreferencesUpdater =
  | Partial<AppPreferences>
  | ((current: AppPreferences) => AppPreferences);

export function useAppPreferences() {
  const [preferences, setPreferences] = useState<AppPreferences>(readAppPreferences);

  useEffect(() => {
    // Sync from backend on mount
    invoke<Setting[]>("list_settings_namespace", { namespace: "app_preferences" })
      .then((settings) => {
        if (settings.length > 0) {
          const next = { ...readAppPreferences() };
          for (const s of settings) {
            if (s.key in next) {
              if (s.valueType === "json") {
                try {
                  (next as Record<string, any>)[s.key] = JSON.parse(s.value);
                } catch { }
              } else if (s.valueType === "boolean") {
                (next as Record<string, any>)[s.key] = s.value === "true";
              } else {
                (next as Record<string, any>)[s.key] = s.value;
              }
            }
          }
          writeAppPreferences(next);
          setPreferences(next);
        }
      })
      .catch((e) => console.warn("Failed to sync settings from backend", e));

    const onStorage = (event: StorageEvent) => {
      if (event.key !== APP_PREFERENCES_STORAGE_KEY) {
        return;
      }
      setPreferences(readAppPreferences());
    };

    window.addEventListener("storage", onStorage);
    return () => window.removeEventListener("storage", onStorage);
  }, []);

  const updatePreferences = useCallback((updater: PreferencesUpdater) => {
    setPreferences((current) => {
      const next =
        typeof updater === "function"
          ? (updater as (state: AppPreferences) => AppPreferences)(current)
          : {
            ...current,
            ...updater,
          };

      writeAppPreferences(next);

      // Sync to backend
      for (const [key, value] of Object.entries(next)) {
        const prevValue = (current as Record<string, any>)[key];
        const stringifiedPrev = typeof prevValue === "object" ? JSON.stringify(prevValue) : String(prevValue);
        const stringifiedNext = typeof value === "object" ? JSON.stringify(value) : String(value);

        if (stringifiedPrev !== stringifiedNext) {
          invoke("set_setting", {
            namespace: "app_preferences",
            key,
            value: stringifiedNext,
            valueType: typeof value === "object" ? "json" : typeof value,
            isEncrypted: false,
          }).catch((e) => console.warn(`Failed to sync setting ${key} to backend`, e));
        }
      }

      return next;
    });
  }, []);

  return {
    preferences,
    updatePreferences,
  };
}
