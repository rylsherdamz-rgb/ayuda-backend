use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{
    env,
    process::Command,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
use tower_http::cors::CorsLayer;

#[derive(Clone)]
struct AppState {
    latest_scan: Arc<Mutex<Option<NfcData>>>,
}

#[derive(Clone)]
struct NfcData {
    hash: String,
    timestamp: u64,
}

#[derive(Deserialize)]
struct ScanRequest {
    nfc_hash: String,
}

#[derive(Deserialize)]
struct RegisterRequest {
    citizen_addr: String,
    citizen_name: String,
    nfc_hash: Option<String>,
    amount: i128,
}

#[derive(Serialize)]
struct ApiResponse {
    status: &'static str,
    message: String,
    result: Option<String>,
}

#[derive(Serialize)]
struct ScanResponse {
    nfc_hash: Option<String>,
    is_fresh: bool,
}

#[tokio::main]
async fn main() {
    let state = AppState {
        latest_scan: Arc::new(Mutex::new(None)),
    };

    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/scan", post(store_scan))
        .route("/api/latest-scan", get(get_latest_scan))
        .route("/api/register", post(register_ayuda))
        .with_state(state)
        .layer(CorsLayer::permissive());

    let port = env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("0.0.0.0:{port}");

    println!("AYUDA PROTOCOL Backend Online at {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn health() -> Json<ApiResponse> {
    Json(ApiResponse {
        status: "success",
        message: "Ayuda Protocol Bridge Online".to_string(),
        result: Some("Hardware Handshake Ready".to_string()),
    })
}

async fn store_scan(
    State(state): State<AppState>,
    Json(payload): Json<ScanRequest>,
) -> Json<ApiResponse> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let mut latest_scan = state.latest_scan.lock().unwrap();

    *latest_scan = Some(NfcData {
        hash: payload.nfc_hash.clone(),
        timestamp: now,
    });

    println!("NFC SIGNAL RECEIVED: {}", payload.nfc_hash);

    Json(ApiResponse {
        status: "success",
        message: "NFC Handshake Captured".to_string(),
        result: None,
    })
}

async fn get_latest_scan(State(state): State<AppState>) -> Json<ScanResponse> {
    let scan_lock = state.latest_scan.lock().unwrap();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    match &*scan_lock {
        Some(data) => {
            let is_fresh = (now - data.timestamp) < 60;
            Json(ScanResponse {
                nfc_hash: Some(data.hash.clone()),
                is_fresh,
            })
        }
        None => Json(ScanResponse {
            nfc_hash: None,
            is_fresh: false,
        }),
    }
}

async fn register_ayuda(
    State(state): State<AppState>,
    Json(payload): Json<RegisterRequest>,
) -> Json<ApiResponse> {
    let nfc_hash = payload
        .nfc_hash
        .or_else(|| {
            state
                .latest_scan
                .lock()
                .unwrap()
                .as_ref()
                .map(|d| d.hash.clone())
        })
        .unwrap_or_default();

    if nfc_hash.is_empty() {
        return Json(ApiResponse {
            status: "error",
            message: "NFC scan required for biometric binding".to_string(),
            result: None,
        });
    }

    let reg_result = invoke_contract(&[
        "register_citizen",
        "--admin",
        &required_env("ADMIN_PUBLIC"),
        "--citizen_addr",
        &payload.citizen_addr,
        "--name",
        &payload.citizen_name,
    ]);

    match reg_result {
        Ok(_) => {
            let fund_result = invoke_contract(&[
                "fund_aid",
                "--admin",
                &required_env("ADMIN_PUBLIC"),
                "--citizen_addr",
                &payload.citizen_addr,
                "--amount",
                &payload.amount.to_string(),
            ]);

            match fund_result {
                Ok(stdout) => Json(ApiResponse {
                    status: "success",
                    message: "Identity Committed & Funded".to_string(),
                    result: Some(stdout),
                }),
                Err(e) => Json(ApiResponse {
                    status: "error",
                    message: e,
                    result: None,
                }),
            }
        }
        Err(e) => Json(ApiResponse {
            status: "error",
            message: e,
            result: None,
        }),
    }
}

fn invoke_contract(args: &[&str]) -> Result<String, String> {
    let contract_id = required_env("CONTRACT_ID");
    let admin_secret = required_env("ADMIN_SECRET");

    let mut command = Command::new("stellar");
    command.args([
        "contract",
        "invoke",
        "--id",
        &contract_id,
        "--source-account",
        &admin_secret,
        "--network",
        "testnet",
        "--",
    ]);
    command.args(args);

    let output = command.output().map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

fn required_env(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("{name} must be set"))
}
use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{
    env,
    process::Command,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
use tower_http::cors::CorsLayer;

#[derive(Clone)]
struct AppState {
    latest_scan: Arc<Mutex<Option<NfcData>>>,
}

#[derive(Clone)]
struct NfcData {
    hash: String,
    timestamp: u64,
}

#[derive(Deserialize)]
struct ScanRequest {
    nfc_hash: String,
}

#[derive(Deserialize)]
struct RegisterRequest {
    citizen_addr: String,
    citizen_name: String,
    nfc_hash: Option<String>,
    amount: i128,
}

#[derive(Serialize)]
struct ApiResponse {
    status: &'static str,
    message: String,
    result: Option<String>,
}

#[derive(Serialize)]
struct ScanResponse {
    nfc_hash: Option<String>,
    is_fresh: bool,
}

#[tokio::main]
async fn main() {
    let state = AppState {
        latest_scan: Arc::new(Mutex::new(None)),
    };

    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/scan", post(store_scan))
        .route("/api/latest-scan", get(get_latest_scan))
        .route("/api/register", post(register_ayuda))
        .with_state(state)
        .layer(CorsLayer::permissive());

    let port = env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("0.0.0.0:{port}");

    println!("AYUDA PROTOCOL Backend Online at {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn health() -> Json<ApiResponse> {
    Json(ApiResponse {
        status: "success",
        message: "Ayuda Protocol Bridge Online".to_string(),
        result: Some("Hardware Handshake Ready".to_string()),
    })
}

async fn store_scan(
    State(state): State<AppState>,
    Json(payload): Json<ScanRequest>,
) -> Json<ApiResponse> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let mut latest_scan = state.latest_scan.lock().unwrap();

    *latest_scan = Some(NfcData {
        hash: payload.nfc_hash.clone(),
        timestamp: now,
    });

    println!("NFC SIGNAL RECEIVED: {}", payload.nfc_hash);

    Json(ApiResponse {
        status: "success",
        message: "NFC Handshake Captured".to_string(),
        result: None,
    })
}

async fn get_latest_scan(State(state): State<AppState>) -> Json<ScanResponse> {
    let scan_lock = state.latest_scan.lock().unwrap();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    match &*scan_lock {
        Some(data) => {
            let is_fresh = (now - data.timestamp) < 60;
            Json(ScanResponse {
                nfc_hash: Some(data.hash.clone()),
                is_fresh,
            })
        }
        None => Json(ScanResponse {
            nfc_hash: None,
            is_fresh: false,
        }),
    }
}

async fn register_ayuda(
    State(state): State<AppState>,
    Json(payload): Json<RegisterRequest>,
) -> Json<ApiResponse> {
    let nfc_hash = payload
        .nfc_hash
        .or_else(|| {
            state
                .latest_scan
                .lock()
                .unwrap()
                .as_ref()
                .map(|d| d.hash.clone())
        })
        .unwrap_or_default();

    if nfc_hash.is_empty() {
        return Json(ApiResponse {
            status: "error",
            message: "NFC scan required for biometric binding".to_string(),
            result: None,
        });
    }

    let reg_result = invoke_contract(&[
        "register_citizen",
        "--admin",
        &required_env("ADMIN_PUBLIC"),
        "--citizen_addr",
        &payload.citizen_addr,
        "--name",
        &payload.citizen_name,
    ]);

    match reg_result {
        Ok(_) => {
            let fund_result = invoke_contract(&[
                "fund_aid",
                "--admin",
                &required_env("ADMIN_PUBLIC"),
                "--citizen_addr",
                &payload.citizen_addr,
                "--amount",
                &payload.amount.to_string(),
            ]);

            match fund_result {
                Ok(stdout) => Json(ApiResponse {
                    status: "success",
                    message: "Identity Committed & Funded".to_string(),
                    result: Some(stdout),
                }),
                Err(e) => Json(ApiResponse {
                    status: "error",
                    message: e,
                    result: None,
                }),
            }
        }
        Err(e) => Json(ApiResponse {
            status: "error",
            message: e,
            result: None,
        }),
    }
}

fn invoke_contract(args: &[&str]) -> Result<String, String> {
    let contract_id = required_env("CONTRACT_ID");
    let admin_secret = required_env("ADMIN_SECRET");

    let mut command = Command::new("stellar");
    command.args([
        "contract",
        "invoke",
        "--id",
        &contract_id,
        "--source-account",
        &admin_secret,
        "--network",
        "testnet",
        "--",
    ]);
    command.args(args);

    let output = command.output().map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

fn required_env(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("{name} must be set"))
}

