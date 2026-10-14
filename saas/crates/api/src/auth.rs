use crate::AppState;
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use rand::{rngs::OsRng, RngCore};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

#[derive(Debug)]
pub struct ApiError(pub StatusCode, pub &'static str);
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"error":self.1}))).into_response()
    }
}
impl From<sqlx::Error> for ApiError {
    fn from(error: sqlx::Error) -> Self {
        // Avoid SQL/parameters/credentials in responses or logs.
        tracing::warn!(kind = ?error.as_database_error().and_then(|e| e.code()), "database operation failed");
        Self(
            StatusCode::SERVICE_UNAVAILABLE,
            "Serviço temporariamente indisponível.",
        )
    }
}
type Result<T> = std::result::Result<T, ApiError>;
fn unauthorized() -> ApiError {
    ApiError(
        StatusCode::UNAUTHORIZED,
        "E-mail ou senha incorretos, ou sessão expirada.",
    )
}
fn invalid() -> ApiError {
    ApiError(
        StatusCode::BAD_REQUEST,
        "Confira os campos. A senha deve ter de 12 a 128 caracteres.",
    )
}
fn forbidden() -> ApiError {
    ApiError(StatusCode::FORBIDDEN, "Operação não permitida.")
}
fn token_hash(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}
fn cookie_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|part| {
            let (key, value) = part.trim().split_once('=')?;
            (key == "contas_session"
                && value.len() == 64
                && value.bytes().all(|b| b.is_ascii_hexdigit()))
            .then(|| value.to_owned())
        })
}
fn cookie(state: &AppState, token: &str, max_age: u32) -> HeaderValue {
    HeaderValue::from_str(&format!(
        "contas_session={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age={max_age}{}",
        if state.config.secure_cookie {
            "; Secure"
        } else {
            ""
        }
    ))
    .expect("generated cookie")
}
pub(crate) fn write_allowed(state: &AppState, headers: &HeaderMap) -> Result<()> {
    if headers.get("origin").and_then(|h| h.to_str().ok()) != Some(state.config.origin.as_str())
        || headers
            .get("x-contas-request")
            .and_then(|h| h.to_str().ok())
            != Some("1")
    {
        return Err(forbidden());
    }
    Ok(())
}
fn email(value: &str) -> Result<String> {
    let value = value.trim().to_ascii_lowercase();
    if value.len() > 254 || !value.is_ascii() || value.bytes().any(|b| b.is_ascii_whitespace()) {
        return Err(invalid());
    }
    match value.split_once('@') {
        Some((user, domain))
            if !user.is_empty() && domain.contains('.') && !domain.contains('@') =>
        {
            Ok(value)
        }
        _ => Err(invalid()),
    }
}
fn name(value: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 100 {
        return Err(invalid());
    }
    Ok(value.to_owned())
}
async fn password_hash(state: &AppState, password: String) -> Result<String> {
    if !(12..=128).contains(&password.chars().count()) || password.len() > 512 {
        return Err(invalid());
    }
    let permit = state
        .hash_slots
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| invalid())?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        Argon2::default()
            .hash_password(password.as_bytes(), &SaltString::generate(&mut OsRng))
            .map(|hash| hash.to_string())
            .map_err(|_| invalid())
    })
    .await
    .map_err(|_| invalid())?
}
async fn rate_limit(state: &AppState, bucket: String, limit: i32) -> Result<()> {
    sqlx::query("DELETE FROM auth_attempts WHERE expires_at < now()")
        .execute(&state.pool)
        .await?;
    let count: i32 = sqlx::query_scalar("INSERT INTO auth_attempts(bucket,attempts,expires_at) VALUES($1,1,now()+interval '15 minutes') ON CONFLICT(bucket) DO UPDATE SET attempts=CASE WHEN auth_attempts.expires_at < now() THEN 1 ELSE auth_attempts.attempts+1 END, expires_at=CASE WHEN auth_attempts.expires_at < now() THEN now()+interval '15 minutes' ELSE auth_attempts.expires_at END RETURNING attempts")
        .bind(bucket).fetch_one(&state.pool).await?;
    if count > limit {
        return Err(ApiError(
            StatusCode::TOO_MANY_REQUESTS,
            "Muitas tentativas. Tente novamente em 15 minutos.",
        ));
    }
    Ok(())
}
pub async fn set_context(
    tx: &mut Transaction<'_, Postgres>,
    tenant: Uuid,
    user: Uuid,
) -> Result<()> {
    sqlx::query("SELECT set_config('app.tenant_id',$1,true), set_config('app.user_id',$2,true)")
        .bind(tenant.to_string())
        .bind(user.to_string())
        .execute(&mut **tx)
        .await?;
    Ok(())
}
async fn issue_session(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    tenant: Uuid,
    user: Uuid,
    headers: &HeaderMap,
) -> Result<HeaderValue> {
    if let Some(previous) = cookie_token(headers) {
        sqlx::query("DELETE FROM sessions WHERE token_hash=$1")
            .bind(token_hash(&previous))
            .execute(&mut **tx)
            .await?;
    }
    sqlx::query("DELETE FROM sessions WHERE user_id=$1 AND expires_at <= now()")
        .bind(user)
        .execute(&mut **tx)
        .await?;
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    let token = hex::encode(bytes);
    sqlx::query("INSERT INTO sessions(token_hash,tenant_id,user_id) VALUES($1,$2,$3)")
        .bind(token_hash(&token))
        .bind(tenant)
        .bind(user)
        .execute(&mut **tx)
        .await?;
    Ok(cookie(state, &token, 43200))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Register {
    pub email: String,
    pub password: String,
    pub password_confirmation: String,
    pub name: String,
    pub tenant_name: String,
}
pub async fn register(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<Register>,
) -> Result<Response> {
    write_allowed(&state, &headers)?;
    if !state.config.registration {
        return Err(forbidden());
    }
    rate_limit(&state, "register:global".into(), 100).await?;
    confirm_password(&input.password, &input.password_confirmation)?;
    let email = email(&input.email)?;
    let user_name = name(&input.name)?;
    let tenant_name = name(&input.tenant_name)?;
    let hash = password_hash(&state, input.password).await?;
    let (user, tenant) = (Uuid::new_v4(), Uuid::new_v4());
    let mut tx = state.pool.begin().await?;
    set_context(&mut tx, tenant, user).await?;
    insert_user(&mut tx, user, &email, &user_name, &hash).await?;
    sqlx::query("INSERT INTO tenants(id,name) VALUES($1,$2)")
        .bind(tenant)
        .bind(tenant_name)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO memberships(tenant_id,user_id,role) VALUES($1,$2,'owner')")
        .bind(tenant)
        .bind(user)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO tenant_settings(tenant_id,updated_by) VALUES($1,$2)")
        .bind(tenant)
        .bind(user)
        .execute(&mut *tx)
        .await?;
    let cookie = issue_session(&state, &mut tx, tenant, user, &headers).await?;
    tx.commit().await?;
    Ok((
        StatusCode::CREATED,
        [(header::SET_COOKIE, cookie)],
        Json(json!({"ok":true})),
    )
        .into_response())
}
async fn insert_user(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    email: &str,
    name: &str,
    hash: &str,
) -> Result<()> {
    let result =
        sqlx::query("INSERT INTO users(id,email,display_name,password_hash) VALUES($1,$2,$3,$4)")
            .bind(id)
            .bind(email)
            .bind(name)
            .bind(hash)
            .execute(&mut **tx)
            .await;
    match result {
        Err(sqlx::Error::Database(ref e)) if e.is_unique_violation() => Err(ApiError(
            StatusCode::CONFLICT,
            "Cadastro indisponível para este e-mail.",
        )),
        other => {
            other?;
            Ok(())
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Login {
    pub email: String,
    pub password: String,
}
pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<Login>,
) -> Result<Response> {
    write_allowed(&state, &headers)?;
    let email = email(&input.email).map_err(|_| unauthorized())?;
    rate_limit(&state, "login:global".into(), 500).await?;
    rate_limit(&state, format!("login:{}", token_hash(&email)), 10).await?;
    if input.password.len() > 512 {
        return Err(unauthorized());
    }
    let row = sqlx::query("SELECT id,password_hash FROM users WHERE email=$1 AND active")
        .bind(&email)
        .fetch_optional(&state.pool)
        .await?;
    let hash: String = row
        .as_ref()
        .map(|r| r.get("password_hash"))
        .unwrap_or_else(|| state.dummy_hash.clone());
    let permit = state
        .hash_slots
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| unauthorized())?;
    let valid = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        PasswordHash::new(&hash).is_ok_and(|hash| {
            Argon2::default()
                .verify_password(input.password.as_bytes(), &hash)
                .is_ok()
        })
    })
    .await
    .map_err(|_| unauthorized())?;
    if !valid {
        return Err(unauthorized());
    }
    let user: Uuid = row.ok_or_else(unauthorized)?.get("id");
    let mut tx = state.pool.begin().await?;
    set_context(&mut tx, Uuid::nil(), user).await?;
    let tenant: Uuid = sqlx::query_scalar(
        "SELECT tenant_id FROM memberships WHERE user_id=$1 ORDER BY tenant_id LIMIT 1",
    )
    .bind(user)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(unauthorized)?;
    set_context(&mut tx, tenant, user).await?;
    let cookie = issue_session(&state, &mut tx, tenant, user, &headers).await?;
    tx.commit().await?;
    Ok(([(header::SET_COOKIE, cookie)], Json(json!({"ok":true}))).into_response())
}
pub struct Identity {
    pub user: Uuid,
    pub tenant: Uuid,
    pub role: String,
    pub email: String,
    pub name: String,
}
pub async fn authorized(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<(Transaction<'static, Postgres>, Identity)> {
    let token = cookie_token(headers).ok_or_else(unauthorized)?;
    let mut tx = state.pool.begin().await?;
    let session = sqlx::query(
        "SELECT user_id,tenant_id FROM sessions WHERE token_hash=$1 AND expires_at>now()",
    )
    .bind(token_hash(&token))
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(unauthorized)?;
    let (user, tenant) = (session.get("user_id"), session.get("tenant_id"));
    set_context(&mut tx, tenant, user).await?;
    let row = sqlx::query("SELECT m.role,u.email,u.display_name FROM memberships m JOIN users u ON u.id=m.user_id WHERE m.user_id=$1 AND m.tenant_id=$2 AND u.active")
        .bind(user).bind(tenant).fetch_optional(&mut *tx).await?.ok_or_else(unauthorized)?;
    Ok((
        tx,
        Identity {
            user,
            tenant,
            role: row.get("role"),
            email: row.get("email"),
            name: row.get("display_name"),
        },
    ))
}
pub async fn me(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<Value>> {
    let (mut tx, identity) = authorized(&state, &headers).await?;
    let tenant: String = sqlx::query_scalar("SELECT name FROM tenants WHERE id=$1")
        .bind(identity.tenant)
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(
        json!({"user_id":identity.user,"email":identity.email,"name":identity.name,"tenant_id":identity.tenant,"tenant_name":tenant,"role":identity.role}),
    ))
}
pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Result<Response> {
    write_allowed(&state, &headers)?;
    if let Some(token) = cookie_token(&headers) {
        sqlx::query("DELETE FROM sessions WHERE token_hash=$1")
            .bind(token_hash(&token))
            .execute(&state.pool)
            .await?;
    }
    Ok((
        StatusCode::NO_CONTENT,
        [(header::SET_COOKIE, cookie(&state, "", 0))],
    )
        .into_response())
}
pub async fn tenant(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    let (mut tx, _) = authorized(&state, &headers).await?;
    let name: String = sqlx::query_scalar("SELECT name FROM tenants WHERE id=$1")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ApiError(StatusCode::NOT_FOUND, "Ambiente não encontrado."))?;
    tx.commit().await?;
    Ok(Json(json!({"id":id,"name":name})))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rename {
    pub name: String,
}
pub async fn rename(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(input): Json<Rename>,
) -> Result<StatusCode> {
    write_allowed(&state, &headers)?;
    let name = name(&input.name)?;
    let (mut tx, identity) = authorized(&state, &headers).await?;
    if identity.role != "owner" {
        return Err(forbidden());
    }
    let result = sqlx::query("UPDATE tenants SET name=$1 WHERE id=$2")
        .bind(name)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    if result.rows_affected() != 1 {
        return Err(ApiError(StatusCode::NOT_FOUND, "Ambiente não encontrado."));
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Member {
    pub email: String,
    pub name: String,
    pub password: String,
    pub password_confirmation: String,
}
pub async fn add_member(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<Member>,
) -> Result<StatusCode> {
    write_allowed(&state, &headers)?;
    let (tx, identity) = authorized(&state, &headers).await?;
    if identity.role != "owner" {
        return Err(forbidden());
    }
    tx.commit().await?;
    let email = email(&input.email)?;
    let name = name(&input.name)?;
    rate_limit(&state, format!("member:{}", identity.tenant), 30).await?;
    confirm_password(&input.password, &input.password_confirmation)?;
    let hash = password_hash(&state, input.password).await?;
    // Revalidate permission after hashing, which runs outside the transaction.
    let (mut tx, identity) = authorized(&state, &headers).await?;
    if identity.role != "owner" {
        return Err(forbidden());
    }
    let user = Uuid::new_v4();
    insert_user(&mut tx, user, &email, &name, &hash).await?;
    sqlx::query("INSERT INTO memberships(tenant_id,user_id,role) VALUES($1,$2,'viewer')")
        .bind(identity.tenant)
        .bind(user)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(StatusCode::CREATED)
}

fn confirm_password(password: &str, confirmation: &str) -> Result<()> {
    if password != confirmation {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "A confirmação de senha não confere.",
        ));
    }
    Ok(())
}
