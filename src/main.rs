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

#[tokio::main]
async fn main() {
    let state = AppState {
        latest_scan: Arc::new(Mutex::new(None)),
    };
    let app = Router::new()
        .route("/api/scan/{hash}", get(handle_scan))
        .route("/api/latest-scan", get(get_latest))
        .route("/api/register", post(register))
        .route("/api/claim", post(claim))
        .with_state(state)
        .layer(CorsLayer::permissive());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn register(State(state): State<AppState>, Json(p): Json<RegisterRequest>) -> Json<String> {
    let hash = state.latest_scan.lock().unwrap().take().unwrap().hash;
    let admin = "GCJJ7WCTRWLR7YLOWZH6VGCYKZ62HG2N7US7AUQPT762GDN7HFA4Y7Q5";

    Command::new("stellar")
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
            admin,
            "--citizen_addr",
            &p.citizen_addr,
            "--nfc_id",
            &hash,
            "--name",
            &p.citizen_name,
        ])
        .output()
        .unwrap();
    Json("Registered".into())
}

async fn handle_scan(Path(hash): Path<String>, State(state): State<AppState>) -> Json<String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    *state.latest_scan.lock().unwrap() = Some(NfcData {
        hash: hash.clone(),
        timestamp: now,
    });
    Json("Scanned".into())
}

async fn get_latest(State(state): State<AppState>) -> Json<Option<String>> {
    let scan = state.latest_scan.lock().unwrap();
    Json(scan.as_ref().map(|s| s.hash.clone()))
}

async fn claim(State(state): State<AppState>, Json(p): Json<ClaimRequest>) -> Json<String> {
    let hash = state.latest_scan.lock().unwrap().take().unwrap().hash;
    Command::new("stellar")
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
            &hash,
        ])
        .output()
        .unwrap();
    Json("Claimed".into())
}
