import {
  Activity,
  Cable,
  CircleStop,
  ExternalLink,
  Globe2,
  Logs,
  Play,
  RefreshCw,
  ShieldCheck,
  SquareTerminal,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import {
  EngineStatus,
  NetworkSnapshot,
  isTauriRuntime,
  listEngines,
  networkSnapshot,
  refreshEngine,
  startEngine,
  stopAllEngines,
  stopEngine,
} from "./native";

type ViewMode = "embedded" | "logs" | "network";

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

  async function runEngineAction(id: string, action: "start" | "stop" | "refresh") {
    setBusyId(id);
    setNotice("");
    try {
      const status =
        action === "start"
          ? await startEngine(id)
          : action === "stop"
            ? await stopEngine(id)
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
            </div>
          </section>
        )}

        {mode === "network" ? (
          <NetworkPanel network={network} onRefresh={() => void refreshNetwork()} />
        ) : mode === "logs" ? (
          <LogPanel selected={selected} />
        ) : (
          <EngineFrame selected={selected} />
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

function EngineFrame({ selected }: { selected: EngineStatus | null }) {
  if (!selected?.uiUrl) {
    return (
      <div className="empty-panel">
        <SquareTerminal size={44} />
        <h3>No embedded view for this engine</h3>
        <p>This engine is controlled by health checks and commands only.</p>
      </div>
    );
  }

  return (
    <div className="frame-panel">
      <div className="frame-header">
        <span>{selected.uiUrl}</span>
        <a href={selected.uiUrl} rel="noreferrer" target="_blank">
          <ExternalLink size={16} />
          Open
        </a>
      </div>
      <iframe src={selected.uiUrl} title={selected.name} />
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

function LogPanel({ selected }: { selected: EngineStatus | null }) {
  return (
    <div className="detail-panel">
      <div className="panel-title">
        <div>
          <p className="eyebrow">Process Logs</p>
          <h3>{selected?.name ?? "No engine"}</h3>
        </div>
      </div>
      <div className="log-box">
        {selected?.logPath ? (
          <>
            <span>Log file:</span>
            <code>{selected.logPath}</code>
          </>
        ) : (
          <span>No log file assigned yet. Start an engine to create one.</span>
        )}
      </div>
    </div>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="metric">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}
