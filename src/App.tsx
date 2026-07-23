import {
  Activity,
  AlertTriangle,
  Cable,
  CheckCircle2,
  CircleStop,
  Copy,
  ExternalLink,
  Globe2,
  Logs,
  Play,
  RefreshCw,
  Repeat2,
  RotateCcw,
  Save,
  Settings2,
  ShieldCheck,
  SquareTerminal,
  Stethoscope,
  XCircle,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import {
  EngineConfig,
  DiagnosticCheck,
  DiagnosticsSnapshot,
  EngineLogTail,
  EngineStatus,
  NetworkSnapshot,
  diagnosticsSnapshot,
  engineLogTail,
  isTauriRuntime,
  listEngines,
  networkSnapshot,
  refreshEngine,
  resetEngineRegistry,
  restartEngine,
  saveEngineConfig,
  startEngine,
  stopAllEngines,
  stopEngine,
} from "./native";

type ViewMode = "embedded" | "logs" | "network" | "registry" | "diagnostics";

const stateLabel: Record<string, string> = {
  running: "Running",
  stopped: "Stopped",
  unknown: "Unknown",
};

export function App() {
  const [engines, setEngines] = useState<EngineStatus[]>([]);
  const [selectedId, setSelectedId] = useState<string>("marinara-clean");
  const [mode, setMode] = useState<ViewMode>("embedded");
  const [network, setNetwork] = useState<NetworkSnapshot | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [notice, setNotice] = useState<string>("");

  const selected = useMemo(
    () => engines.find((engine) => engine.id === selectedId) ?? engines[0] ?? null,
    [engines, selectedId],
  );

  async function refreshAll() {
    try {
      const next = await listEngines();
      setEngines(next);
      if (!next.some((engine) => engine.id === selectedId)) {
        setSelectedId(next[0]?.id ?? "marinara-clean");
      }
    } catch (error) {
      setNotice(error instanceof Error ? error.message : "Could not refresh engines.");
    }
  }

  async function refreshNetwork() {
    try {
      setNetwork(await networkSnapshot());
    } catch (error) {
      setNotice(error instanceof Error ? error.message : "Could not refresh network status.");
    }
  }

  useEffect(() => {
    void refreshAll();
    void refreshNetwork();
    const timer = window.setInterval(() => {
      void refreshAll();
    }, 5000);
    return () => window.clearInterval(timer);
  }, []);

  async function runEngineAction(id: string, action: "start" | "stop" | "restart" | "refresh") {
    setBusyId(id);
    setNotice("");
    try {
      const status =
        action === "start"
          ? await startEngine(id)
          : action === "stop"
            ? await stopEngine(id)
            : action === "restart"
              ? await restartEngine(id)
              : await refreshEngine(id);
      setEngines((current) => current.map((engine) => (engine.id === id ? status : engine)));
      if (action === "start") setMode("embedded");
    } catch (error) {
      setNotice(error instanceof Error ? error.message : "Engine action failed.");
    } finally {
      setBusyId(null);
    }
  }

  async function stopEverything() {
    setBusyId("all");
    setNotice("");
    try {
      setEngines(await stopAllEngines());
    } catch (error) {
      setNotice(error instanceof Error ? error.message : "Stop all failed.");
    } finally {
      setBusyId(null);
    }
  }

  async function saveRegistry(engine: EngineConfig) {
    setBusyId(`registry:${engine.id}`);
    setNotice("");
    try {
      const status = await saveEngineConfig(engine);
      setEngines((current) => current.map((candidate) => (candidate.id === status.id ? status : candidate)));
      setNotice(`${status.name} registry saved.`);
    } catch (error) {
      setNotice(error instanceof Error ? error.message : "Could not save engine registry.");
    } finally {
      setBusyId(null);
    }
  }

  async function resetRegistry() {
    setBusyId("registry:reset");
    setNotice("");
    try {
      const next = await resetEngineRegistry();
      setEngines(next);
      if (!next.some((engine) => engine.id === selectedId)) {
        setSelectedId(next[0]?.id ?? "marinara-clean");
      }
      setNotice("Engine registry reset to defaults.");
    } catch (error) {
      setNotice(error instanceof Error ? error.message : "Could not reset engine registry.");
    } finally {
      setBusyId(null);
    }
  }

  return (
    <main className="app-shell">
      <aside className="sidebar">
        <div className="brand-block">
          <div className="brand-mark">MTC</div>
          <div>
            <h1>SpicyTavernCockPit</h1>
            <p>Local engine control deck</p>
          </div>
        </div>

        <nav className="engine-list">
          {engines.map((engine) => (
            <button
              className={`engine-row ${selectedId === engine.id ? "active" : ""}`}
              key={engine.id}
              onClick={() => setSelectedId(engine.id)}
              type="button"
            >
              <span className={`status-dot ${engine.state}`} />
              <span>
                <strong>{engine.name}</strong>
                <small>{engine.port ? `:${engine.port}` : "utility"}</small>
              </span>
            </button>
          ))}
        </nav>

        <button className="danger-button" disabled={busyId === "all"} onClick={stopEverything} type="button">
          <CircleStop size={18} />
          Stop All
        </button>
      </aside>

      <section className="workspace">
        <header className="topbar">
          <div>
            <p className="eyebrow">Stable desktop shell</p>
            <h2>{selected?.name ?? "No engine selected"}</h2>
          </div>
          <div className="toolbar">
            <button onClick={() => void refreshAll()} type="button">
              <RefreshCw size={17} />
              Refresh
            </button>
            <button onClick={() => setMode("network")} type="button">
              <Globe2 size={17} />
              Network
            </button>
            <button onClick={() => setMode("diagnostics")} type="button">
              <Stethoscope size={17} />
              Diagnostics
            </button>
            <button onClick={() => setMode("registry")} type="button">
              <Settings2 size={17} />
              Registry
            </button>
            <button onClick={() => setMode("logs")} type="button">
              <Logs size={17} />
              Logs
            </button>
          </div>
        </header>

        {!isTauriRuntime() && (
          <div className="notice">
            Browser preview is UI-only. Launch with <code>npm run tauri:dev</code> to control processes.
          </div>
        )}

        {notice && <div className="notice warn">{notice}</div>}

        {selected && (
          <section className="status-strip">
            <StatusPill icon={<Activity size={16} />} label={stateLabel[selected.state]} ok={selected.state === "running"} />
            <StatusPill
              icon={<SquareTerminal size={16} />}
              label={processLabel(selected)}
              ok={selected.processSource === "managed"}
            />
            <StatusPill icon={<Cable size={16} />} label={selected.healthMessage || "Health pending"} ok={selected.healthOk} />
            <StatusPill icon={<ShieldCheck size={16} />} label={networkLabel(network)} ok={network?.country === "US"} />
            <div className="engine-actions">
              <button
                disabled={busyId === selected.id}
                onClick={() => void runEngineAction(selected.id, "start")}
                type="button"
              >
                <Play size={17} />
                Start
              </button>
              <button
                disabled={busyId === selected.id}
                onClick={() => void runEngineAction(selected.id, "stop")}
                type="button"
              >
                <CircleStop size={17} />
                Stop
              </button>
              <button
                disabled={busyId === selected.id}
                onClick={() => void runEngineAction(selected.id, "restart")}
                type="button"
              >
                <Repeat2 size={17} />
                Restart
              </button>
            </div>
          </section>
        )}

        {mode === "network" ? (
          <NetworkPanel network={network} onRefresh={() => void refreshNetwork()} />
        ) : mode === "diagnostics" ? (
          <DiagnosticsPanel />
        ) : mode === "logs" ? (
          <LogPanel selected={selected} />
        ) : mode === "registry" ? (
          <RegistryPanel
            busy={busyId?.startsWith("registry:") ?? false}
            onReset={() => void resetRegistry()}
            onSave={(engine) => void saveRegistry(engine)}
            selected={selected}
          />
        ) : (
          <EngineFrame
            busy={selected ? busyId === selected.id : false}
            onStart={(id) => void runEngineAction(id, "start")}
            selected={selected}
          />
        )}
      </section>
    </main>
  );
}

function StatusPill(props: { icon: React.ReactNode; label: string; ok: boolean }) {
  return (
    <div className={`status-pill ${props.ok ? "ok" : "muted"}`}>
      {props.icon}
      <span>{props.label}</span>
    </div>
  );
}

function networkLabel(snapshot: NetworkSnapshot | null) {
  if (!snapshot) return "Network pending";
  if (snapshot.country) return `${snapshot.country} · ${snapshot.publicIp}`;
  return snapshot.message || "Network unknown";
}

function processLabel(engine: EngineStatus) {
  if (engine.processSource === "managed") return engine.pid ? `Managed pid ${engine.pid}` : "Managed process";
  if (engine.processSource === "external") return engine.processMessage || "Running outside cockpit";
  return engine.processMessage || "No process";
}

function EngineFrame({
  busy,
  onStart,
  selected,
}: {
  busy: boolean;
  onStart: (id: string) => void;
  selected: EngineStatus | null;
}) {
  const [frameKey, setFrameKey] = useState(0);
  const [copyMessage, setCopyMessage] = useState("");

  useEffect(() => {
    setFrameKey(0);
    setCopyMessage("");
  }, [selected?.id, selected?.uiUrl]);

  if (!selected?.uiUrl) {
    return (
      <div className="empty-panel">
        <SquareTerminal size={44} />
        <h3>No embedded view for this engine</h3>
        <p>This engine is controlled by health checks and commands only.</p>
      </div>
    );
  }

  const routeReady = selected.portListening || selected.healthOk || selected.state === "running";

  async function copyUrl() {
    if (!selected?.uiUrl) return;
    try {
      await navigator.clipboard.writeText(selected.uiUrl);
      setCopyMessage("Copied");
    } catch {
      setCopyMessage("Copy failed");
    }
  }

  return (
    <div className={`frame-panel ${routeReady ? "ready" : "waiting"}`}>
      <div className="frame-header">
        <div className="frame-address">
          <span className={`route-dot ${routeReady ? "ready" : "waiting"}`} />
          <span>{selected.uiUrl}</span>
        </div>
        <div className="frame-tools">
          {copyMessage && <small>{copyMessage}</small>}
          <button onClick={() => void copyUrl()} type="button">
            <Copy size={16} />
            Copy
          </button>
          <button onClick={() => setFrameKey((current) => current + 1)} type="button">
            <RefreshCw size={16} />
            Reload
          </button>
          <a href={selected.uiUrl} rel="noreferrer" target="_blank">
            <ExternalLink size={16} />
            Open
          </a>
        </div>
      </div>
      {routeReady ? (
        <iframe key={`${selected.id}:${frameKey}`} referrerPolicy="no-referrer" src={selected.uiUrl} title={selected.name} />
      ) : (
        <div className="route-placeholder">
          <SquareTerminal size={44} />
          <h3>{selected.name} is offline</h3>
          <p>{selected.processMessage || selected.healthMessage}</p>
          <button disabled={busy} onClick={() => onStart(selected.id)} type="button">
            <Play size={17} />
            Start
          </button>
        </div>
      )}
    </div>
  );
}

function NetworkPanel(props: { network: NetworkSnapshot | null; onRefresh: () => void }) {
  const snapshot = props.network;
  return (
    <div className="detail-panel">
      <div className="panel-title">
        <div>
          <p className="eyebrow">Egress</p>
          <h3>Network Status</h3>
        </div>
        <button onClick={props.onRefresh} type="button">
          <RefreshCw size={17} />
          Refresh Network
        </button>
      </div>
      <div className="metric-grid">
        <Metric label="Public IP" value={snapshot?.publicIp || "Pending"} />
        <Metric label="Location" value={snapshot ? `${snapshot.city}, ${snapshot.region}, ${snapshot.country}` : "Pending"} />
        <Metric label="Provider" value={snapshot?.org || "Pending"} />
        <Metric label="Chub" value={snapshot ? `${snapshot.chubStatus ?? "n/a"} · ${snapshot.chubCountry || "unknown"}` : "Pending"} />
      </div>
      <p className="panel-note">{snapshot?.message || "Network checks run from the desktop shell process."}</p>
    </div>
  );
}

function DiagnosticsPanel() {
  const [snapshot, setSnapshot] = useState<DiagnosticsSnapshot | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  async function refreshDiagnostics() {
    setBusy(true);
    setError("");
    try {
      setSnapshot(await diagnosticsSnapshot());
    } catch (error) {
      setError(error instanceof Error ? error.message : "Could not refresh diagnostics.");
    } finally {
      setBusy(false);
    }
  }

  useEffect(() => {
    void refreshDiagnostics();
  }, []);

  const checksByCategory = useMemo(() => groupChecks(snapshot?.checks ?? []), [snapshot]);

  return (
    <div className="detail-panel diagnostics-panel">
      <div className="panel-title">
        <div>
          <p className="eyebrow">Health</p>
          <h3>Diagnostics</h3>
        </div>
        <button disabled={busy} onClick={() => void refreshDiagnostics()} type="button">
          <RefreshCw size={17} />
          Refresh Checks
        </button>
      </div>
      {error && <div className="notice warn">{error}</div>}
      <p className="panel-note">
        {snapshot ? `Generated ${new Date(snapshot.generatedAt).toLocaleString()}` : "Diagnostics pending."}
      </p>
      <div className="diagnostic-sections">
        {checksByCategory.map(([category, checks]) => (
          <section className="diagnostic-section" key={category}>
            <h4>{category}</h4>
            <div className="diagnostic-grid">
              {checks.map((check) => (
                <DiagnosticRow check={check} key={check.id} />
              ))}
            </div>
          </section>
        ))}
      </div>
    </div>
  );
}

function groupChecks(checks: DiagnosticCheck[]) {
  const grouped = new Map<string, DiagnosticCheck[]>();
  for (const check of checks) {
    grouped.set(check.category, [...(grouped.get(check.category) ?? []), check]);
  }
  return Array.from(grouped.entries());
}

function DiagnosticRow({ check }: { check: DiagnosticCheck }) {
  const icon =
    check.status === "ok" ? (
      <CheckCircle2 size={17} />
    ) : check.status === "warn" ? (
      <AlertTriangle size={17} />
    ) : (
      <XCircle size={17} />
    );

  return (
    <article className={`diagnostic-row ${check.status}`}>
      <div className="diagnostic-status">{icon}</div>
      <div>
        <strong>{check.label}</strong>
        <span>{check.message}</span>
        {check.detail && <code>{check.detail}</code>}
      </div>
    </article>
  );
}

function LogPanel({ selected }: { selected: EngineStatus | null }) {
  const [tail, setTail] = useState<EngineLogTail | null>(null);
  const [busy, setBusy] = useState(false);

  async function refreshTail() {
    if (!selected) {
      setTail(null);
      return;
    }
    setBusy(true);
    try {
      setTail(await engineLogTail(selected.id));
    } catch (error) {
      setTail({
        path: selected.logPath,
        content: "",
        lineCount: 0,
        message: error instanceof Error ? error.message : "Could not read log tail.",
      });
    } finally {
      setBusy(false);
    }
  }

  useEffect(() => {
    void refreshTail();
    if (!selected) return;
    const timer = window.setInterval(() => {
      void refreshTail();
    }, 3000);
    return () => window.clearInterval(timer);
  }, [selected?.id, selected?.logPath]);

  return (
    <div className="detail-panel">
      <div className="panel-title">
        <div>
          <p className="eyebrow">Process Logs</p>
          <h3>{selected?.name ?? "No engine"}</h3>
        </div>
        <button disabled={busy || !selected} onClick={() => void refreshTail()} type="button">
          <RefreshCw size={17} />
          Refresh Tail
        </button>
      </div>
      <div className="log-box">
        {tail?.path || selected?.logPath ? (
          <>
            <span>Log file:</span>
            <code>{tail?.path ?? selected?.logPath}</code>
          </>
        ) : (
          <span>No log file assigned yet. Start an engine to create one.</span>
        )}
      </div>
      <div className="log-tail">
        <span>{tail?.message ?? "Log tail pending."}</span>
        <pre>{tail?.content || "No log output yet."}</pre>
      </div>
    </div>
  );
}

function RegistryPanel(props: {
  busy: boolean;
  onReset: () => void;
  onSave: (engine: EngineConfig) => void;
  selected: EngineStatus | null;
}) {
  const [draft, setDraft] = useState<EngineConfig | null>(() => engineConfigFromStatus(props.selected));

  useEffect(() => {
    setDraft(engineConfigFromStatus(props.selected));
  }, [props.selected]);

  if (!draft) {
    return (
      <div className="empty-panel">
        <Settings2 size={44} />
        <h3>No engine selected</h3>
        <button disabled={props.busy} onClick={props.onReset} type="button">
          <RotateCcw size={17} />
          Reset Registry
        </button>
      </div>
    );
  }

  function updateField<K extends keyof EngineConfig>(field: K, value: EngineConfig[K]) {
    setDraft((current) => (current ? { ...current, [field]: value } : current));
  }

  return (
    <form
      className="detail-panel registry-panel"
      onSubmit={(event) => {
        event.preventDefault();
        props.onSave(draft);
      }}
    >
      <div className="panel-title">
        <div>
          <p className="eyebrow">Engine Registry</p>
          <h3>{draft.name}</h3>
        </div>
        <div className="panel-actions">
          <button disabled={props.busy} onClick={props.onReset} type="button">
            <RotateCcw size={17} />
            Reset
          </button>
          <button disabled={props.busy} type="submit">
            <Save size={17} />
            Save
          </button>
        </div>
      </div>

      <div className="registry-grid">
        <label className="field span-2">
          <span>Name</span>
          <input value={draft.name} onChange={(event) => updateField("name", event.target.value)} />
        </label>
        <label className="field">
          <span>Id</span>
          <input readOnly value={draft.id} />
        </label>
        <label className="field">
          <span>Port</span>
          <input
            inputMode="numeric"
            min={1}
            type="number"
            value={draft.port ?? ""}
            onChange={(event) => updateField("port", event.target.value ? Number(event.target.value) : null)}
          />
        </label>
        <label className="field span-4">
          <span>Path</span>
          <input value={draft.cwd} onChange={(event) => updateField("cwd", event.target.value)} />
        </label>
        <label className="field span-2">
          <span>UI URL</span>
          <input value={draft.uiUrl ?? ""} onChange={(event) => updateField("uiUrl", event.target.value || null)} />
        </label>
        <label className="field span-2">
          <span>Health URL</span>
          <input value={draft.healthUrl ?? ""} onChange={(event) => updateField("healthUrl", event.target.value || null)} />
        </label>
        <label className="field span-4">
          <span>Command</span>
          <textarea value={draft.command} onChange={(event) => updateField("command", event.target.value)} />
        </label>
        <label className="field span-4">
          <span>Description</span>
          <input value={draft.description} onChange={(event) => updateField("description", event.target.value)} />
        </label>
      </div>
    </form>
  );
}

function engineConfigFromStatus(engine: EngineStatus | null): EngineConfig | null {
  if (!engine) return null;
  return {
    id: engine.id,
    name: engine.name,
    description: engine.description,
    cwd: engine.cwd,
    command: engine.command,
    port: engine.port,
    uiUrl: engine.uiUrl,
    healthUrl: engine.healthUrl,
  };
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="metric">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}
