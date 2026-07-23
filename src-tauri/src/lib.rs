use chrono::Utc;
use reqwest::header::HeaderMap;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::{create_dir_all, File},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::Mutex,
    time::Duration,
};
use tauri::{Manager, State};

#[derive(Clone, Debug, Serialize)]
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
    health_ok: bool,
    health_message: String,
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

fn engine_by_id(id: &str) -> Result<EngineConfig, String> {
    default_engines()
        .into_iter()
        .find(|engine| engine.id == id)
        .ok_or_else(|| format!("Unknown engine: {id}"))
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
    let (pid, state) = {
        let mut children = registry.children.lock().expect("process registry poisoned");
        if let Some(child) = children.get_mut(&engine.id) {
            match child.try_wait() {
                Ok(Some(_)) => {
                    children.remove(&engine.id);
                    (None, "stopped".to_string())
                }
                Ok(None) => (Some(child.id()), "running".to_string()),
                Err(_) => (Some(child.id()), "unknown".to_string()),
            }
        } else {
            (None, "stopped".to_string())
        }
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
        Ok(response) if response.status().is_success() => (true, format!("HTTP {}", response.status())),
        Ok(response) => (false, format!("HTTP {}", response.status())),
        Err(error) => (false, error.to_string()),
    }
}

async fn build_status(engine: &EngineConfig, registry: &ProcessRegistry) -> EngineStatus {
    let (health_ok, health_message) = health_for(engine).await;
    status_from_parts(engine, registry, health_ok, health_message)
}

#[tauri::command]
async fn list_engines(registry: State<'_, ProcessRegistry>) -> Result<Vec<EngineStatus>, String> {
    let mut statuses = Vec::new();
    for engine in default_engines() {
        statuses.push(build_status(&engine, &registry).await);
    }
    Ok(statuses)
}

#[tauri::command]
async fn engine_status(id: String, registry: State<'_, ProcessRegistry>) -> Result<EngineStatus, String> {
    let engine = engine_by_id(&id)?;
    Ok(build_status(&engine, &registry).await)
}

#[tauri::command]
async fn start_engine(
    id: String,
    app: tauri::AppHandle,
    registry: State<'_, ProcessRegistry>,
) -> Result<EngineStatus, String> {
    let engine = engine_by_id(&id)?;
    if !Path::new(&engine.cwd).exists() {
        return Err(format!("Engine path does not exist: {}", engine.cwd));
    }

    let already_running = {
        let mut children = registry.children.lock().expect("process registry poisoned");
        if let Some(child) = children.get_mut(&engine.id) {
            if child.try_wait().map_err(|error| error.to_string())?.is_none() {
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
            "printf '\\n[%s] starting {}\\n' \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\"; {}",
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

#[tauri::command]
async fn stop_engine(id: String, registry: State<'_, ProcessRegistry>) -> Result<EngineStatus, String> {
    let engine = engine_by_id(&id)?;
    if let Some(mut child) = registry
        .children
        .lock()
        .expect("process registry poisoned")
        .remove(&engine.id)
    {
        let _ = child.kill();
        let _ = child.wait();
    }
    Ok(build_status(&engine, &registry).await)
}

#[tauri::command]
async fn stop_all_engines(registry: State<'_, ProcessRegistry>) -> Result<Vec<EngineStatus>, String> {
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

    list_engines(registry).await
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
            network_snapshot,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Tauri application");
}
