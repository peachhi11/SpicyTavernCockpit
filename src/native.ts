import { invoke } from "@tauri-apps/api/core";

export type EngineState = "running" | "stopped" | "unknown";

export type EngineStatus = {
  id: string;
  name: string;
  description: string;
  cwd: string;
  command: string;
  port: number | null;
  uiUrl: string | null;
  healthUrl: string | null;
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

export async function listEngines(): Promise<EngineStatus[]> {
  if (!isTauriRuntime()) return [];
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
  if (!isTauriRuntime()) return [];
  return invoke<EngineStatus[]>("stop_all_engines");
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
