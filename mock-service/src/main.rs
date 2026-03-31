use axum::{
    extract::{Json, Path},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
use clap::Parser;
use serde_json::{json, Value};
use std::net::SocketAddr;
use tokio::net::TcpListener;

#[derive(Parser, Debug)]
#[command(name = "mock-service", about = "Echo HTTP server for rs-rok integration tests")]
struct Args {
    /// Port to listen on
    #[arg(short, long, default_value_t = 9999)]
    port: u16,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .json()
        .init();

    let args = Args::parse();

    let app = Router::new()
        .route("/echo", get(echo_get).post(echo_post))
        .route("/status/{code}", get(status_handler))
        .route("/slow/{ms}", get(slow_handler))
        .route("/health", get(health));

    let addr = SocketAddr::from(([127, 0, 0, 1], args.port));
    tracing::info!("mock-service listening on {}", addr);

    let listener = TcpListener::bind(addr).await.expect("failed to bind");
    axum::serve(listener, app)
        .await
        .expect("server error");
}

async fn echo_get(headers: HeaderMap) -> impl IntoResponse {
    let header_map: serde_json::Map<String, Value> = headers
        .iter()
        .map(|(k, v)| {
            (
                k.as_str().to_owned(),
                Value::String(v.to_str().unwrap_or("").to_owned()),
            )
        })
        .collect();

    Json(json!({
        "method": "GET",
        "headers": header_map,
        "body": null
    }))
}

async fn echo_post(headers: HeaderMap, body: String) -> impl IntoResponse {
    let header_map: serde_json::Map<String, Value> = headers
        .iter()
        .map(|(k, v)| {
            (
                k.as_str().to_owned(),
                Value::String(v.to_str().unwrap_or("").to_owned()),
            )
        })
        .collect();

    Json(json!({
        "method": "POST",
        "headers": header_map,
        "body": body
    }))
}

async fn status_handler(Path(code): Path<u16>) -> impl IntoResponse {
    let status = StatusCode::from_u16(code).unwrap_or(StatusCode::BAD_REQUEST);
    (status, format!("Status: {}", code))
}

async fn slow_handler(Path(ms): Path<u64>) -> impl IntoResponse {
    let capped = ms.min(30_000); // cap at 30s
    tokio::time::sleep(std::time::Duration::from_millis(capped)).await;
    Json(json!({ "delayed_ms": capped }))
}

async fn health() -> &'static str {
    "ok"
}
