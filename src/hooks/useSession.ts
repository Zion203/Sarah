import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export const SESSION_STORAGE_KEY = "sarah_current_session_v1";

export interface Session {
    id: string;
    userId: string;
    title: string | null;
    modelId: string | null;
    systemPrompt: string | null;
    contextWindowUsed: number | null;
    tokenCount: number;
    messageCount: number;
    status: string;
    summary: string | null;
    tags: string;
    pinned: number;
    forkedFromSessionId: string | null;
    forkedAtMessageId: string | null;
    metadata: string;
    lastMessageAt: string | null;
    createdAt: string;
    updatedAt: string;
}

export function useSession() {
    const [currentSessionId, setCurrentSessionId] = useState<string | null>(null);
    const [isReady, setIsReady] = useState(false);
    const [defaultUserId, setDefaultUserId] = useState<string>("default");

    // Initialize: Resume from localStorage or fetch last session
    useEffect(() => {
        let mounted = true;

        async function initSession() {
            try {
                const user = await invoke<{ id: string }>("get_default_user");
                if (mounted) setDefaultUserId(user.id);

                const storedId = window.localStorage.getItem(SESSION_STORAGE_KEY);
                if (storedId) {
                    if (mounted) setCurrentSessionId(storedId);
                } else {
                    // If no stored session, find the most recent one or create a new one
                    const sessions = await invoke<Session[]>("list_sessions", {
                        userId: user.id,
                        limit: 1,
                        cursor: null,
                    });

                    if (sessions.length > 0 && mounted) {
                        const latestId = sessions[0].id;
                        setCurrentSessionId(latestId);
                        window.localStorage.setItem(SESSION_STORAGE_KEY, latestId);
                    } else if (mounted) {
                        const newSession = await invoke<Session>("create_session", {
                            userId: user.id,
                            modelId: null,
                        });
                        setCurrentSessionId(newSession.id);
                        window.localStorage.setItem(SESSION_STORAGE_KEY, newSession.id);
                    }
                }
            } catch (error) {
                console.warn("Failed to initialize session from backend:", error);
            } finally {
                if (mounted) setIsReady(true);
            }
        }

        void initSession();

        return () => {
            mounted = false;
        };
    }, []);

    // Update localStorage when session changes across the app
    useEffect(() => {
        if (!currentSessionId) return;
        window.localStorage.setItem(SESSION_STORAGE_KEY, currentSessionId);
    }, [currentSessionId]);

    const createNewSession = useCallback(async () => {
        try {
            const newSession = await invoke<Session>("create_session", {
                userId: defaultUserId,
                modelId: null,
            });
            setCurrentSessionId(newSession.id);
            return newSession.id;
        } catch (error) {
            console.error("Failed to create new session:", error);
            return null;
        }
    }, [defaultUserId]);

    const switchSession = useCallback((sessionId: string) => {
        setCurrentSessionId(sessionId);
    }, []);

    return {
        currentSessionId,
        isReady,
        createNewSession,
        switchSession,
    };
}
