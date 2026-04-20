use axum::{
    extract::{Path, State},
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

#[derive(Serialize)]
struct StatusResponse {
    pool_remaining: i128,
    total_distributed: i128,
    logs: Vec<TransactionLog>,
}

#[derive(Serialize)]
struct TransactionLog {
    id: String,
    name: String,
    addr: String,
    amount: i128,
    status: String,
}

#[tokio::main]
async fn main() {
    let state = AppState {
        latest_scan: Arc::new(Mutex::new(None)),
    };

    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/scan", post(store_scan_json))
        .route("/api/scan/:hash", get(handle_path_scan)) // Capture from URL (GET)
        .route("/api/scan/:hash", post(handle_path_scan)) // Capture from URL (POST)
        .route("/api/latest-scan", get(get_latest_scan))
        .route("/api/register", post(register_citizen))
        .route("/api/status", get(get_protocol_status))
        .with_state(state)
        .layer(CorsLayer::permissive());

    let port = env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("0.0.0.0:{}", port);
    println!("--- AYUDA PROTOCOL BRIDGE ONLINE ---");
    println!("NODE_ADDR: {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn health() -> Json<ApiResponse> {
    Json(ApiResponse {
        status: "success",
        message: "Bridge Active".into(),
        result: None,
    })
}

async fn handle_path_scan(
    Path(hash): Path<String>,
    State(state): State<AppState>,
) -> Json<ApiResponse> {
    update_scan_state(state, hash.clone());

    Json(ApiResponse {
        status: "success",
        message: format!("HANDSHAKE_CAPTURED: {}", hash),
        result: None,
    })
}

async fn store_scan_json(
    State(state): State<AppState>,
    Json(payload): Json<ScanRequest>,
) -> Json<ApiResponse> {
    update_scan_state(state, payload.nfc_hash.clone());

    Json(ApiResponse {
        status: "success",
        message: "NFC_HANDSHAKE_CAPTURED".into(),
        result: None,
    })
}

fn update_scan_state(state: AppState, hash: String) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let mut scan = state.latest_scan.lock().unwrap();
    *scan = Some(NfcData {
        hash,
        timestamp: now,
    });
}

async fn get_latest_scan(State(state): State<AppState>) -> Json<ScanResponse> {
    let scan = state.latest_scan.lock().unwrap();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    match &*scan {
        Some(data) => Json(ScanResponse {
            nfc_hash: Some(data.hash.clone()),
            is_fresh: (now - data.timestamp) < 60,
        }),
        None => Json(ScanResponse {
            nfc_hash: None,
            is_fresh: false,
        }),
    }
}

async fn register_citizen(
    State(state): State<AppState>,
    Json(p): Json<RegisterRequest>,
) -> Json<ApiResponse> {
    let nfc_check = state.latest_scan.lock().unwrap();
    if nfc_check.is_none() {
        return Json(ApiResponse {
            status: "error",
            message: "NO_HARDWARE_SIGNAL".into(),
            result: None,
        });
    }

    let contract_id = env::var("CONTRACT_ID").expect("CONTRACT_ID_NOT_SET");
    let admin_key = env::var("ADMIN_SECRET").expect("ADMIN_KEY_NOT_SET");

    let output = Command::new("stellar")
        .args([
            "contract",
            "invoke",
            "--id",
            &contract_id,
            "--source-account",
            &admin_key,
            "--network",
            "testnet",
            "--",
            "register_citizen",
            "--citizen_addr",
            &p.citizen_addr,
            "--name",
            &p.citizen_name,
        ])
        .output();

    match output {
        Ok(out) if out.status.success() => Json(ApiResponse {
            status: "success",
            message: "IDENTITY_COMMITTED_TO_LEDGER".into(),
            result: Some(String::from_utf8_lossy(&out.stdout).into()),
        }),
        _ => Json(ApiResponse {
            status: "error",
            message: "BLOCKCHAIN_INVOCATION_FAILED".into(),
            result: None,
        }),
    }
}

async fn get_protocol_status() -> Json<StatusResponse> {
    let contract_id = env::var("CONTRACT_ID").unwrap_or_default();

    let pool_val = Command::new("stellar")
        .args([
            "contract",
            "invoke",
            "--id",
            &contract_id,
            "--network",
            "testnet",
            "--",
            "get_pool_balance",
        ])
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .trim()
                .parse::<i128>()
                .unwrap_or(0)
        })
        .unwrap_or(0);

    let logs = vec![TransactionLog {
        id: "TX_LATEST".into(),
        name: "SYSTEM_GEN".into(),
        addr: "ST_NETWORK".into(),
        amount: 50,
        status: "SYNCED".into(),
    }];

    Json(StatusResponse {
        pool_remaining: pool_val,
        total_distributed: 1250000 - pool_val,
        logs,
    })
}
