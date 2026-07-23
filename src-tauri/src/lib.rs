use chrono::Utc;
use reqwest::header::HeaderMap;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::{create_dir_all, read_to_string, File},
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::Mutex,
    thread::sleep,
    time::Duration,
};
use tauri::{Manager, State};

const REGISTRY_FILE_NAME: &str = "engine-registry.json";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct EngineConfig {
    id: String,
    name: String,
    description: String,
    cwd: String,
    command: String,
    port: Option<u16>,
    ui_url: Option<String>,
    health_url: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct EngineRegistryFile {
    version: u16,
    updated_at: String,
    engines: Vec<EngineConfig>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EngineStatus {
    id: String,
    name: String,
    description: String,
    cwd: String,
    command: String,
    port: Option<u16>,
    ui_url: Option<String>,
    health_url: Option<String>,
    log_path: Option<String>,
    state: String,
    pid: Option<u32>,
    process_source: String,
    process_message: String,
    port_listening: bool,
    health_ok: bool,
    health_message: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EngineLogTail {
    path: Option<String>,
    content: String,
    line_count: usize,
    message: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticCheck {
    id: String,
    category: String,
    label: String,
    status: String,
    message: String,
    detail: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticsSnapshot {
    generated_at: String,
    checks: Vec<DiagnosticCheck>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NetworkSnapshot {
    public_ip: String,
    city: String,
    region: String,
    country: String,
    org: String,
    chub_status: Option<u16>,
    chub_country: String,
    chub_region: String,
    chub_ok: bool,
    message: String,
}

#[derive(Default)]
struct ProcessRegistry {
    children: Mutex<HashMap<String, Child>>,
    logs: Mutex<HashMap<String, PathBuf>>,
}

#[derive(Clone, Debug)]
struct PortSnapshot {
    listening: bool,
    pid: Option<u32>,
    message: String,
}

fn default_engines() -> Vec<EngineConfig> {
    vec![
        EngineConfig {
            id: "marinara-clean".into(),
            name: "Marinara Clean".into(),
            description: "Upstream-clean Marinara Engine checkout.".into(),
            cwd: "/Library/Developer/GitHub2.0/Marinara-Engine-upstream-clean".into(),
            command: "export PATH=/opt/homebrew/Cellar/node@24/24.18.0/bin:$PATH; export PORT=7860; export NODE_ENV=production; unset CHUB_OUTBOUND_PROXY; node packages/server/dist/index.js".into(),
            port: Some(7860),
            ui_url: Some("http://127.0.0.1:7860".into()),
            health_url: Some("http://127.0.0.1:7860/api/health".into()),
        },
        EngineConfig {
            id: "marinara-sandbox".into(),
            name: "Marinara Sandbox".into(),
            description: "Experimental Marinara/HumanOS checkout.".into(),
            cwd: "/Library/Developer/GitHub2.0/SillyTavern/plugins/SillyTavern-EverythingPlugin/Untitled/Marinara-Engine".into(),
            command: "export PATH=/opt/homebrew/Cellar/node@24/24.18.0/bin:$PATH; export PORT=7861; export NODE_ENV=production; unset CHUB_OUTBOUND_PROXY; node packages/server/dist/index.js".into(),
            port: Some(7861),
            ui_url: Some("http://127.0.0.1:7861".into()),
            health_url: Some("http://127.0.0.1:7861/api/health".into()),
        },
        EngineConfig {
            id: "sillytavern".into(),
            name: "SillyTavern".into(),
            description: "Local SillyTavern web engine.".into(),
            cwd: "/Library/Developer/GitHub2.0/SillyTavern".into(),
            command: "node server.js".into(),
            port: Some(8000),
            ui_url: Some("http://127.0.0.1:8000".into()),
            health_url: Some("http://127.0.0.1:8000".into()),
        },
        EngineConfig {
            id: "ollama".into(),
            name: "Ollama".into(),
            description: "Local model runtime.".into(),
            cwd: "/".into(),
            command: "ollama serve".into(),
            port: Some(11434),
            ui_url: None,
            health_url: Some("http://127.0.0.1:11434/api/tags".into()),
        },
    ]
}

fn registry_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let app_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Could not resolve app data directory: {error}"))?;
    create_dir_all(&app_dir)
        .map_err(|error| format!("Could not create app data directory: {error}"))?;
    Ok(app_dir.join(REGISTRY_FILE_NAME))
}

fn registry_file_for(engines: Vec<EngineConfig>) -> EngineRegistryFile {
    EngineRegistryFile {
        version: 1,
        updated_at: Utc::now().to_rfc3339(),
        engines,
    }
}

fn write_registry(app: &tauri::AppHandle, engines: Vec<EngineConfig>) -> Result<(), String> {
    let path = registry_path(app)?;
    let registry = registry_file_for(engines);
    let payload = serde_json::to_string_pretty(&registry)
        .map_err(|error| format!("Could not serialize engine registry: {error}"))?;
    std::fs::write(&path, payload)
        .map_err(|error| format!("Could not write engine registry: {error}"))
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|inner| {
        let trimmed = inner.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn normalize_engine(mut engine: EngineConfig) -> EngineConfig {
    engine.id = engine.id.trim().to_string();
    engine.name = engine.name.trim().to_string();
    engine.description = engine.description.trim().to_string();
    engine.cwd = engine.cwd.trim().to_string();
    engine.command = engine.command.trim().to_string();
    engine.ui_url = normalize_optional(engine.ui_url);
    engine.health_url = normalize_optional(engine.health_url);
    engine
}

fn validate_engine(engine: &EngineConfig) -> Result<(), String> {
    if engine.id.is_empty() {
        return Err("Engine id is required.".into());
    }
    if !engine.id.chars().all(|character| {
        character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
    }) {
        return Err("Engine id can only use lowercase letters, numbers, and hyphens.".into());
    }
    if engine.name.is_empty() {
        return Err("Engine name is required.".into());
    }
    if engine.cwd.is_empty() {
        return Err("Engine path is required.".into());
    }
    if engine.command.is_empty() {
        return Err("Launch command is required.".into());
    }
    Ok(())
}

fn load_registry(app: &tauri::AppHandle) -> Result<Vec<EngineConfig>, String> {
    let path = registry_path(app)?;
    if !path.exists() {
        let engines = default_engines();
        write_registry(app, engines.clone())?;
        return Ok(engines);
    }

    let content = read_to_string(&path)
        .map_err(|error| format!("Could not read engine registry: {error}"))?;
    let registry: EngineRegistryFile = serde_json::from_str(&content)
        .map_err(|error| format!("Could not parse engine registry: {error}"))?;

    let mut engines = Vec::new();
    for engine in registry.engines {
        let normalized = normalize_engine(engine);
        validate_engine(&normalized)?;
        engines.push(normalized);
    }
    Ok(engines)
}

fn engine_by_id(app: &tauri::AppHandle, id: &str) -> Result<EngineConfig, String> {
    load_registry(app)?
        .into_iter()
        .find(|engine| engine.id == id)
        .ok_or_else(|| format!("Unknown engine: {id}"))
}

fn first_lsof_pid(port: u16) -> Option<u32> {
    let tcp_filter = format!("-iTCP:{port}");
    let output = Command::new("lsof")
        .args(["-nP", tcp_filter.as_str(), "-sTCP:LISTEN", "-t"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.trim().parse::<u32>().ok())
}

fn port_snapshot(port: Option<u16>) -> PortSnapshot {
    let Some(port) = port else {
        return PortSnapshot {
            listening: false,
            pid: None,
            message: "No listener port configured.".into(),
        };
    };

    if let Some(pid) = first_lsof_pid(port) {
        return PortSnapshot {
            listening: true,
            pid: Some(pid),
            message: format!("Port {port} is listening on pid {pid}."),
        };
    }

    let address: SocketAddr = match format!("127.0.0.1:{port}").parse() {
        Ok(address) => address,
        Err(_) => {
            return PortSnapshot {
                listening: false,
                pid: None,
                message: format!("Port {port} is not a valid local socket."),
            };
        }
    };

    let listening = TcpStream::connect_timeout(&address, Duration::from_millis(250)).is_ok();
    PortSnapshot {
        listening,
        pid: None,
        message: if listening {
            format!("Port {port} accepted a local connection.")
        } else {
            format!("Port {port} is free.")
        },
    }
}

fn diagnostic_check(
    id: impl Into<String>,
    category: impl Into<String>,
    label: impl Into<String>,
    status: impl Into<String>,
    message: impl Into<String>,
    detail: impl Into<String>,
) -> DiagnosticCheck {
    DiagnosticCheck {
        id: id.into(),
        category: category.into(),
        label: label.into(),
        status: status.into(),
        message: message.into(),
        detail: detail.into(),
    }
}

fn shell_output(command: &str) -> Result<String, String> {
    let output = Command::new("/bin/zsh")
        .arg("-lc")
        .arg(format!(
            "export PATH=/opt/homebrew/bin:/usr/local/bin:/opt/homebrew/Cellar/node@24/24.18.0/bin:$PATH; {command}"
        ))
        .output()
        .map_err(|error| error.to_string())?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if output.status.success() {
        Ok(stdout)
    } else if stderr.is_empty() {
        Err(stdout)
    } else {
        Err(stderr)
    }
}

fn tool_check(id: &str, label: &str, command: &str) -> DiagnosticCheck {
    match shell_output(command) {
        Ok(output) if !output.is_empty() => {
            diagnostic_check(id, "Toolchain", label, "ok", output, command)
        }
        Ok(_) => diagnostic_check(
            id,
            "Toolchain",
            label,
            "warn",
            "Command returned no output.",
            command,
        ),
        Err(error) => diagnostic_check(id, "Toolchain", label, "fail", error, command),
    }
}

async fn marinara_chub_diagnostic(
    client: &reqwest::Client,
    engine: &EngineConfig,
) -> DiagnosticCheck {
    let Some(health_url) = engine.health_url.as_deref() else {
        return diagnostic_check(
            format!("{}:chub", engine.id),
            "Marinara Chub",
            format!("{} Chub egress", engine.name),
            "warn",
            "No health URL is configured.",
            "",
        );
    };

    let base = health_url
        .trim_end_matches("/api/health")
        .trim_end_matches('/');
    let url = format!("{base}/api/bot-browser/chub/egress-debug");
    match client.get(&url).send().await {
        Ok(response) => {
            let status = response.status();
            let detail = response.text().await.unwrap_or_default();
            diagnostic_check(
                format!("{}:chub", engine.id),
                "Marinara Chub",
                format!("{} Chub egress", engine.name),
                if status.is_success() { "ok" } else { "fail" },
                format!("HTTP {status}"),
                detail.chars().take(800).collect::<String>(),
            )
        }
        Err(error) => diagnostic_check(
            format!("{}:chub", engine.id),
            "Marinara Chub",
            format!("{} Chub egress", engine.name),
            "fail",
            error.to_string(),
            url,
        ),
    }
}

fn log_path(app: &tauri::AppHandle, engine_id: &str) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_log_dir()
        .map_err(|error| format!("Could not resolve log directory: {error}"))?
        .join("engines");
    create_dir_all(&dir).map_err(|error| format!("Could not create log directory: {error}"))?;
    Ok(dir.join(format!("{engine_id}.log")))
}

fn status_from_parts(
    engine: &EngineConfig,
    registry: &ProcessRegistry,
    health_ok: bool,
    health_message: String,
) -> EngineStatus {
    let managed_pid = {
        let mut children = registry.children.lock().expect("process registry poisoned");
        if let Some(child) = children.get_mut(&engine.id) {
            match child.try_wait() {
                Ok(Some(_)) => {
                    children.remove(&engine.id);
                    None
                }
                Ok(None) => Some(child.id()),
                Err(_) => Some(child.id()),
            }
        } else {
            None
        }
    };

    let port = port_snapshot(engine.port);
    let (pid, state, process_source, process_message) = if let Some(pid) = managed_pid {
        (
            Some(pid),
            "running".to_string(),
            "managed".to_string(),
            format!("Managed process pid {pid}."),
        )
    } else if port.listening {
        (
            port.pid,
            "running".to_string(),
            "external".to_string(),
            port.message.clone(),
        )
    } else {
        (
            None,
            "stopped".to_string(),
            "none".to_string(),
            port.message.clone(),
        )
    };

    let log_path = registry
        .logs
        .lock()
        .expect("log registry poisoned")
        .get(&engine.id)
        .map(|path| path.to_string_lossy().to_string());

    EngineStatus {
        id: engine.id.clone(),
        name: engine.name.clone(),
        description: engine.description.clone(),
        cwd: engine.cwd.clone(),
        command: engine.command.clone(),
        port: engine.port,
        ui_url: engine.ui_url.clone(),
        health_url: engine.health_url.clone(),
        log_path,
        state,
        pid,
        process_source,
        process_message,
        port_listening: port.listening,
        health_ok,
        health_message,
    }
}

async fn health_for(engine: &EngineConfig) -> (bool, String) {
    let Some(url) = engine.health_url.as_deref() else {
        return (false, "No health endpoint".into());
    };
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(4))
        .build()
    {
        Ok(client) => client,
        Err(error) => return (false, format!("HTTP client error: {error}")),
    };
    match client.get(url).send().await {
        Ok(response) if response.status().is_success() => {
            (true, format!("HTTP {}", response.status()))
        }
        Ok(response) => (false, format!("HTTP {}", response.status())),
        Err(error) => (false, error.to_string()),
    }
}

async fn build_status(engine: &EngineConfig, registry: &ProcessRegistry) -> EngineStatus {
    let (health_ok, health_message) = health_for(engine).await;
    status_from_parts(engine, registry, health_ok, health_message)
}

#[tauri::command]
async fn list_engines(
    app: tauri::AppHandle,
    registry: State<'_, ProcessRegistry>,
) -> Result<Vec<EngineStatus>, String> {
    let mut statuses = Vec::new();
    for engine in load_registry(&app)? {
        statuses.push(build_status(&engine, &registry).await);
    }
    Ok(statuses)
}

#[tauri::command]
async fn engine_status(
    id: String,
    app: tauri::AppHandle,
    registry: State<'_, ProcessRegistry>,
) -> Result<EngineStatus, String> {
    let engine = engine_by_id(&app, &id)?;
    Ok(build_status(&engine, &registry).await)
}

#[tauri::command]
async fn start_engine(
    id: String,
    app: tauri::AppHandle,
    registry: State<'_, ProcessRegistry>,
) -> Result<EngineStatus, String> {
    let engine = engine_by_id(&app, &id)?;
    if !Path::new(&engine.cwd).exists() {
        return Err(format!("Engine path does not exist: {}", engine.cwd));
    }

    let already_running = {
        let mut children = registry.children.lock().expect("process registry poisoned");
        if let Some(child) = children.get_mut(&engine.id) {
            if child
                .try_wait()
                .map_err(|error| error.to_string())?
                .is_none()
            {
                true
            } else {
                children.remove(&engine.id);
                false
            }
        } else {
            false
        }
    };
    if already_running {
        return Ok(build_status(&engine, &registry).await);
    }

    let port = port_snapshot(engine.port);
    if port.listening {
        return Ok(build_status(&engine, &registry).await);
    }

    let path = log_path(&app, &engine.id)?;
    let stdout = File::options()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("Could not open log file: {error}"))?;
    let stderr = stdout
        .try_clone()
        .map_err(|error| format!("Could not clone log file handle: {error}"))?;

    let child = Command::new("/bin/zsh")
        .arg("-lc")
        .arg(format!(
            "export PATH=/opt/homebrew/bin:/usr/local/bin:/opt/homebrew/Cellar/node@24/24.18.0/bin:$PATH; printf '\\n[%s] starting {}\\n' \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\"; {}",
            engine.id, engine.command
        ))
        .current_dir(&engine.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .map_err(|error| format!("Could not start {}: {error}", engine.name))?;

    registry
        .logs
        .lock()
        .expect("log registry poisoned")
        .insert(engine.id.clone(), path);
    registry
        .children
        .lock()
        .expect("process registry poisoned")
        .insert(engine.id.clone(), child);

    Ok(build_status(&engine, &registry).await)
}

fn stop_managed_child(engine_id: &str, registry: &ProcessRegistry) -> bool {
    if let Some(mut child) = registry
        .children
        .lock()
        .expect("process registry poisoned")
        .remove(engine_id)
    {
        let _ = child.kill();
        let _ = child.wait();
        return true;
    }
    false
}

fn tail_lines(content: &str, line_count: usize) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.len().saturating_sub(line_count.max(1));
    lines[start..].join("\n")
}

#[tauri::command]
fn engine_log_tail(
    id: String,
    line_count: Option<usize>,
    app: tauri::AppHandle,
    registry: State<'_, ProcessRegistry>,
) -> Result<EngineLogTail, String> {
    let engine = engine_by_id(&app, &id)?;
    let count = line_count.unwrap_or(160);
    let path = registry
        .logs
        .lock()
        .expect("log registry poisoned")
        .get(&engine.id)
        .cloned()
        .unwrap_or(log_path(&app, &engine.id)?);
    let path_text = path.to_string_lossy().to_string();

    if !path.exists() {
        return Ok(EngineLogTail {
            path: Some(path_text),
            content: String::new(),
            line_count: 0,
            message: "No log file exists yet for this engine.".into(),
        });
    }

    let content =
        read_to_string(&path).map_err(|error| format!("Could not read log file: {error}"))?;
    let tail = tail_lines(&content, count);
    let returned = tail.lines().count();

    Ok(EngineLogTail {
        path: Some(path_text),
        content: tail,
        line_count: returned,
        message: if returned == 0 {
            "Log file is empty.".into()
        } else {
            format!("Showing last {returned} log lines.")
        },
    })
}

#[tauri::command]
async fn stop_engine(
    id: String,
    app: tauri::AppHandle,
    registry: State<'_, ProcessRegistry>,
) -> Result<EngineStatus, String> {
    let engine = engine_by_id(&app, &id)?;
    stop_managed_child(&engine.id, &registry);
    Ok(build_status(&engine, &registry).await)
}

#[tauri::command]
async fn restart_engine(
    id: String,
    app: tauri::AppHandle,
    registry: State<'_, ProcessRegistry>,
) -> Result<EngineStatus, String> {
    let engine = engine_by_id(&app, &id)?;
    let stopped_managed_process = stop_managed_child(&engine.id, &registry);

    if !stopped_managed_process {
        let port = port_snapshot(engine.port);
        if port.listening {
            return Err(format!(
                "{} is already running outside this cockpit. {} Stop that process first before restarting here.",
                engine.name, port.message
            ));
        }
    }

    sleep(Duration::from_millis(350));
    start_engine(id, app, registry).await
}

#[tauri::command]
async fn stop_all_engines(
    app: tauri::AppHandle,
    registry: State<'_, ProcessRegistry>,
) -> Result<Vec<EngineStatus>, String> {
    let children: Vec<Child> = registry
        .children
        .lock()
        .expect("process registry poisoned")
        .drain()
        .map(|(_, child)| child)
        .collect();

    for mut child in children {
        let _ = child.kill();
        let _ = child.wait();
    }

    list_engines(app, registry).await
}

#[tauri::command]
async fn save_engine_config(
    engine: EngineConfig,
    app: tauri::AppHandle,
    registry: State<'_, ProcessRegistry>,
) -> Result<EngineStatus, String> {
    let engine = normalize_engine(engine);
    validate_engine(&engine)?;

    let mut engines = load_registry(&app)?;
    let index = engines
        .iter()
        .position(|candidate| candidate.id == engine.id)
        .ok_or_else(|| format!("Unknown engine: {}", engine.id))?;
    engines[index] = engine.clone();
    write_registry(&app, engines)?;

    Ok(build_status(&engine, &registry).await)
}

#[tauri::command]
async fn reset_engine_registry(
    app: tauri::AppHandle,
    registry: State<'_, ProcessRegistry>,
) -> Result<Vec<EngineStatus>, String> {
    let engines = default_engines();
    write_registry(&app, engines.clone())?;

    let mut statuses = Vec::new();
    for engine in engines {
        statuses.push(build_status(&engine, &registry).await);
    }
    Ok(statuses)
}

#[tauri::command]
async fn diagnostics_snapshot(
    app: tauri::AppHandle,
    registry: State<'_, ProcessRegistry>,
) -> Result<DiagnosticsSnapshot, String> {
    let engines = load_registry(&app)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(4))
        .build()
        .map_err(|error| error.to_string())?;

    let mut checks = Vec::new();
    for engine in &engines {
        let path_exists = Path::new(&engine.cwd).exists();
        checks.push(diagnostic_check(
            format!("{}:path", engine.id),
            "Engine Paths",
            format!("{} path", engine.name),
            if path_exists { "ok" } else { "fail" },
            if path_exists {
                "Path exists.".to_string()
            } else {
                "Path does not exist.".to_string()
            },
            engine.cwd.clone(),
        ));

        let status = status_from_parts(engine, &registry, false, String::new());
        checks.push(diagnostic_check(
            format!("{}:process", engine.id),
            "Processes",
            format!("{} process", engine.name),
            match status.process_source.as_str() {
                "managed" => "ok",
                "external" => "warn",
                _ => "warn",
            },
            status.process_message,
            status
                .pid
                .map(|pid| format!("pid {pid}"))
                .unwrap_or_else(|| "No pid".into()),
        ));

        let port = port_snapshot(engine.port);
        checks.push(diagnostic_check(
            format!("{}:port", engine.id),
            "Ports",
            format!("{} port", engine.name),
            if port.listening { "ok" } else { "warn" },
            port.message,
            engine
                .port
                .map(|port| format!("127.0.0.1:{port}"))
                .unwrap_or_else(|| "No port configured".into()),
        ));

        let (health_ok, health_message) = health_for(engine).await;
        checks.push(diagnostic_check(
            format!("{}:health", engine.id),
            "Health Endpoints",
            format!("{} health", engine.name),
            if health_ok { "ok" } else { "fail" },
            health_message,
            engine.health_url.clone().unwrap_or_default(),
        ));

        if engine.id.starts_with("marinara") {
            checks.push(marinara_chub_diagnostic(&client, engine).await);
        }
    }

    checks.push(tool_check(
        "toolchain:node",
        "Node",
        "command -v node && node --version",
    ));
    checks.push(tool_check(
        "toolchain:pnpm",
        "pnpm",
        "command -v pnpm && pnpm --version",
    ));
    checks.push(tool_check(
        "toolchain:ollama",
        "Ollama",
        "command -v ollama && ollama --version",
    ));

    match network_snapshot().await {
        Ok(snapshot) => {
            checks.push(diagnostic_check(
                "network:egress",
                "Network",
                "Default egress",
                if snapshot.country == "US" {
                    "ok"
                } else {
                    "warn"
                },
                snapshot.message,
                format!(
                    "{} {} {} {}",
                    snapshot.public_ip, snapshot.city, snapshot.region, snapshot.org
                ),
            ));
            checks.push(diagnostic_check(
                "network:chub",
                "Network",
                "Chub API reachability",
                if snapshot.chub_ok { "ok" } else { "fail" },
                snapshot
                    .chub_status
                    .map(|status| format!("HTTP {status}"))
                    .unwrap_or_else(|| "No HTTP status".into()),
                format!("{} {}", snapshot.chub_country, snapshot.chub_region),
            ));
        }
        Err(error) => checks.push(diagnostic_check(
            "network:egress",
            "Network",
            "Default egress",
            "fail",
            error,
            "",
        )),
    }

    Ok(DiagnosticsSnapshot {
        generated_at: Utc::now().to_rfc3339(),
        checks,
    })
}

#[derive(Deserialize)]
struct IpInfo {
    ip: Option<String>,
    city: Option<String>,
    region: Option<String>,
    country: Option<String>,
    org: Option<String>,
}

#[tauri::command]
async fn network_snapshot() -> Result<NetworkSnapshot, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|error| error.to_string())?;

    let ip_info = client
        .get("https://ipinfo.io/json")
        .send()
        .await
        .map_err(|error| format!("IP check failed: {error}"))?
        .json::<IpInfo>()
        .await
        .map_err(|error| format!("IP check parse failed: {error}"))?;

    let mut chub_status = None;
    let mut chub_country = String::new();
    let mut chub_region = String::new();
    let mut chub_ok = false;

    if let Ok(response) = client
        .get("https://api.chub.ai/search?search=char&first=1&page=1&nsfw=true")
        .send()
        .await
    {
        chub_status = Some(response.status().as_u16());
        chub_ok = response.status().is_success();
        let headers: &HeaderMap = response.headers();
        chub_country = headers
            .get("x-src-country")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_string();
        chub_region = headers
            .get("x-src-region")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_string();
    }

    let country = ip_info.country.unwrap_or_default();
    let public_ip = ip_info.ip.unwrap_or_default();
    let message = if country == "US" {
        format!("Default cockpit egress is US at {public_ip}.")
    } else if country.is_empty() {
        "Could not determine default cockpit egress.".to_string()
    } else {
        format!("Default cockpit egress is {country} at {public_ip}.")
    };

    Ok(NetworkSnapshot {
        public_ip,
        city: ip_info.city.unwrap_or_default(),
        region: ip_info.region.unwrap_or_default(),
        country,
        org: ip_info.org.unwrap_or_default(),
        chub_status,
        chub_country,
        chub_region,
        chub_ok,
        message,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(ProcessRegistry::default())
        .plugin(tauri_plugin_store::Builder::default().build())
        .setup(|app| {
            let app_dir = app.path().app_data_dir()?;
            create_dir_all(&app_dir)?;
            let handle = app.handle().clone();
            if let Err(error) = load_registry(&handle) {
                log::warn!("Engine registry could not be initialized: {error}");
            }
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            log::info!("SpicyTavernCockPit booted at {}", Utc::now());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_engines,
            engine_status,
            start_engine,
            stop_engine,
            stop_all_engines,
            save_engine_config,
            reset_engine_registry,
            restart_engine,
            engine_log_tail,
            diagnostics_snapshot,
            network_snapshot,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Tauri application");
}
