use sqlx::postgres::PgPoolOptions;
use std::{env, time::Duration};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "contas_api=info,tower_http=info".into()),
        )
        .init();
    let database_url = env::var("DATABASE_URL")
        .map_err(|_| "Defina DATABASE_URL para o banco PostgreSQL de desenvolvimento")?;
    let bind = env::var("APP_BIND").unwrap_or_else(|_| "127.0.0.1:8080".into());
    let web_dir = env::var("WEB_DIR").unwrap_or_else(|_| "web".into());
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(2))
        .connect_lazy(&database_url)?;
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!(address = %listener.local_addr()?, "Contas SaaS iniciado");
    axum::serve(listener, contas_api::app(pool, &web_dir))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
