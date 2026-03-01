import { invoke } from "@tauri-apps/api/core";

import { useEffect, useState, useCallback } from "react";
import { Button } from "@/components/ui/button";
import { Loader2, CheckCircle2, AlertCircle, RefreshCw, Sparkles, BrainCircuit, ShieldCheck, Zap } from "lucide-react";

export interface SetupState {
    id: string;
    userId: string | null;
    status: string;
    currentStage: string;
    progressPct: number;
    selectedBundle: string | null;
    hardwareProfileId: string | null;
    lastError: string | null;
    metadata: string;
}

interface SetupWindowProps {
    initialState?: SetupState | null;
    onComplete: () => void;
}

const STAGE_LABELS: Record<string, string> = {
    "stage_a_preflight": "Scanning hardware capabilities...",
    "stage_b_starter_model_install": "Preparing knowledge graph...",
    "stage_b_core_vectors": "Initializing Core Vectors...",
    "stage_b_neural_routing": "Preparing Neural Routing...",
    "stage_b_model_download": "Downloading AI Models...",
    "stage_c_runtime_profile": "Optimizing runtime parameters...",
    "stage_d_background_upgrade_queued": "Setup complete!",
    "stage_d_skipped": "Setup complete!",
};

const TIPS = [
    { icon: ShieldCheck, text: "Sarah runs completely locally. None of your queries are ever sent to the cloud." },
    { icon: Sparkles, text: "Switch between different AI models on the fly in the Models tab." },
    { icon: BrainCircuit, text: "Sarah uses intelligent memory to remember your previous conversations seamlessly." },
    { icon: Zap, text: "You can open Sarah from anywhere by pressing the global shortcut (Alt+Space)." },
];

