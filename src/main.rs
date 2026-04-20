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
struct RegisterRequest {
    citizen_addr: String,
    citizen_name: String,
    amount: i128,
}

#[derive(Deserialize)]
struct ClaimRequest {
    beneficiary_addr: String,
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
        .route("/api/latest-scan", get(get_latest_scan))
        // NFC Hardware Entry Points
        .route("/api/scan/{hash}", get(handle_path_scan))
        .route("/api/scan/{hash}", post(handle_path_scan))
        // Admin Operations
        .route("/api/register", post(register_citizen))
        // Beneficiary Operations
        .route("/api/claim", post(claim_aid))
        .with_state(state)
        .layer(CorsLayer::permissive());

    let port = env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("0.0.0.0:{}", port);

    println!("--- AYUDA PROTOCOL BRIDGE ONLINE ---");
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
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let mut scan = state.latest_scan.lock().unwrap();
    *scan = Some(NfcData {
        hash: hash.clone(),
        timestamp: now,
    });

    Json(ApiResponse {
        status: "success",
        message: format!("HANDSHAKE_CAPTURED: {}", hash),
        result: None,
    })
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
    let hash_val: String;

    {
        let mut nfc_check = state.latest_scan.lock().unwrap();
        if nfc_check.is_none() {
            return Json(ApiResponse {
                status: "error",
                message: "NO_HARDWARE_SIGNAL".into(),
                result: None,
            });
        }
        hash_val = nfc_check.as_ref().unwrap().hash.clone();
        *nfc_check = None; // Clear scan after use
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
            "--admin",
            &admin_key, // The contract requires admin Address
            "--citizen_addr",
            &p.citizen_addr,
            "--nfc_id",
            &hash_val,
            "--name",
            &p.citizen_name,
        ])
        .output();

    match output {
        Ok(out) if out.status.success() => Json(ApiResponse {
            status: "success",
            message: "IDENTITY_COMMITTED".into(),
            result: Some(String::from_utf8_lossy(&out.stdout).into()),
        }),
        _ => Json(ApiResponse {
            status: "error",
            message: "BLOCKCHAIN_REGISTRATION_FAILED".into(),
            result: None,
        }),
    }
}

async fn claim_aid(
    State(state): State<AppState>,
    Json(p): Json<ClaimRequest>,
) -> Json<ApiResponse> {
    let hash_val: String;

    {
        let mut nfc_check = state.latest_scan.lock().unwrap();
        if nfc_check.is_none() {
            return Json(ApiResponse {
                status: "error",
                message: "PHYSICAL_CARD_REQUIRED".into(),
                result: None,
            });
        }
        hash_val = nfc_check.as_ref().unwrap().hash.clone();
    }

    let contract_id = env::var("CONTRACT_ID").expect("CONTRACT_ID_NOT_SET");

    let output = Command::new("stellar")
        .args([
            "contract",
            "invoke",
            "--id",
            &contract_id,
            "--network",
            "testnet",
            "--",
            "claim_aid",
            "--citizen_addr",
            &p.beneficiary_addr,
            "--nfc_id",
            &hash_val,
        ])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let mut nfc_check = state.latest_scan.lock().unwrap();
            *nfc_check = None;
            Json(ApiResponse {
                status: "success",
                message: "CLAIM_DISBURSED".into(),
                result: Some(String::from_utf8_lossy(&out.stdout).into()),
            })
        }
        _ => Json(ApiResponse {
            status: "error",
            message: "CLAIM_DENIED_VERIFICATION_FAILED".into(),
            result: None,
        }),
    }
}

