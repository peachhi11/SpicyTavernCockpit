import { spawn } from "node:child_process";
import http from "node:http";

const host = "localhost";
const port = 3027;
const devUrl = `http://${host}:${port}/`;

function isDevServerReady() {
  return new Promise((resolve) => {
    const request = http.get(devUrl, (response) => {
      response.resume();
      resolve(response.statusCode ? response.statusCode < 500 : false);
    });
    request.on("error", () => resolve(false));
    request.setTimeout(500, () => {
      request.destroy();
      resolve(false);
    });
  });
}

let child = null;

function exitCleanly() {
  if (child) {
    child.kill("SIGTERM");
  }
  process.exit(0);
}

process.on("SIGINT", exitCleanly);
process.on("SIGTERM", exitCleanly);

if (await isDevServerReady()) {
  console.log(`[tauri-before-dev] Reusing existing Vite server at ${devUrl}`);
  setInterval(() => {}, 60_000);
} else {
  console.log(`[tauri-before-dev] Starting Vite server at ${devUrl}`);
  child = spawn("npm", ["run", "dev"], {
    stdio: "inherit",
    env: process.env,
  });

  child.on("exit", (code, signal) => {
    if (signal) {
      process.kill(process.pid, signal);
      return;
    }
    process.exit(code ?? 0);
  });
}
