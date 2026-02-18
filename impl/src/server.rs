use axum::{
    extract::State,
    http::StatusCode,
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;

use crate::runner::Runner;
use crate::store::FileStore;
use crate::tools::ToolRegistry;
use crate::value::Value;

#[derive(Clone)]
struct AppState {
    store_path: PathBuf,
    // ToolRegistry is not Clone, so we wrap it in Arc for sharing across requests if needed,
    // but Runner takes ownership. We'll need to create a new registry or make it clonable.
    // For now, let's just recreate it per request since it's cheap (just inserting function pointers).
}

#[derive(Deserialize)]
struct RunRequest {
    id: String,
    source: String,
}

#[derive(Serialize)]
struct RunResponse {
    result: Value,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

pub async fn serve(port: u16, store_path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    let state = AppState { store_path };

    let app = Router::new()
        .route("/run", post(run_handler))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("listening on {}", addr);
    
    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn run_handler(
    State(state): State<AppState>,
    Json(payload): Json<RunRequest>,
) -> Result<Json<RunResponse>, (StatusCode, Json<ErrorResponse>)> {
    let store_path = state.store_path.clone();
    let id = payload.id.clone();
    let source = payload.source.clone();

    // Offload the blocking runner to a thread
    let result = tokio::task::spawn_blocking(move || {
        let store = FileStore::new(store_path);
        let tools = ToolRegistry::new();
                let mut runner = Runner::new(store, tools);
                runner.run(&id, &source, None)
            })
    .await
    .map_err(|e| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: format!("Task join error: {}", e),
        }),
    ))?;

    match result {
        Ok(value) => Ok(Json(RunResponse { result: value })),
        Err(e) => Err((
            StatusCode::BAD_REQUEST, // Or Internal Server Error depending on error
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}
