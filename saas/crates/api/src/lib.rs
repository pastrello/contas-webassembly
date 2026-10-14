use axum::{extract::State, http::StatusCode, routing::get, Json, Router};
use contas_contracts::ServiceStatus;
use sqlx::PgPool;
use tower_http::{services::ServeDir, trace::TraceLayer};

pub fn app(pool: PgPool, web_dir: &str) -> Router {
    Router::new()
        .route("/health/live", get(|| async { StatusCode::NO_CONTENT }))
        .route("/health/ready", get(ready))
        .route("/api/status", get(status))
        .fallback_service(ServeDir::new(web_dir))
        .layer(TraceLayer::new_for_http())
        .with_state(pool)
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

async fn ready(State(pool): State<PgPool>) -> StatusCode {
    if database_ready(&pool).await {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    }
}

async fn status(State(pool): State<PgPool>) -> (StatusCode, Json<ServiceStatus>) {
    let connected = database_ready(&pool).await;
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
    async fn liveness_does_not_depend_on_database_but_readiness_does() {
        let pool = PgPoolOptions::new()
            .acquire_timeout(std::time::Duration::from_millis(100))
            .connect_lazy("postgres://test:test@127.0.0.1:1/test")
            .unwrap();
        let router = app(pool, "missing-web-dir");
        for (path, expected) in [
            ("/health/live", StatusCode::NO_CONTENT),
            ("/health/ready", StatusCode::SERVICE_UNAVAILABLE),
            ("/api/status", StatusCode::SERVICE_UNAVAILABLE),
            ("/api/unknown", StatusCode::NOT_FOUND),
        ] {
            let response = router
                .clone()
                .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), expected, "{path}");
        }
    }
}
