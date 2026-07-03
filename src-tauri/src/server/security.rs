use axum::body::Body;
use axum::http::{header, HeaderName, HeaderValue, Request};
use axum::middleware::Next;
use axum::response::Response;

pub(super) async fn add_security_headers(request: Request<Body>, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    insert_header(headers, header::CONTENT_SECURITY_POLICY, "default-src 'self'; script-src 'self'; script-src-attr 'none'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'none'; connect-src 'self'; media-src 'none'; object-src 'none'; child-src 'none'; frame-src 'none'; frame-ancestors 'none'; worker-src 'none'; manifest-src 'none'; base-uri 'none'; form-action 'self'");
    insert_header(headers, header::X_CONTENT_TYPE_OPTIONS, "nosniff");
    insert_header(headers, header::REFERRER_POLICY, "no-referrer");
    insert_header(headers, header::X_FRAME_OPTIONS, "DENY");
    insert_header(
        headers,
        HeaderName::from_static("permissions-policy"),
        "camera=(), microphone=(), geolocation=()",
    );
    insert_header(headers, header::CACHE_CONTROL, "no-store");
    insert_header(headers, header::PRAGMA, "no-cache");
    response
}

fn insert_header(headers: &mut http::HeaderMap, name: HeaderName, value: &'static str) {
    headers.insert(name, HeaderValue::from_static(value));
}
