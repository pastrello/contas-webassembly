pub mod auth;
pub mod catalogs;
use argon2::{
    password_hash::{PasswordHasher, SaltString},
    Argon2,
};
use axum::{
    extract::{DefaultBodyLimit, State},
    http::{header, HeaderValue, StatusCode},
    routing::{get, post},
    Json, Router,
};
use contas_contracts::ServiceStatus;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tower_http::{services::ServeDir, set_header::SetResponseHeaderLayer, trace::TraceLayer};

#[derive(Clone)]
pub struct Config {
    pub origin: String,
    pub secure_cookie: bool,
    pub registration: bool,
}
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Config,
    pub hash_slots: Arc<Semaphore>,
    pub dummy_hash: String,
}
pub fn app(pool: PgPool, web_dir: &str, config: Config) -> Router {
    let dummy_hash = Argon2::default()
        .hash_password(
            b"not-a-real-user-password",
            &SaltString::generate(&mut rand::rngs::OsRng),
        )
        .expect("argon2 default parameters")
        .to_string();
    Router::new()
        .route("/health/live",get(|| async {StatusCode::NO_CONTENT}))
        .route("/health/ready",get(ready))
        .route("/api/status",get(status))
        .route("/api/auth/register",post(auth::register))
        .route("/api/auth/login",post(auth::login))
        .route("/api/auth/logout",post(auth::logout))
        .route("/api/auth/me",get(auth::me))
        .route("/api/tenants/{id}",get(auth::tenant).put(auth::rename))
        .route("/api/members",post(auth::add_member))
        .route("/api/accounts",get(catalogs::accounts).post(catalogs::create_account))
        .route("/api/accounts/{id}",get(catalogs::get_account).put(catalogs::update_account))
        .route("/api/histories",get(catalogs::histories).post(catalogs::create_history))
        .route("/api/histories/{id}",get(catalogs::get_history).put(catalogs::update_history))
        .fallback_service(ServeDir::new(web_dir))
        .layer(DefaultBodyLimit::max(8192))
        .layer(SetResponseHeaderLayer::overriding(header::CACHE_CONTROL,HeaderValue::from_static("no-store")))
        .layer(SetResponseHeaderLayer::overriding(header::CONTENT_SECURITY_POLICY,HeaderValue::from_static("default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'")))
        .layer(SetResponseHeaderLayer::overriding(header::X_CONTENT_TYPE_OPTIONS,HeaderValue::from_static("nosniff")))
        .layer(SetResponseHeaderLayer::overriding(header::REFERRER_POLICY,HeaderValue::from_static("same-origin")))
        .layer(TraceLayer::new_for_http())
        .with_state(AppState {pool,config,hash_slots:Arc::new(Semaphore::new(4)),dummy_hash})
}
pub async fn check_runtime_role(pool: &PgPool) -> Result<(), sqlx::Error> {
    let safe:bool = sqlx::query_scalar("SELECT NOT (r.rolsuper OR r.rolbypassrls OR r.rolcreatedb OR r.rolcreaterole OR r.rolreplication) AND NOT EXISTS (SELECT 1 FROM pg_auth_members m WHERE m.member=r.oid) AND NOT EXISTS (SELECT 1 FROM pg_class c WHERE c.oid IN ('tenants'::regclass,'memberships'::regclass,'tenant_settings'::regclass,'accounts'::regclass,'histories'::regclass) AND (c.relowner=r.oid OR NOT c.relrowsecurity OR NOT c.relforcerowsecurity)) FROM pg_roles r WHERE r.rolname=current_user").fetch_one(pool).await?;
    if !safe {
        return Err(sqlx::Error::Protocol(
            "API requires a non-owner role without elevated privileges and enforced RLS".into(),
        ));
    }
    Ok(())
}
async fn database_ready(pool: &PgPool) -> bool {
    matches!(
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            sqlx::query_scalar::<_, i32>("SELECT 1").fetch_one(pool)
        )
        .await,
        Ok(Ok(1))
    )
}
async fn ready(State(state): State<AppState>) -> StatusCode {
    if database_ready(&state.pool).await {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    }
}
async fn status(State(state): State<AppState>) -> (StatusCode, Json<ServiceStatus>) {
    let connected = database_ready(&state.pool).await;
    (
        if connected {
            StatusCode::OK
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        },
        Json(ServiceStatus {
            service: "Contas SaaS".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            database: if connected {
                "connected"
            } else {
                "unavailable"
            }
            .into(),
        }),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use sqlx::postgres::PgPoolOptions;
    use tower::ServiceExt;
    #[tokio::test]
    async fn offline_and_anonymous_requests_fail_closed() {
        let pool = PgPoolOptions::new()
            .acquire_timeout(std::time::Duration::from_millis(100))
            .connect_lazy("postgres://test:test@127.0.0.1:1/test")
            .unwrap();
        let router = app(
            pool,
            "missing-web-dir",
            Config {
                origin: "http://localhost:8080".into(),
                secure_cookie: false,
                registration: true,
            },
        );
        for (path, expected) in [
            ("/health/live", 204),
            ("/health/ready", 503),
            ("/api/status", 503),
            ("/api/auth/me", 401),
            ("/api/unknown", 404),
        ] {
            let response = router
                .clone()
                .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status().as_u16(), expected, "{path}");
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        }
    }
}
