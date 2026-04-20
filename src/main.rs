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

#[derive(Clone, Debug)]
struct NfcData {
    hash: String,
    timestamp: u64,
}

#[derive(Deserialize)]
struct RegisterRequest {
    citizen_addr: String,
    citizen_name: String,
}

#[derive(Deserialize)]
struct ClaimRequest {
    beneficiary_addr: String,
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
        .route("/api/health", get(|| async { "Ayuda Bridge Online" }))
        // FIXED: Axum 0.8 uses {hash} syntax
        .route("/api/scan/{hash}", get(handle_incoming_scan))
        .route("/api/scan/{hash}", post(handle_incoming_scan))
        .route("/api/latest-scan", get(get_latest_scan))
        .route("/api/register", post(register_citizen))
        .route("/api/claim", post(claim_aid))
        .with_state(state)
        .layer(CorsLayer::permissive());

    let port = env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("0.0.0.0:{}", port);

    println!("🚀 [SERVER] Ayuda Protocol Bridge starting on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// 1. THIS CAPTURES THE SCAN FROM THE NFC HARDWARE
async fn handle_incoming_scan(
    Path(hash): Path<String>,
    State(state): State<AppState>,
) -> Json<String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let mut scan = state.latest_scan.lock().unwrap();
    *scan = Some(NfcData {
        hash: hash.clone(),
        timestamp: now,
    });

    // THIS WILL LOG TO YOUR TERMINAL
    println!("💳 [SCAN] New Card Detected! Hash: {}", hash);

    Json(format!("Handshake Received: {}", hash))
}

// 2. THIS TELLS THE FRONTEND IF A CARD IS WAITING
async fn get_latest_scan(State(state): State<AppState>) -> Json<ScanResponse> {
    let scan = state.latest_scan.lock().unwrap();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    match &*scan {
        Some(data) => {
            let is_fresh = (now - data.timestamp) < 60; // Valid for 60 seconds
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

// 3. ADMIN REGISTRATION
async fn register_citizen(
    State(state): State<AppState>,
    Json(p): Json<RegisterRequest>,
) -> Json<String> {
    let nfc_id = {
        let mut scan = state.latest_scan.lock().unwrap();
        scan.take().map(|s| s.hash).unwrap_or_default()
    };

    if nfc_id.is_empty() {
        return Json("ERROR: No card scanned recently".into());
    }

    println!(
        "📝 [ADMIN] Registering {} with NFC {}",
        p.citizen_name, nfc_id
    );

    let output = Command::new("stellar")
        .args([
            "contract",
            "invoke",
            "--id",
            &env::var("CONTRACT_ID").unwrap(),
            "--source-account",
            &env::var("ADMIN_SECRET").unwrap(),
            "--network",
            "testnet",
            "--",
            "register_citizen",
            "--admin",
            "GCJJ7WCTRWLR7YLOWZH6VGCYKZ62HG2N7US7AUQPT762GDN7HFA4Y7Q5",
            "--citizen_addr",
            &p.citizen_addr,
            "--nfc_id",
            &nfc_id,
            "--name",
            &p.citizen_name,
        ])
        .output();

    match output {
        Ok(out) if out.status.success() => Json("Success: Identity Committed".into()),
        _ => Json("Error: Blockchain write failed".into()),
    }
}

// 4. BENEFICIARY CLAIM
async fn claim_aid(State(state): State<AppState>, Json(p): Json<ClaimRequest>) -> Json<String> {
    let nfc_id = {
        let mut scan = state.latest_scan.lock().unwrap();
        scan.take().map(|s| s.hash).unwrap_or_default()
    };

    if nfc_id.is_empty() {
        return Json("ERROR: Physical card tap required".into());
    }

    println!(
        "💰 [CLAIM] Processing claim for {} using NFC {}",
        p.beneficiary_addr, nfc_id
    );

    let output = Command::new("stellar")
        .args([
            "contract",
            "invoke",
            "--id",
            &env::var("CONTRACT_ID").unwrap(),
            "--network",
            "testnet",
            "--",
            "claim_aid",
            "--citizen_addr",
            &p.beneficiary_addr,
            "--nfc_id",
            &nfc_id,
        ])
        .output();

    match output {
        Ok(out) if out.status.success() => Json("Success: Funds Disbursed".into()),
        _ => Json("Error: Claim denied or verification failed".into()),
    }
}

