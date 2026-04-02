use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use vidodo_capability::{CapabilityRegistry, route};
use vidodo_ir::{Diagnostic, ResponseEnvelope};

pub(crate) struct AppState {
    registry: CapabilityRegistry,
}

#[tokio::main]
async fn main() {
    let state = Arc::new(AppState { registry: CapabilityRegistry::default() });

    let app = build_router(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:7400")
        .await
        .expect("failed to bind to 127.0.0.1:7400");
    eprintln!("core-service listening on 127.0.0.1:7400");
    axum::serve(listener, app).await.expect("server error");
}

fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/capabilities", get(capabilities))
        .route("/capability/{id}", post(invoke_capability))
        .with_state(state)
}

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({"status": "ok"}))
}

async fn capabilities(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let list: Vec<_> = state
        .registry
        .list()
        .iter()
        .map(|desc| {
            serde_json::json!({
                "capability": desc.capability,
                "version": desc.version,
                "execution_mode": desc.execution_mode,
                "description": desc.description,
            })
        })
        .collect();
    Json(serde_json::json!({"capabilities": list}))
}

async fn invoke_capability(
    State(_state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match route(&id) {
        Ok(target) => {
            // WSH-06/07 will implement actual dispatch per RouteTarget.
            // For now, acknowledge the route was resolved.
            let envelope = ResponseEnvelope::new(
                "accepted",
                &id,
                "stub-request",
                serde_json::json!({"route": format!("{target:?}")}),
                Vec::<Diagnostic>::new(),
                Vec::<String>::new(),
                Vec::<String>::new(),
            );
            (StatusCode::OK, Json(serde_json::to_value(envelope).unwrap_or_default()))
        }
        Err(diagnostic) => {
            let envelope = ResponseEnvelope::new(
                "error",
                &id,
                "stub-request",
                serde_json::json!(null),
                vec![*diagnostic],
                Vec::<String>::new(),
                Vec::<String>::new(),
            );
            (StatusCode::NOT_FOUND, Json(serde_json::to_value(envelope).unwrap_or_default()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    fn test_app() -> Router {
        let state = Arc::new(AppState { registry: CapabilityRegistry::default() });
        build_router(state)
    }

    #[tokio::test]
    async fn health_returns_ok() {
        let app = test_app();
        let response =
            app.oneshot(Request::get("/health").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "ok");
    }

    #[tokio::test]
    async fn capabilities_returns_list() {
        let app = test_app();
        let response =
            app.oneshot(Request::get("/capabilities").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let capabilities = json["capabilities"].as_array().unwrap();
        assert!(capabilities.len() >= 12, "expected 12+ capabilities, got {}", capabilities.len());
    }

    #[tokio::test]
    async fn invoke_known_capability_returns_accepted() {
        let app = test_app();
        let response = app
            .oneshot(
                Request::post("/capability/asset.ingest")
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "accepted");
        assert_eq!(json["capability"], "asset.ingest");
    }

    #[tokio::test]
    async fn invoke_unknown_capability_returns_not_found() {
        let app = test_app();
        let response = app
            .oneshot(
                Request::post("/capability/nonexistent.thing")
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "error");
    }
}