export default function SetupWindow({ initialState, onComplete }: SetupWindowProps) {
    const [setupState, setSetupState] = useState<SetupState | null>(initialState || null);
    const [isStarting, setIsStarting] = useState(false);
    const [isRetrying, setIsRetrying] = useState(false);
    const [tipIndex, setTipIndex] = useState(0);

    const fetchState = useCallback(async () => {
        try {
            const state = await invoke<SetupState>("get_setup_status");
            setSetupState(state);
            if (state.status === "completed") {
                onComplete();
            }
        } catch (e) {
            console.error("Failed to fetch setup state", e);
        }
    }, [onComplete]);

    useEffect(() => {
        fetchState();

        // Poll setup state every second while in progress
        const interval = setInterval(() => {
            if (setupState?.status === "in_progress") {
                fetchState();
            }
        }, 1000);

        return () => clearInterval(interval);
    }, [setupState?.status, fetchState]);

    useEffect(() => {
        if (setupState?.status !== "in_progress") return;

        // Rotate tips every 5 seconds while downloading
        const tipInterval = setInterval(() => {
            setTipIndex((i) => (i + 1) % TIPS.length);
        }, 5000);

        return () => clearInterval(tipInterval);
    }, [setupState?.status]);

    const handleStart = async () => {
        setIsStarting(true);
        try {
            const nextState = await invoke<SetupState>("start_first_run_setup");
            setSetupState(nextState);
        } catch (e) {
            console.error("Failed to start setup", e);
        } finally {
            setIsStarting(false);
        }
    };

    const handleRetry = async () => {
        setIsRetrying(true);
        try {
            const nextState = await invoke<SetupState>("retry_setup_stage");
            setSetupState(nextState);
        } catch (e) {
            console.error("Failed to retry setup", e);
        } finally {
            setIsRetrying(false);
        }
    };

    const currentStageLabel = setupState?.currentStage
        ? (STAGE_LABELS[setupState.currentStage] || `Setting up (${setupState.currentStage.replace(/_/g, " ")})...`)
        : "Preparing initialization...";

    const CurrentTipIcon = TIPS[tipIndex].icon;

    return (
        <div
            className="flex h-screen w-screen flex-col items-center justify-center bg-background text-foreground relative overflow-hidden"
            data-tauri-drag-region
        >
            {/* Background subtle effect */}
            <div className="absolute inset-0 z-0 bg-gradient-to-br from-primary/5 via-background to-background pointer-events-none" />

            <div className="w-full max-w-md p-8 rounded-2xl border border-border/60 bg-card/80 backdrop-blur-xl shadow-2xl flex flex-col items-center text-center relative z-10">

                <div className="mb-8 w-16 h-16 rounded-full bg-primary/10 flex items-center justify-center text-primary border border-primary/20 shadow-inner">
                    <svg
                        xmlns="http://www.w3.org/2000/svg"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        strokeWidth="2"
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        className="w-8 h-8"
                    >
                        <path d="M12 2v20" />
                        <path d="M17 5H9.5a3.5 3.5 0 0 0 0 7h5a3.5 3.5 0 0 1 0 7H6" />
                    </svg>
                </div>

                <h1 className="text-2xl font-bold mb-2 tracking-tight">Sarah AI Setup</h1>

                {!setupState || setupState.status === "not_started" ? (
                    <>
                        <p className="text-muted-foreground mb-8 text-sm">
                            Welcome to your local AI environment. Sarah needs to prepare your device and securely download necessary neural models before we begin.
                        </p>
                        <Button
                            size="lg"
                            className="w-full font-medium"
                            onClick={handleStart}
                            disabled={isStarting}
                        >
                            {isStarting ? <Loader2 className="w-4 h-4 mr-2 animate-spin" /> : null}
                            {isStarting ? "Starting..." : "Begin Setup"}
                        </Button>
                    </>
                ) : setupState.status === "in_progress" ? (
                    <div className="w-full flex flex-col items-center">
                        <div className="flex items-center gap-2 mb-4">
                            <Loader2 className="w-4 h-4 text-primary animate-spin" />
                            <p className="text-sm font-medium text-foreground">
                                {currentStageLabel}
                            </p>
                        </div>

                        <div className="w-full bg-muted/60 rounded-full h-2.5 min-h-2.5 mb-2 overflow-hidden border border-border/50 shadow-inner">
                            <div
                                className="bg-primary h-full transition-all duration-700 ease-out"
                                style={{ width: `${Math.max(2, setupState.progressPct)}%` }}
                            />
                        </div>
                        <div className="flex w-full justify-between items-center px-1 mb-8">
                            <p className="text-xs text-muted-foreground">
                                Please do not close the app
                            </p>
                            <p className="text-xs font-semibold text-primary">
                                {Math.round(setupState.progressPct)}%
                            </p>
                        </div>

                        {/* Beautiful Tips Carousel */}
                        <div className="w-full h-20 bg-muted/30 border border-border/40 rounded-xl p-4 flex items-center shadow-sm relative overflow-hidden group">
                            <div className="absolute inset-0 bg-gradient-to-r from-transparent via-primary/5 to-transparent -translate-x-full animate-[shimmer_2s_infinite] pointer-events-none" />
                            <div className="flex gap-4 items-center w-full transition-opacity duration-500 key={tipIndex}">
                                <div className="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center shrink-0">
                                    <CurrentTipIcon className="w-5 h-5 text-primary" />
                                </div>
                                <p className="text-xs text-muted-foreground text-left font-medium leading-relaxed">
                                    {TIPS[tipIndex].text}
                                </p>
                            </div>
                        </div>

                    </div>
                ) : setupState.status === "failed" ? (
                    <div className="w-full flex flex-col items-center">
                        <div className="bg-destructive/10 text-destructive p-4 rounded-xl border border-destructive/20 w-full mb-6 text-sm text-left flex gap-3">
                            <AlertCircle className="w-5 h-5 shrink-0 mt-0.5" />
                            <div className="flex-1">
                                <p className="font-medium mb-1">Setup Failed</p>
                                <p className="opacity-90 leading-snug">{setupState.lastError || "An unknown error occurred during setup."}</p>
                            </div>
                        </div>
                        <Button
                            variant="outline"
                            size="lg"
                            className="w-full"
                            onClick={handleRetry}
                            disabled={isRetrying}
                        >
                            {isRetrying ? <Loader2 className="w-4 h-4 mr-2 animate-spin" /> : <RefreshCw className="w-4 h-4 mr-2" />}
                            {isRetrying ? "Retrying..." : "Retry Setup"}
                        </Button>
                    </div>
                ) : setupState.status === "completed" ? (
                    <div className="w-full flex flex-col items-center animate-in fade-in zoom-in duration-500">
                        <div className="p-4 bg-primary/10 rounded-full mb-4">
                            <CheckCircle2 className="w-12 h-12 text-primary" />
                        </div>
                        <p className="text-foreground font-medium mb-8">Setup is complete! Launching Sarah...</p>
                    </div>
                ) : null}

            </div>
        </div>
    );
}
