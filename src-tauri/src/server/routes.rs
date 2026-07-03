use super::{download, upload, *};
use axum::extract::DefaultBodyLimit;
use axum::middleware;
use axum::routing::{get, post};
use axum::Router;

pub fn build_router(app_state: AppState, app: Option<EventHandle>) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/d/:token", get(download::download_page))
        .route("/api/share/:token", get(download::share_api))
        .route(
            "/download/:token",
            get(download::download_file).head(download::download_head),
        )
        .route("/u/:token", get(upload::upload_page))
        .route("/upload.js", get(upload::upload_script))
        .route("/api/upload-request/:token", post(upload::upload_request))
        .route("/api/upload-status/:token", get(upload::upload_status))
        .route(
            "/upload/:token",
            post(upload::upload_file).layer(DefaultBodyLimit::disable()),
        )
        .route("/health", get(health))
        .with_state(HttpState { app_state, app })
        .layer(middleware::from_fn(security::add_security_headers))
}

pub fn build_onboarding_router() -> Router {
    Router::new()
        .route("/connect", get(onboarding_page))
        .route("/connect.js", get(onboarding_script))
        .route("/health", get(health))
        .layer(middleware::from_fn(security::add_security_headers))
}
