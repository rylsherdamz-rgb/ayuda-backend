use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{env, process::Command, sync::{Arc, Mutex}};
use tower_http::cors::CorsLayer;

#[derive(Clone, Default)]
struct AppState {
    latest_scan: Arc<Mutex<Option<String>>>,
}

#[derive(Deserialize)]
struct ScanRequest {
    nfc_hash: String,
}

#[derive(Deserialize)]
struct RegisterRequest {
    student_addr: Option<String>,
    student_name: Option<String>,
    certificate_hash: Option<String>,
    reward_amount: Option<i128>,
    citizen_addr: Option<String>,
    citizen_id: Option<String>,
    nfc_hash: Option<String>,
}

#[derive(Deserialize)]
struct VerifyRequest {
    student_addr: String,
    certificate_hash: String,
}

#[derive(Serialize)]
struct ApiResponse {
    status: &'static str,
    message: String,
    result: Option<String>,
    certificate_hash: Option<String>,
}

#[derive(Serialize)]
struct ScanResponse {
    nfc_hash: Option<String>,
}

#[tokio::main]
async fn main() {
    let state = AppState::default();

    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/scan", post(store_scan))
        .route("/api/latest-scan", get(get_latest_scan))
        .route("/api/register", post(register_certificate))
        .route("/api/verify", post(verify_certificate))
        .with_state(state)
        .layer(CorsLayer::permissive());

    let port = env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("0.0.0.0:{port}");

    println!("Stellaroid Earn backend online at {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn health() -> Json<ApiResponse> {
    Json(ApiResponse {
        status: "success",
        message: "backend online".to_string(),
        result: None,
        certificate_hash: None,
    })
}

async fn store_scan(
    State(state): State<AppState>,
    Json(payload): Json<ScanRequest>,
) -> Json<ApiResponse> {
    let mut latest_scan = state.latest_scan.lock().unwrap();
    *latest_scan = Some(payload.nfc_hash.clone());

    Json(ApiResponse {
        status: "success",
        message: "scan stored".to_string(),
        result: None,
        certificate_hash: Some(payload.nfc_hash),
    })
}

async fn get_latest_scan(State(state): State<AppState>) -> Json<ScanResponse> {
    let latest_scan = state.latest_scan.lock().unwrap().clone();
    Json(ScanResponse { nfc_hash: latest_scan })
}

async fn register_certificate(
    State(state): State<AppState>,
    Json(payload): Json<RegisterRequest>,
) -> Json<ApiResponse> {
    let student_addr = payload
        .student_addr
        .or(payload.citizen_addr)
        .unwrap_or_default();
    let student_name = payload
        .student_name
        .or(payload.citizen_id)
        .unwrap_or_default();

    let certificate_hash = payload
        .certificate_hash
        .or(payload.nfc_hash)
        .or_else(|| state.latest_scan.lock().unwrap().clone())
        .unwrap_or_default();

    if student_addr.is_empty() || student_name.is_empty() || certificate_hash.is_empty() {
        return Json(ApiResponse {
            status: "error",
            message: "student_addr, student_name, and certificate_hash are required".to_string(),
            result: None,
            certificate_hash: if certificate_hash.is_empty() {
                None
            } else {
                Some(certificate_hash)
            },
        });
    }

    let register_result = invoke_contract(&[
        "register_certificate",
        "--admin",
        &required_env("ADMIN_PUBLIC"),
        "--student",
        &student_addr,
        "--student_name",
        &student_name,
        "--certificate_hash",
        &certificate_hash,
    ]);

    match register_result {
        Ok(stdout) => {
            if let Some(reward_amount) = payload.reward_amount.filter(|amount| *amount > 0) {
                let reward_result = invoke_contract(&[
                    "reward_student",
                    "--admin",
                    &required_env("ADMIN_PUBLIC"),
                    "--student",
                    &student_addr,
                    "--amount",
                    &reward_amount.to_string(),
                ]);

                match reward_result {
                    Ok(reward_stdout) => Json(ApiResponse {
                        status: "success",
                        message: "certificate registered and reward sent".to_string(),
                        result: Some(format!(
                            "register: {}\nreward: {}",
                            stdout.trim(),
                            reward_stdout.trim()
                        )),
                        certificate_hash: Some(certificate_hash),
                    }),
                    Err(error) => Json(ApiResponse {
                        status: "error",
                        message: format!("certificate registered but reward failed: {error}"),
                        result: Some(stdout),
                        certificate_hash: Some(certificate_hash),
                    }),
                }
            } else {
                Json(ApiResponse {
                    status: "success",
                    message: "certificate registered".to_string(),
                    result: Some(stdout),
                    certificate_hash: Some(certificate_hash),
                })
            }
        }
        Err(error) => Json(ApiResponse {
            status: "error",
            message: error,
            result: None,
            certificate_hash: Some(certificate_hash),
        }),
    }
}

async fn verify_certificate(Json(payload): Json<VerifyRequest>) -> Json<ApiResponse> {
    if payload.student_addr.is_empty() || payload.certificate_hash.is_empty() {
        return Json(ApiResponse {
            status: "error",
            message: "student_addr and certificate_hash are required".to_string(),
            result: None,
            certificate_hash: None,
        });
    }

    match invoke_contract(&[
        "verify_certificate",
        "--student",
        &payload.student_addr,
        "--certificate_hash",
        &payload.certificate_hash,
    ]) {
        Ok(stdout) => Json(ApiResponse {
            status: "success",
            message: "verification completed".to_string(),
            result: Some(stdout),
            certificate_hash: Some(payload.certificate_hash),
        }),
        Err(error) => Json(ApiResponse {
            status: "error",
            message: error,
            result: None,
            certificate_hash: Some(payload.certificate_hash),
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

    let output = command.output().map_err(|error| error.to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if output.status.success() {
        Ok(stdout)
    } else if stderr.is_empty() {
        Err("contract invocation failed".to_string())
    } else {
        Err(stderr)
    }
}

fn required_env(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("{name} must be set"))
}
