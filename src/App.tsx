import {
  Check,
  ChevronDown,
  CircleStop,
  Copy,
  Download,
  Headphones,
  GripHorizontal,
  Mic,
  Minus,
  Pause,
  Play,
  Send,
  Settings,
  ShieldCheck,
  ShieldOff,
  Trash2,
  X,
} from "lucide-react";
import { isValidElement, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { PhysicalPosition } from "@tauri-apps/api/dpi";
import { getCurrentWindow } from "@tauri-apps/api/window";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import {
  cancelResponse,
  deleteApiKey,
  deleteGoogleKey,
  deleteOpenRouterKey,
  exitApplication,
  downloadLocalModel,
  getLocalModelStatus,
  getChatGptModels,
  getChatGptStatus,
  getProtectionStatus,
  hasApiKey,
  hasGoogleKey,
  hasOpenRouterKey,
  hideWindow,
  isTauri,
  pauseCapture,
  resumeCapture,
  saveApiKey,
  saveGoogleKey,
  saveOpenRouterKey,
  signInChatGpt,
  signOutChatGpt,
  setupGlobalShortcuts,
  startCapture,
  stopCapture,
  submitPrompt,
  subscribe,
  type LocalModelStatus,
  type ProtectionStatus,
  type ChatGptModel,
  type ChatGptStatus,
} from "./tauri";

type CaptureState = "stopped" | "listening" | "paused";
type CaptureSource = "system" | "microphone";
type Language = "auto" | "en" | "vi";
type CaptionEngine = "local" | "openai";
type AnswerProvider = "chatgpt" | "openai" | "openrouter" | "google";
type OpenRouterModel = "google/gemma-4-26b-a4b-it:free" | "openrouter/free";
type GoogleModel = "gemini-3.8-flash" | "gemini-3.1-flash-lite";
type ChatMessage = { id: number; role: "user" | "assistant"; content: string; label?: string };
type DragState = {
  pointerId: number;
  startX: number;
  startY: number;
  latestX: number;
  latestY: number;
  windowX?: number;
  windowY?: number;
  frame?: number;
};

const sourceLabels: Record<CaptureSource, string> = {
  system: "System Audio",
  microphone: "Microphone",
};

const languageLabels: Record<Language, string> = {
  auto: "Auto EN / VI · ưu tiên VI",
  en: "English",
  vi: "Tiếng Việt",
};

function markdownNodeText(node: ReactNode): string {
  if (typeof node === "string" || typeof node === "number") return String(node);
  if (Array.isArray(node)) return node.map(markdownNodeText).join("");
  if (isValidElement<{ children?: ReactNode }>(node)) return markdownNodeText(node.props.children);
  return "";
}

function MarkdownAnswer({ content }: { content: string }) {
  return (
    <ReactMarkdown
      remarkPlugins={[remarkGfm]}
      components={{
        a: ({ node: _node, ...props }) => <a {...props} target="_blank" rel="noreferrer" />,
        pre: ({ node: _node, children, ...props }) => {
          const code = markdownNodeText(children).replace(/\n$/, "");
          return (
            <div className="code-block">
              <div className="code-toolbar"><span>Code</span><button onClick={() => void navigator.clipboard.writeText(code)}>Copy</button></div>
              <pre {...props}>{children}</pre>
            </div>
          );
        },
      }}
    >
      {content}
    </ReactMarkdown>
  );
}

function answerModelLabel(provider: AnswerProvider, openRouterModel: OpenRouterModel, googleModel: GoogleModel, openAiModel: string) {
  if (provider === "chatgpt") return openAiModel || "ChatGPT plan";
  if (provider === "google") return googleModel === "gemini-3.8-flash" ? "Gemini 3.8 Flash" : "Gemini 3.1 Flash-Lite";
  if (provider === "openrouter") return openRouterModel === "openrouter/free" ? "OpenRouter Auto Free" : "Gemma 4 Fast Free";
  return openAiModel || "OpenAI";
}

function App() {
  const dragState = useRef<DragState | null>(null);
  const lastFinalCaption = useRef({ text: "", at: 0 });
  const nextMessageId = useRef(1);
  const answerScrollRef = useRef<HTMLDivElement>(null);
  const [captureState, setCaptureState] = useState<CaptureState>("stopped");
  const [source, setSource] = useState<CaptureSource>("system");
  const [language, setLanguage] = useState<Language>("auto");
  const [transcript, setTranscript] = useState("");
  const [chatHistory, setChatHistory] = useState<ChatMessage[]>([]);
  const [protection, setProtection] =
    useState<ProtectionStatus>("unavailable");
  const [copied, setCopied] = useState<"transcript" | "answer" | null>(null);
  const [partial, setPartial] = useState("");
  const [error, setError] = useState("");
  const [answering, setAnswering] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [apiConfigured, setApiConfigured] = useState(false);
  const [apiKey, setApiKey] = useState("");
  const [openRouterConfigured, setOpenRouterConfigured] = useState(false);
  const [openRouterKey, setOpenRouterKey] = useState("");
  const [openRouterModel, setOpenRouterModel] = useState<OpenRouterModel>(() => (localStorage.getItem("ia-openrouter-model") as OpenRouterModel) || "google/gemma-4-26b-a4b-it:free");
  const [googleConfigured, setGoogleConfigured] = useState(false);
  const [googleKey, setGoogleKey] = useState("");
  const [googleModel, setGoogleModel] = useState<GoogleModel>(() => (localStorage.getItem("ia-google-model") as GoogleModel) || "gemini-3.8-flash");
  const [answerProvider, setAnswerProvider] = useState<AnswerProvider>(() => (localStorage.getItem("ia-answer-provider") as AnswerProvider) || "openrouter");
  const [settingsBusy, setSettingsBusy] = useState(false);
  const [chatGptStatus, setChatGptStatus] = useState<ChatGptStatus>({ connected: false, planUsageEnabled: false });
  const [chatGptModels, setChatGptModels] = useState<ChatGptModel[]>([]);
  const [captionEngine, setCaptionEngine] = useState<CaptionEngine>(() => (localStorage.getItem("ia-caption-engine") as CaptionEngine) || "local");
  const [localModels, setLocalModels] = useState<LocalModelStatus[]>([]);
  const [selectedLocalModel, setSelectedLocalModel] = useState(() => localStorage.getItem("ia-local-model") || "large-v3-turbo-q5");
  const [modelDownload, setModelDownload] = useState<{ modelId: string; percent: number } | null>(null);
  const [model, setModel] = useState(() => localStorage.getItem("ia-model") || "gpt-5.5");
  const [systemPrompt, setSystemPrompt] = useState(() => localStorage.getItem("ia-system-prompt") || "Answer the interview question clearly and concisely. Use the same language as the question.");

  useEffect(() => {
    void getProtectionStatus().then(setProtection).catch((reason) => setError(String(reason)));
    void hasApiKey().then(setApiConfigured);
    void hasOpenRouterKey().then(setOpenRouterConfigured);
    void hasGoogleKey().then(setGoogleConfigured);
    void getChatGptStatus().then((status) => {
      setChatGptStatus(status);
      if (status.connected && status.planUsageEnabled) void getChatGptModels().then((models) => {
        setChatGptModels(models);
        if (models.length && !models.some((item) => item.slug === model)) setModel(models[0].slug);
      }).catch(() => undefined);
    });
    void getLocalModelStatus().then(setLocalModels).catch((reason) => setError(String(reason)));
    void setupGlobalShortcuts().catch((reason) => setError(String(reason)));
    const unlisteners: Array<() => void> = [];
    let disposed = false;
    void Promise.all([
      subscribe<string>("capture-state-changed", (value) => setCaptureState(value as CaptureState)),
      subscribe<string>("transcript-partial", setPartial),
      subscribe<string>("transcript-final", (value) => {
        const normalized = value.trim();
        const now = Date.now();
        if (!normalized || (lastFinalCaption.current.text === normalized && now - lastFinalCaption.current.at < 3000)) return;
        lastFinalCaption.current = { text: normalized, at: now };
        setTranscript((current) => `${current}${current.trim() ? " " : ""}${normalized}`);
        setPartial("");
      }),
      subscribe<string>("transcription-error", (value) => setError(value)),
      subscribe<{ modelId: string; percent: number }>("local-model-download-progress", setModelDownload),
      subscribe<void>("response-started", () => setAnswering(true)),
      subscribe<string>("response-provider-changed", (label) => setChatHistory((current) => {
        const next = [...current];
        for (let cursor = next.length - 1; cursor >= 0; cursor -= 1) {
          if (next[cursor].role === "assistant") {
            next[cursor] = { ...next[cursor], label, content: "" };
            break;
          }
        }
        return next;
      })),
      subscribe<string>("response-delta", (value) => setChatHistory((current) => {
        const next = [...current];
        let index = -1;
        for (let cursor = next.length - 1; cursor >= 0; cursor -= 1) {
          if (next[cursor].role === "assistant") { index = cursor; break; }
        }
        if (index >= 0) next[index] = { ...next[index], content: next[index].content + value };
        return next;
      })),
      subscribe<void>("response-completed", () => setAnswering(false)),
      subscribe<void>("response-cancelled", () => {
        setAnswering(false);
        setChatHistory((current) => current.length && current[current.length - 1].role === "assistant" && !current[current.length - 1].content ? current.slice(0, -1) : current);
      }),
      subscribe<string>("response-error", (value) => {
        setAnswering(false);
        setError(value);
        setChatHistory((current) => current.length && current[current.length - 1].role === "assistant" && !current[current.length - 1].content ? current.slice(0, -1) : current);
      }),
    ]).then((items) => {
      if (disposed) items.forEach((unlisten) => unlisten());
      else unlisteners.push(...items);
    });
    return () => {
      disposed = true;
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, []);

  useEffect(() => {
    const container = answerScrollRef.current;
    if (container) container.scrollTop = container.scrollHeight;
  }, [chatHistory, answering]);

  useEffect(() => {
    const toggle = () => void toggleCapture();
    const send = () => void submit();
    window.addEventListener("ia-toggle-capture", toggle);
    window.addEventListener("ia-submit", send);
    return () => { window.removeEventListener("ia-toggle-capture", toggle); window.removeEventListener("ia-submit", send); };
  });

  const stateLabel = useMemo(() => {
    if (captureState === "listening") return "Listening";
    if (captureState === "paused") return "Paused";
    return "Ready";
  }, [captureState]);

  async function toggleCapture() {
    setError("");
    try {
      if (captureState === "stopped") await startCapture(source, language, captionEngine, selectedLocalModel);
      else if (captureState === "paused") await resumeCapture();
      else await pauseCapture();
    } catch (reason) { setError(String(reason)); }
  }

  async function changeSource(nextSource: CaptureSource) {
    if (captureState !== "stopped") await stopCapture().catch(() => undefined);
    setSource(nextSource);
  }

  async function copyText(value: string, target: "transcript" | "answer") {
    if (!value) return;
    await navigator.clipboard.writeText(value);
    setCopied(target);
    window.setTimeout(() => setCopied(null), 1200);
  }

  async function submit() {
    const question = transcript.trim();
    if (!question || answering) return;
    if (answerProvider === "chatgpt" && !chatGptStatus.connected) {
      setError("ChatGPT chưa kết nối hoàn chỉnh. Mở Settings và chọn Continue with ChatGPT.");
      setSettingsOpen(true);
      return;
    }
    setError("");
    const context = chatHistory
      .filter((message) => message.content.trim())
      .map(({ role, content }) => ({ role, content }))
      .slice(-12);
    const userMessage: ChatMessage = { id: nextMessageId.current++, role: "user", content: question };
    const assistantMessage: ChatMessage = {
      id: nextMessageId.current++,
      role: "assistant",
      content: "",
      label: answerModelLabel(answerProvider, openRouterModel, googleModel, model),
    };
    setChatHistory((current) => [...current, userMessage, assistantMessage]);
    setAnswering(true);
    setTranscript((current) => current.trim() === question ? "" : current);
    try {
      await submitPrompt([...context, { role: "user", content: question }], model, openRouterModel, googleModel, systemPrompt, answerProvider);
    } catch (reason) {
      setAnswering(false);
      setError(String(reason));
      setChatHistory((current) => current.filter((message) => message.id !== assistantMessage.id));
    }
  }

  const latestAnswer = [...chatHistory].reverse().find((message) => message.role === "assistant" && message.content)?.content || "";

  async function saveSettings() {
    setSettingsBusy(true); setError("");
    try {
      if (apiKey.trim()) { await saveApiKey(apiKey); setApiConfigured(true); setApiKey(""); }
      if (openRouterKey.trim()) { await saveOpenRouterKey(openRouterKey); setOpenRouterConfigured(true); setOpenRouterKey(""); }
      if (googleKey.trim()) { await saveGoogleKey(googleKey); setGoogleConfigured(true); setGoogleKey(""); }
      localStorage.setItem("ia-model", model);
      localStorage.setItem("ia-system-prompt", systemPrompt);
      localStorage.setItem("ia-caption-engine", captionEngine);
      localStorage.setItem("ia-local-model", selectedLocalModel);
      localStorage.setItem("ia-answer-provider", answerProvider);
      localStorage.setItem("ia-openrouter-model", openRouterModel);
      localStorage.setItem("ia-google-model", googleModel);
      setSettingsOpen(false);
    } catch (reason) { setError(String(reason)); }
    finally { setSettingsBusy(false); }
  }

  async function installLocalModel(modelId: string) {
    setSettingsBusy(true); setError(""); setModelDownload({ modelId, percent: 0 });
    try { setLocalModels(await downloadLocalModel(modelId)); }
    catch (reason) { setError(String(reason)); }
    finally { setSettingsBusy(false); setModelDownload(null); }
  }

  async function connectChatGpt() {
    setSettingsBusy(true); setError("");
    try {
      const status = await signInChatGpt();
      setChatGptStatus(status);
      const models = await getChatGptModels();
      setChatGptModels(models);
      if (models.length && !models.some((item) => item.slug === model)) setModel(models[0].slug);
    } catch (reason) { setError(String(reason)); }
    finally { setSettingsBusy(false); }
  }

  async function disconnectChatGpt() {
    setSettingsBusy(true); setError("");
    try { await signOutChatGpt(); setChatGptStatus({ connected: false, planUsageEnabled: false }); setChatGptModels([]); }
    catch (reason) { setError(String(reason)); }
    finally { setSettingsBusy(false); }
  }

  async function beginOverlayDrag(event: React.PointerEvent<HTMLDivElement>) {
    if (event.button !== 0 || !isTauri()) return;
    event.preventDefault();
    event.currentTarget.setPointerCapture(event.pointerId);
    const drag: DragState = {
      pointerId: event.pointerId,
      startX: event.screenX,
      startY: event.screenY,
      latestX: event.screenX,
      latestY: event.screenY,
    };
    dragState.current = drag;
    try {
      const position = await getCurrentWindow().outerPosition();
      if (dragState.current !== drag) return;
      drag.windowX = position.x;
      drag.windowY = position.y;
      scheduleOverlayMove(drag);
    } catch (reason) {
      dragState.current = null;
      setError(`Could not drag window: ${String(reason)}`);
    }
  }

  function scheduleOverlayMove(drag: DragState) {
    if (drag.frame !== undefined || drag.windowX === undefined || drag.windowY === undefined) return;
    drag.frame = window.requestAnimationFrame(() => {
      drag.frame = undefined;
      if (dragState.current !== drag) return;
      const x = Math.round(drag.windowX! + drag.latestX - drag.startX);
      const y = Math.round(drag.windowY! + drag.latestY - drag.startY);
      void getCurrentWindow().setPosition(new PhysicalPosition(x, y));
    });
  }

  function moveOverlay(event: React.PointerEvent<HTMLDivElement>) {
    const drag = dragState.current;
    if (!drag || drag.pointerId !== event.pointerId) return;
    drag.latestX = event.screenX;
    drag.latestY = event.screenY;
    scheduleOverlayMove(drag);
  }

  function endOverlayDrag(event: React.PointerEvent<HTMLDivElement>) {
    const drag = dragState.current;
    if (!drag || drag.pointerId !== event.pointerId) return;
    if (drag.frame !== undefined) window.cancelAnimationFrame(drag.frame);
    if (drag.windowX !== undefined && drag.windowY !== undefined) {
      const x = Math.round(drag.windowX + event.screenX - drag.startX);
      const y = Math.round(drag.windowY + event.screenY - drag.startY);
      void getCurrentWindow().setPosition(new PhysicalPosition(x, y));
    }
    dragState.current = null;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId);
  }

  return (
    <main className="app-shell">
      <header className="titlebar">
        <div className="brand">
          <span className="brand-mark">IA</span>
          <div>
            <strong>InterviewAssist</strong>
            <span>Private live workspace</span>
          </div>
        </div>

        <div
          className="drag-handle"
          title="Drag to move window"
          aria-label="Drag to move window"
          onPointerDown={(event) => void beginOverlayDrag(event)}
          onPointerMove={moveOverlay}
          onPointerUp={endOverlayDrag}
          onPointerCancel={endOverlayDrag}
        >
          <GripHorizontal size={18} />
          <span>Drag window</span>
        </div>

        <div className="window-actions">
          <button className="icon-button" aria-label="Settings" title="Settings" onClick={() => setSettingsOpen(true)}>
            <Settings size={16} />
          </button>
          <button
            className="icon-button"
            aria-label="Minimize to tray"
            title="Minimize to tray"
            onClick={() => void hideWindow()}
          >
            <Minus size={17} />
          </button>
          <button
            className="icon-button danger-hover"
            aria-label="Exit"
            title="Exit"
            onClick={() => void exitApplication()}
          >
            <X size={17} />
          </button>
        </div>
      </header>

      <section className="status-strip">
        <div className={`protection-status ${protection}`}>
          {protection === "protected" ? (
            <ShieldCheck size={16} />
          ) : (
            <ShieldOff size={16} />
          )}
          <span>
            {protection === "protected"
              ? "Screen share protected"
              : "Protection unavailable in browser preview"}
          </span>
        </div>
        <div className={`capture-pill ${captureState}`}>
          <i />
          {stateLabel}
        </div>
      </section>

      {error && <div className="error-banner"><span>{error}</span><button onClick={() => setError("")}>Dismiss</button></div>}

      <section className="control-card">
        <div className="segmented-control" aria-label="Capture source">
          <button
            className={source === "system" ? "active" : ""}
            onClick={() => changeSource("system")}
          >
            <Headphones size={17} />
            System Audio
          </button>
          <button
            className={source === "microphone" ? "active" : ""}
            onClick={() => changeSource("microphone")}
          >
            <Mic size={17} />
            Microphone
          </button>
        </div>

        <div className="select-wrap">
          <label htmlFor="language">Language</label>
          <div>
            <select
              id="language"
              value={language}
              onChange={(event) => setLanguage(event.target.value as Language)}
            >
              {Object.entries(languageLabels).map(([value, label]) => (
                <option key={value} value={value}>
                  {label}
                </option>
              ))}
            </select>
            <ChevronDown size={15} />
          </div>
        </div>

        <div className="capture-actions">
          <button className="primary-button" onClick={toggleCapture}>
            {captureState === "listening" ? (
              <Pause size={17} fill="currentColor" />
            ) : (
              <Play size={17} fill="currentColor" />
            )}
            {captureState === "listening"
              ? "Pause"
              : captureState === "paused"
                ? "Continue"
                : "Start"}
          </button>
          <button
            className="secondary-button icon-only"
            aria-label="Stop"
            title="Stop"
            disabled={captureState === "stopped"}
            onClick={() => void stopCapture().catch((reason) => setError(String(reason)))}
          >
            <CircleStop size={18} />
          </button>
        </div>
      </section>

      <section className="workspace-grid">
        <article className="panel transcript-panel">
          <div className="panel-heading">
            <div>
              <span className="eyebrow">INPUT</span>
              <h2>Live caption</h2>
            </div>
            <span className="source-label">
              {sourceLabels[source]} · {languageLabels[language]} · {captionEngine === "local" ? "Local" : "OpenAI"}
            </span>
          </div>

          <textarea
            value={transcript}
            onChange={(event) => setTranscript(event.target.value)}
            placeholder="Start listening or type a question here…"
            spellCheck
          />

          {captureState === "listening" && (
            <div className="partial-caption">
              <span className="audio-wave"><i /><i /><i /><i /></span>
              {partial || "Waiting for speech…"}
            </div>
          )}

          <div className="panel-footer">
            <div className="text-actions">
              <button
                onClick={() => void copyText(transcript, "transcript")}
                disabled={!transcript}
              >
                {copied === "transcript" ? <Check size={15} /> : <Copy size={15} />}
                {copied === "transcript" ? "Copied" : "Copy"}
              </button>
              <button onClick={() => setTranscript("")} disabled={!transcript}>
                <Trash2 size={15} /> Clear
              </button>
            </div>
            <button
              className="submit-button"
              onClick={() => void submit()}
              disabled={!transcript.trim() || answering}
            >
              Submit
              <span>Ctrl ↵</span>
              <Send size={15} />
            </button>
          </div>
        </article>

        <article className="panel answer-panel">
          <div className="panel-heading">
            <div>
              <span className="eyebrow">{answerProvider === "chatgpt" ? "CHATGPT PLAN" : answerProvider === "openrouter" ? "OPENROUTER FREE" : answerProvider === "google" ? "GOOGLE GEMINI" : "OPENAI API"}</span>
              <h2>Suggested answer</h2>
            </div>
            <div className="answer-actions">
              <button className="answer-copy" onClick={() => void copyText(latestAnswer, "answer")} disabled={!latestAnswer}>
                {copied === "answer" ? <Check size={15} /> : <Copy size={15} />}
                {copied === "answer" ? "Copied" : "Copy latest"}
              </button>
              <button className="answer-copy" onClick={() => setChatHistory([])} disabled={!chatHistory.length || answering} title="Clear chat history">
                <Trash2 size={15} /> Clear
              </button>
              {answering && <button className="answer-copy" onClick={() => void cancelResponse()}>Cancel</button>}
            </div>
          </div>

          <div ref={answerScrollRef} className={`answer-content ${chatHistory.length ? "has-content" : ""}`}>
            {chatHistory.length ? chatHistory.map((message) => (
              <div className={`chat-message ${message.role}`} key={message.id}>
                <span>{message.role === "user" ? "You" : message.label || answerModelLabel(answerProvider, openRouterModel, googleModel, model)}</span>
                <div>{message.role === "assistant" && message.content
                  ? <div className="markdown-content"><MarkdownAnswer content={message.content} /></div>
                  : message.content || (answering && message.role === "assistant" ? "Thinking…" : "")}</div>
              </div>
            )) : (
              <div className="empty-state">
                <span className="spark">✦</span>
                <strong>Your answer will appear here</strong>
                <p>Edit the caption, then submit when you are ready.</p>
              </div>
            )}
          </div>
        </article>
      </section>

      {settingsOpen && <div className="modal-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) setSettingsOpen(false); }}>
        <section className="settings-modal" role="dialog" aria-modal="true" aria-label="Settings">
          <div className="modal-heading"><div><span className="eyebrow">SETTINGS</span><h2>Caption & AI</h2></div><button className="icon-button" onClick={() => setSettingsOpen(false)}><X size={18} /></button></div>
          <label>1. Live caption engine <small>Controls audio transcription only</small></label>
          <div className="engine-options">
            <button className={captionEngine === "local" ? "active" : ""} onClick={() => setCaptionEngine("local")}>
              <strong>Local Whisper</strong><span>Offline · audio stays on this PC</span>
            </button>
            <button className={captionEngine === "openai" ? "active" : ""} onClick={() => setCaptionEngine("openai")}>
              <strong>OpenAI</strong><span>Cloud · requires API key</span>
            </button>
          </div>
          {captionEngine === "local" && <div className="local-model-list">
            {localModels.map((item) => <div className={`local-model-row ${selectedLocalModel === item.id ? "selected" : ""}`} key={item.id}>
              <label className="model-choice">
                <input type="radio" name="local-model" checked={selectedLocalModel === item.id} onChange={() => setSelectedLocalModel(item.id)} />
                <div><strong>{item.name}</strong><span>{item.description} · {item.sizeLabel}{item.installed ? " · Installed" : ""}</span></div>
              </label>
              {!item.installed && <button className="secondary-button" disabled={settingsBusy} onClick={() => void installLocalModel(item.id)}><Download size={15} />{modelDownload?.modelId === item.id ? `${modelDownload.percent}%` : "Download"}</button>}
            </div>)}
          </div>}
          <label>2. Answer provider <small>Used only after you press Submit</small></label>
          <div className="engine-options answer-provider-options">
            <button className={answerProvider === "chatgpt" ? "active" : ""} onClick={() => setAnswerProvider("chatgpt")}>
              <strong>ChatGPT plan</strong><span>Đăng nhập bằng Plus / Pro · không cần API key</span>
            </button>
            <button className={answerProvider === "openrouter" ? "active" : ""} onClick={() => setAnswerProvider("openrouter")}>
              <strong>OpenRouter Free</strong><span>Free router · model tự chọn</span>
            </button>
            <button className={answerProvider === "openai" ? "active" : ""} onClick={() => setAnswerProvider("openai")}>
              <strong>OpenAI</strong><span>Dùng API key và model riêng</span>
            </button>
            <button className={answerProvider === "google" ? "active" : ""} onClick={() => setAnswerProvider("google")}>
              <strong>Google AI Studio</strong><span>Gemini Free Tier</span>
            </button>
          </div>
          {answerProvider === "chatgpt" && <>
            <label>ChatGPT account <small>{chatGptStatus.connected ? `${chatGptStatus.email || "Connected"} · plan usage enabled` : "For eligible ChatGPT Plus and Pro plans"}</small></label>
            <div className="chatgpt-connect-row">
              {!chatGptStatus.connected
                ? <button className="primary-button" disabled={settingsBusy} onClick={() => void connectChatGpt()}>Continue with ChatGPT</button>
                : <button className="secondary-button" disabled={settingsBusy} onClick={() => void disconnectChatGpt()}>Disconnect</button>}
            </div>
            {chatGptStatus.connected && <>
              <label>ChatGPT model <small>Models available to this account</small></label>
              <select value={model} onChange={(event) => setModel(event.target.value)}>
                {chatGptModels.map((item) => <option value={item.slug} key={item.slug}>{item.display_name}</option>)}
              </select>
            </>}
          </>}
          {answerProvider === "openrouter" && <>
            <label>Free response mode <small>Fast multilingual avoids random slow models</small></label>
            <select value={openRouterModel} onChange={(event) => setOpenRouterModel(event.target.value as OpenRouterModel)}>
              <option value="google/gemma-4-26b-a4b-it:free">Fast multilingual · recommended</option>
              <option value="openrouter/free">Auto free router · random model</option>
            </select>
            <label>OpenRouter API key <small>{openRouterConfigured ? "Configured securely in Windows Credential Manager" : "Required for the free router"}</small></label>
            <input type="password" value={openRouterKey} onChange={(event) => setOpenRouterKey(event.target.value)} placeholder={openRouterConfigured ? "Enter a new key to replace it" : "sk-or-v1-…"} autoComplete="off" />
          </>}
          {answerProvider === "google" && <>
            <label>Gemini model <small>Availability and quota depend on your Free Tier project</small></label>
            <select value={googleModel} onChange={(event) => setGoogleModel(event.target.value as GoogleModel)}>
              <option value="gemini-3.8-flash">Gemini 3.8 Flash · recommended</option>
              <option value="gemini-3.1-flash-lite">Gemini 3.1 Flash-Lite · fastest</option>
            </select>
            <label>Google AI Studio API key <small>{googleConfigured ? "Configured securely in Windows Credential Manager" : "Create a key in Google AI Studio"}</small></label>
            <input type="password" value={googleKey} onChange={(event) => setGoogleKey(event.target.value)} placeholder={googleConfigured ? "Enter a new key to replace it" : "AIza…"} autoComplete="off" />
          </>}
          <label>OpenAI API key <small>{apiConfigured ? "Configured securely in Windows Credential Manager" : "Only needed for OpenAI API caption/answers; not needed for ChatGPT plan"}</small></label>
          <input type="password" value={apiKey} onChange={(event) => setApiKey(event.target.value)} placeholder={apiConfigured ? "Enter a new key to replace it" : "sk-…"} autoComplete="off" />
          {answerProvider === "openai" && <><label>Response model</label><input value={model} onChange={(event) => setModel(event.target.value)} /></>}
          <label>System prompt</label>
          <textarea value={systemPrompt} onChange={(event) => setSystemPrompt(event.target.value)} />
          <p className="privacy-note">Keys are stored in Windows Credential Manager. Google Gemini Free Tier has limited quotas and Google may use Free Tier content to improve its products.</p>
          <div className="modal-actions">
            {answerProvider === "openrouter" && openRouterConfigured && <button className="secondary-button" onClick={() => void deleteOpenRouterKey().then(() => setOpenRouterConfigured(false)).catch((reason) => setError(String(reason)))}>Delete Router key</button>}
            {answerProvider === "google" && googleConfigured && <button className="secondary-button" onClick={() => void deleteGoogleKey().then(() => setGoogleConfigured(false)).catch((reason) => setError(String(reason)))}>Delete Google key</button>}
            {apiConfigured && <button className="secondary-button" onClick={() => void deleteApiKey().then(() => setApiConfigured(false)).catch((reason) => setError(String(reason)))}>Delete key</button>}
            <button className="primary-button" disabled={settingsBusy} onClick={() => void saveSettings()}>{settingsBusy ? "Working…" : "Save"}</button>
          </div>
        </section>
      </div>}
    </main>
  );
}

export default App;

