# SpicyTavernCockPit

SillyTavern and Marinara are engines with web wrappers. SpicyTavernCockPit is the desktop shell that gives those engines one stable macOS app identity, one control surface, and one place to inspect local runtime health.

The first cockpit goal is deliberately practical: make the local stack easier to start, stop, route through VPN, and diagnose.

## MVP Surface

- Tauri v2 desktop shell with Rust process supervision.
- React + Tailwind control deck.
- Stable bundle identity: `com.peachhi11.spicytaverncockpit`.
- Engine registry for Marinara clean, Marinara sandbox, SillyTavern, and Ollama.
- Persisted registry editor for engine paths, launch commands, ports, UI URLs, and health URLs.
- Start, stop, restart, stop-all, health check, and embedded localhost views.
- Managed-vs-external process detection for already-running local engines.
- Network diagnostics for default egress and Chub reachability.
- Per-engine log file locations and in-app log tailing.

## Local Development

```zsh
npm install
npm run build
cd src-tauri
cargo check
```

Run the browser-only UI preview:

```zsh
npm run dev
```

Run the desktop shell with Rust process control:

```zsh
npm run tauri:dev
```

## Architecture Lanes

- `Engine-Registry`: engine definitions, ports, paths, env, and per-profile runtime targets.
- `Process-Supervisor`: Rust child-process lifecycle, logs, stop-all, and restart behavior.
- `Local-WebView-Router`: embedded local app views and browser handoff behavior.
- `Health/Diagnostics-Panel`: health probes, network egress checks, Chub checks, and provider tests.
- `Profile-Config`: persisted user profiles, repo paths, custom ports, and stack presets.

## Design Rule

The cockpit supervises external engines. It does not fork, absorb, or replace SillyTavern or Marinara. Those projects remain their own checkouts; this app owns launch, routing, health, and visibility.

## Engine Registry

The desktop shell writes `engine-registry.json` into the app data directory on first boot. The Registry panel can edit each engine's local checkout path, launch command, port, embedded UI URL, and health URL, then the Rust supervisor uses those saved values for start, stop, and health checks.

Default engines:

- `marinara-clean`: `/Library/Developer/GitHub2.0/Marinara-Engine-upstream-clean`, port `7860`.
- `marinara-sandbox`: `/Library/Developer/GitHub2.0/SillyTavern/plugins/SillyTavern-EverythingPlugin/Untitled/Marinara-Engine`, port `7861`.
- `sillytavern`: `/Library/Developer/GitHub2.0/SillyTavern`, port `8000`.
- `ollama`: local `ollama serve`, port `11434`.

## Process Supervisor

The Rust supervisor launches engines through `/bin/zsh -lc` with a Homebrew-aware `PATH`, so registry commands can use `node`, `pnpm`, `./start.sh`, or `ollama serve`. Before launching, it checks the configured listener port and reports an external process instead of spawning a duplicate. Stop and restart only kill child processes that the cockpit launched itself.

The Logs panel tails the last chunk of each managed engine log from the app log directory. Engines started outside the cockpit remain visible through port/process detection, but their external logs are not claimed by this app.
