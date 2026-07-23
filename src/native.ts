import { invoke } from "@tauri-apps/api/core";

export type EngineState = "running" | "stopped" | "unknown";

export type EngineConfig = {
  id: string;
  name: string;
  description: string;
  cwd: string;
  command: string;
  port: number | null;
  uiUrl: string | null;
  healthUrl: string | null;
};

export type EngineStatus = EngineConfig & {
  logPath: string | null;
  state: EngineState;
  pid: number | null;
  healthOk: boolean;
  healthMessage: string;
};

export type NetworkSnapshot = {
  publicIp: string;
  city: string;
  region: string;
  country: string;
  org: string;
  chubStatus: number | null;
  chubCountry: string;
  chubRegion: string;
  chubOk: boolean;
  message: string;
};

export function isTauriRuntime() {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

const browserMessage =
  "Open this inside the Tauri desktop shell to control local engines.";

const browserRegistryKey = "spicy-tavern-cockpit.engine-registry";

const defaultEngineConfigs: EngineConfig[] = [
  {
    id: "marinara-clean",
    name: "Marinara Clean",
    description: "Upstream-clean Marinara Engine checkout.",
    cwd: "/Library/Developer/GitHub2.0/Marinara-Engine-upstream-clean",
    command:
      "export PATH=/opt/homebrew/Cellar/node@24/24.18.0/bin:$PATH; export PORT=7860; export NODE_ENV=production; unset CHUB_OUTBOUND_PROXY; node packages/server/dist/index.js",
    port: 7860,
    uiUrl: "http://127.0.0.1:7860",
    healthUrl: "http://127.0.0.1:7860/api/health",
  },
  {
    id: "marinara-sandbox",
    name: "Marinara Sandbox",
    description: "Experimental Marinara/HumanOS checkout.",
    cwd: "/Library/Developer/GitHub2.0/SillyTavern/plugins/SillyTavern-EverythingPlugin/Untitled/Marinara-Engine",
    command:
      "export PATH=/opt/homebrew/Cellar/node@24/24.18.0/bin:$PATH; export PORT=7861; export NODE_ENV=production; unset CHUB_OUTBOUND_PROXY; node packages/server/dist/index.js",
    port: 7861,
    uiUrl: "http://127.0.0.1:7861",
    healthUrl: "http://127.0.0.1:7861/api/health",
  },
  {
    id: "sillytavern",
    name: "SillyTavern",
    description: "Local SillyTavern web engine.",
    cwd: "/Library/Developer/GitHub2.0/SillyTavern",
    command: "node server.js",
    port: 8000,
    uiUrl: "http://127.0.0.1:8000",
    healthUrl: "http://127.0.0.1:8000",
  },
  {
    id: "ollama",
    name: "Ollama",
    description: "Local model runtime.",
    cwd: "/",
    command: "ollama serve",
    port: 11434,
    uiUrl: null,
    healthUrl: "http://127.0.0.1:11434/api/tags",
  },
];

function statusFromConfig(engine: EngineConfig): EngineStatus {
  return {
    ...engine,
    logPath: null,
    state: "stopped",
    pid: null,
    healthOk: false,
    healthMessage: browserMessage,
  };
}

function browserConfigs() {
  const stored = window.localStorage.getItem(browserRegistryKey);
  if (!stored) return defaultEngineConfigs;
  try {
    return JSON.parse(stored) as EngineConfig[];
  } catch {
    return defaultEngineConfigs;
  }
}

function saveBrowserConfigs(engines: EngineConfig[]) {
  window.localStorage.setItem(browserRegistryKey, JSON.stringify(engines));
}

export async function listEngines(): Promise<EngineStatus[]> {
  if (!isTauriRuntime()) return browserConfigs().map(statusFromConfig);
  return invoke<EngineStatus[]>("list_engines");
}

export async function refreshEngine(id: string): Promise<EngineStatus> {
  if (!isTauriRuntime()) throw new Error(browserMessage);
  return invoke<EngineStatus>("engine_status", { id });
}

export async function startEngine(id: string): Promise<EngineStatus> {
  if (!isTauriRuntime()) throw new Error(browserMessage);
  return invoke<EngineStatus>("start_engine", { id });
}

export async function stopEngine(id: string): Promise<EngineStatus> {
  if (!isTauriRuntime()) throw new Error(browserMessage);
  return invoke<EngineStatus>("stop_engine", { id });
}

export async function stopAllEngines(): Promise<EngineStatus[]> {
  if (!isTauriRuntime()) return browserConfigs().map(statusFromConfig);
  return invoke<EngineStatus[]>("stop_all_engines");
}

export async function saveEngineConfig(engine: EngineConfig): Promise<EngineStatus> {
  if (!isTauriRuntime()) {
    const engines = browserConfigs();
    const index = engines.findIndex((candidate) => candidate.id === engine.id);
    if (index === -1) throw new Error(`Unknown engine: ${engine.id}`);
    engines[index] = engine;
    saveBrowserConfigs(engines);
    return statusFromConfig(engine);
  }
  return invoke<EngineStatus>("save_engine_config", { engine });
}

export async function resetEngineRegistry(): Promise<EngineStatus[]> {
  if (!isTauriRuntime()) {
    saveBrowserConfigs(defaultEngineConfigs);
    return defaultEngineConfigs.map(statusFromConfig);
  }
  return invoke<EngineStatus[]>("reset_engine_registry");
}

export async function networkSnapshot(): Promise<NetworkSnapshot> {
  if (!isTauriRuntime()) {
    return {
      publicIp: "",
      city: "",
      region: "",
      country: "",
      org: "",
      chubStatus: null,
      chubCountry: "",
      chubRegion: "",
      chubOk: false,
      message: browserMessage,
    };
  }
  return invoke<NetworkSnapshot>("network_snapshot");
}
