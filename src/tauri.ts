import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { register, unregisterAll } from "@tauri-apps/plugin-global-shortcut";

export type ProtectionStatus = "protected" | "unavailable" | "error";
export type CaptureState = "stopped" | "listening" | "paused";
export type LocalModelStatus = { id: string; name: string; sizeLabel: string; description: string; installed: boolean; path: string; sizeBytes: number };
export type ChatMessage = { role: "user" | "assistant"; content: string };
export type ChatGptStatus = { connected: boolean; email?: string; planUsageEnabled: boolean };
export type ChatGptModel = { slug: string; display_name: string };
export const isTauri = () => "__TAURI_INTERNALS__" in window;

export async function getProtectionStatus(): Promise<ProtectionStatus> {
  if (!isTauri()) return "unavailable";
  return invoke<ProtectionStatus>("screen_protection_status");
}
export const hasApiKey = () => isTauri() ? invoke<boolean>("has_api_key") : Promise.resolve(false);
export const saveApiKey = (value: string) => invoke<void>("save_api_key", { value, test: true });
export const deleteApiKey = () => invoke<void>("delete_api_key");
export const hasOpenRouterKey = () => isTauri() ? invoke<boolean>("has_openrouter_key") : Promise.resolve(false);
export const saveOpenRouterKey = (value: string) => invoke<void>("save_openrouter_key", { value, test: true });
export const deleteOpenRouterKey = () => invoke<void>("delete_openrouter_key");
export const hasGoogleKey = () => isTauri() ? invoke<boolean>("has_google_key") : Promise.resolve(false);
export const saveGoogleKey = (value: string) => invoke<void>("save_google_key", { value, test: true });
export const deleteGoogleKey = () => invoke<void>("delete_google_key");
export const testConnection = () => invoke<void>("test_openai_connection");
export const getChatGptStatus = () => isTauri() ? invoke<ChatGptStatus>("chatgpt_status") : Promise.resolve({ connected: false, planUsageEnabled: false });
export const signInChatGpt = () => invoke<ChatGptStatus>("chatgpt_sign_in");
export const signOutChatGpt = () => invoke<void>("chatgpt_sign_out");
export const getChatGptModels = () => invoke<ChatGptModel[]>("chatgpt_models");
export const getLocalModelStatus = () => isTauri() ? invoke<LocalModelStatus[]>("local_model_status") : Promise.resolve([]);
export const downloadLocalModel = (modelId: string) => invoke<LocalModelStatus[]>("download_local_model", { modelId });
export const startCapture = (source: string, language: string, captionEngine: string, localModel: string) => invoke<void>("start_capture", { source, language, captionEngine, localModel });
export const pauseCapture = () => invoke<void>("pause_capture");
export const resumeCapture = () => invoke<void>("resume_capture");
export const stopCapture = () => invoke<void>("stop_capture");
export const submitPrompt = (messages: ChatMessage[], model: string, openRouterModel: string, googleModel: string, systemPrompt: string, provider: string) => invoke<void>("submit_prompt", { messages, model, openrouterModel: openRouterModel, googleModel, systemPrompt, provider });
export const cancelResponse = () => invoke<void>("cancel_response");

export async function subscribe<T>(event: string, callback: (payload: T) => void): Promise<UnlistenFn> {
  if (!isTauri()) return () => undefined;
  return listen<T>(event, ({ payload }) => callback(payload));
}

let shortcutSetup: Promise<void> | null = null;

async function registerGlobalShortcuts(): Promise<void> {
  if (!isTauri()) return;
  await unregisterAll();
  await register("CommandOrControl+Shift+L", () => window.dispatchEvent(new Event("ia-toggle-capture")));
  await register("CommandOrControl+Enter", () => window.dispatchEvent(new Event("ia-submit")));
  await register("CommandOrControl+Shift+H", async () => {
    const handle = getCurrentWindow();
    if (await handle.isVisible()) await handle.hide();
    else { await handle.show(); await handle.setFocus(); }
  });
  await register("CommandOrControl+Shift+Q", () => void exitApplication());
}

export function setupGlobalShortcuts(): Promise<void> {
  shortcutSetup ??= registerGlobalShortcuts().catch((error) => {
    shortcutSetup = null;
    throw error;
  });
  return shortcutSetup;
}

export async function hideWindow(): Promise<void> { if (isTauri()) await getCurrentWindow().hide(); }
export async function exitApplication(): Promise<void> { if (isTauri()) await invoke("exit_app"); }
