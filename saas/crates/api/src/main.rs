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
    if env::args().any(|a| a == "--migrate") {
        let url = env::var("MIGRATION_DATABASE_URL")
            .map_err(|_| "Defina MIGRATION_DATABASE_URL somente para executar migracoes")?;
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&url)
            .await?;
        sqlx::migrate::Migrator::new(std::path::Path::new("migrations"))
            .await?
            .run(&pool)
            .await?;
        if let Ok(password) = env::var("APP_DATABASE_PASSWORD") {
            if password.len() < 16 {
                return Err("APP_DATABASE_PASSWORD deve ter pelo menos 16 caracteres".into());
            }
            let mut tx = pool.begin().await?;
            sqlx::query("SELECT set_config('contas.bootstrap_password',$1,true)")
                .bind(password)
                .execute(&mut *tx)
                .await?;
            sqlx::query("DO $bootstrap$ BEGIN EXECUTE format('ALTER ROLE contas_app LOGIN PASSWORD %L', current_setting('contas.bootstrap_password')); END $bootstrap$").execute(&mut *tx).await?;
            tx.commit().await?;
        }
        pool.close().await;
        println!("Migracoes aplicadas.");
        return Ok(());
    }
    let url = env::var("DATABASE_URL")
        .map_err(|_| "Defina DATABASE_URL com a conta restrita contas_app")?;
    let bind = env::var("APP_BIND").unwrap_or_else(|_| "127.0.0.1:8080".into());
    let origin = env::var("PUBLIC_ORIGIN").unwrap_or_else(|_| "http://127.0.0.1:8080".into());
    let development = env::var("APP_ENV").as_deref() == Ok("development");
    let secure_cookie = !development;
    if (!development && !origin.starts_with("https://")) || origin.ends_with('/') {
        return Err(
            "PUBLIC_ORIGIN deve usar HTTPS (exceto APP_ENV=development) e nao terminar com /"
                .into(),
        );
    }
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(2))
        .connect(&url)
        .await?;
    contas_api::check_runtime_role(&pool).await?;
    let config = contas_api::Config {
        origin,
        secure_cookie,
        registration: env::var("ALLOW_REGISTRATION").as_deref() == Ok("true"),
    };
    let web = env::var("WEB_DIR").unwrap_or_else(|_| "web".into());
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!(address=%listener.local_addr()?,"Contas SaaS iniciado");
    axum::serve(listener, contas_api::app(pool, &web, config))
        .with_graceful_shutdown(shutdown())
        .await?;
    Ok(())
}
async fn shutdown() {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("SIGTERM handler");
        tokio::select! { _=tokio::signal::ctrl_c()=>{}, _=terminate.recv()=>{} }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
