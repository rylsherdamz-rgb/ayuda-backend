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

#[derive(Clone, Debug, Serialize, Deserialize)]
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

#[derive(Serialize)]
struct TxResponse {
    xdr: String,
    status: String,
}

const ADMIN_PUBKEY: &str = "GAJPZCOVW34KTYF764X74ZRYOJIF3H2XKCRWH4CARVRZD5M4WJ2XVWLW";

#[tokio::main]
async fn main() {
    let state = AppState {
        latest_scan: Arc::new(Mutex::new(None)),
    };

    let app = Router::new()
        .route("/api/health", get(|| async { "Ayuda Protocol Online" }))
        .route(
            "/api/scan/{hash}",
            get(handle_incoming_scan).post(handle_incoming_scan),
        )
        .route("/api/latest-scan", get(get_latest_scan))
        .route("/api/register", post(register_citizen))
        .route("/api/claim", post(claim_aid))
        .with_state(state)
        .layer(CorsLayer::permissive());

    let port = env::var("PORT").unwrap_or_else(|_| "10000".to_string());
    let addr = format!("0.0.0.0:{}", port);

    println!("🚀 [SERVER] Ayuda Bridge running on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

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

    println!("💳 [SCAN] NFC Detected: {}", hash);
    Json(format!("Handshake Received: {}", hash))
}

async fn get_latest_scan(State(state): State<AppState>) -> Json<ScanResponse> {
    let scan = state.latest_scan.lock().unwrap();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    match &*scan {
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

async fn register_citizen(
    State(state): State<AppState>,
    Json(p): Json<RegisterRequest>,
) -> Json<TxResponse> {
    let nfc_id = {
        let scan = state.latest_scan.lock().unwrap();
        scan.as_ref().map(|s| s.hash.clone()).unwrap_or_default()
    };

    if nfc_id.is_empty() {
        return Json(TxResponse {
            xdr: "".into(),
            status: "ERROR: Tap Required".into(),
        });
    }

    let contract_id = env::var("CONTRACT_ID").expect("CONTRACT_ID not set");

    let output = Command::new("stellar")
        .args([
            "contract",
            "invoke",
            "--id",
            &contract_id,
            "--network",
            "testnet",
            "--source-account",
            ADMIN_PUBKEY,
            "--build-only",
            "--base64",
            "--",
            "register_citizen",
            "--admin",
            ADMIN_PUBKEY,
            "--citizen_addr",
            &p.citizen_addr,
            "--nfc_id",
            &nfc_id,
            "--name",
            &p.citizen_name,
        ])
        .output();

    handle_stellar_output(output, state)
}

async fn claim_aid(State(state): State<AppState>, Json(p): Json<ClaimRequest>) -> Json<TxResponse> {
    let nfc_id = {
        let scan = state.latest_scan.lock().unwrap();
        scan.as_ref().map(|s| s.hash.clone()).unwrap_or_default()
    };

    if nfc_id.is_empty() {
        return Json(TxResponse {
            xdr: "".into(),
            status: "ERROR: Tap Required".into(),
        });
    }

    let contract_id = env::var("CONTRACT_ID").expect("CONTRACT_ID not set");

    let output = Command::new("stellar")
        .args([
            "contract",
            "invoke",
            "--id",
            &contract_id,
            "--network",
            "testnet",
            "--source-account",
            &p.beneficiary_addr,
            "--build-only",
            "--base64",
            "--",
            "claim_aid",
            "--citizen_addr",
            &p.beneficiary_addr,
            "--nfc_id",
            &nfc_id,
        ])
        .output();

    handle_stellar_output(output, state)
}

fn handle_stellar_output(
    output: Result<std::process::Output, std::io::Error>,
    state: AppState,
) -> Json<TxResponse> {
    match output {
        Ok(out) => {
            if out.status.success() {
                let xdr = String::from_utf8_lossy(&out.stdout).trim().to_string();
                let mut scan = state.latest_scan.lock().unwrap();
                *scan = None;

                Json(TxResponse {
                    xdr,
                    status: "pending_signature".into(),
                })
            } else {
                let err = String::from_utf8_lossy(&out.stderr);
                Json(TxResponse {
                    xdr: "".into(),
                    status: format!("Stellar Error: {}", err),
                })
            }
        }
        Err(e) => Json(TxResponse {
            xdr: "".into(),
            status: format!("CLI Error: {}", e),
        }),
    }
}

