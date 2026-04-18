use axum::{routing::post, Json, Router};
use serde::Deserialize;
use std::env;
use std::process::Command;
use tower_http::cors::CorsLayer;

#[derive(Deserialize)]
struct RegisterRequest {
    citizen_addr: String,
    citizen_id: String,
    nfc_hash: String,
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/register", post(register_citizen))
        .layer(CorsLayer::permissive());

    // Use Render's PORT or default to 3000 for local testing
    let port = env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("0.0.0.0:{}", port);

    println!("Ayuda Backend online at {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn register_citizen(Json(payload): Json<RegisterRequest>) -> Json<serde_json::Value> {
    // Pull configuration from Environment Variables set in Render
    let contract_id = env::var("CONTRACT_ID").unwrap_or_else(|_| "NOT_SET".to_string());
    let admin_secret = env::var("ADMIN_SECRET").expect("ADMIN_SECRET must be set");
    let admin_public = env::var("ADMIN_PUBLIC").expect("ADMIN_PUBLIC must be set");

    let output = Command::new("stellar")
        .args([
            "contract",
            "invoke",
            "--id",
            &contract_id,
            "--source-account",
            &admin_secret, // Use the secret S... key from env
            "--network",
            "testnet",
            "--",
            "register_citizen",
            "--admin",
            &admin_public,
            "--citizen_addr",
            &payload.citizen_addr,
            "--name", // Make sure this matches your lib.rs parameter name
            &payload.citizen_id,
            "--nfc_hash",
            &payload.nfc_hash,
        ])
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();

            if out.status.success() {
                Json(serde_json::json!({ "status": "success", "result": stdout }))
            } else {
                Json(serde_json::json!({ "status": "error", "message": stderr }))
            }
        }
        Err(e) => Json(serde_json::json!({ "status": "error", "message": e.to_string() })),
    }
}
