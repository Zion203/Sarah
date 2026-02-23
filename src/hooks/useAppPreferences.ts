import { useCallback, useEffect, useState } from "react";

export type ScreenCaptureSurface = "screen" | "window";

export interface AppPreferences {
  allowScreenRecording: boolean;
  captureOutputDirectory: null | string;
  screenCaptureSurface: ScreenCaptureSurface;
  screenPermissions: Record<ScreenCaptureSurface, boolean>;
  screenPermissionGrantedAt: Record<ScreenCaptureSurface, null | string>;
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
      return next;
    });
  }, []);

  return {
    preferences,
    updatePreferences,
  };
}
