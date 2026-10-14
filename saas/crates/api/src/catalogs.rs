use crate::{
    auth::{self, ApiError},
    AppState,
};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use contas_contracts::{money, Account, AccountInput, History, HistoryInput};
use serde_json::{json, Value};
use sqlx::{postgres::PgRow, Row};
use uuid::Uuid;
type Result<T> = std::result::Result<T, ApiError>;
fn bad(message: &'static str) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, message)
}
fn text(value: &str, max: usize, required: bool) -> Result<String> {
    let value = value.trim();
    if (required && value.is_empty()) || value.chars().count() > max {
        return Err(bad(
            "Confira os campos obrigatórios e seus limites de tamanho.",
        ));
    }
    Ok(value.to_owned())
}
fn code(value: &str) -> Result<String> {
    let value = value.trim().to_ascii_uppercase();
    if value.is_empty()
        || value.len() > 20
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
    {
        return Err(bad(
            "Código: use até 20 letras, números, ponto, hífen ou sublinhado.",
        ));
    }
    Ok(if value.len() == 1 {
        format!("0{value}")
    } else {
        value
    })
}
fn owner(role: &str) -> Result<()> {
    if role != "owner" {
        return Err(ApiError(
            StatusCode::FORBIDDEN,
            "Somente o administrador pode alterar cadastros.",
        ));
    }
    Ok(())
}
fn conflict() -> ApiError {
    ApiError(StatusCode::CONFLICT,"O registro foi alterado em outra sessão. Cancele a edição e atualize a lista antes de tentar novamente.")
}
fn missing() -> ApiError {
    ApiError(StatusCode::NOT_FOUND, "Registro não encontrado.")
}
fn db(error: sqlx::Error) -> ApiError {
    if error
        .as_database_error()
        .is_some_and(|e| e.is_unique_violation())
    {
        return ApiError(
            StatusCode::CONFLICT,
            "Esse código já está cadastrado neste ambiente, inclusive entre os inativos.",
        );
    }
    error.into()
}
fn account(row: PgRow) -> Account {
    Account {
        id: row.get::<Uuid, _>("id").to_string(),
        code: row.get("code"),
        number: row.get("number"),
        name: row.get("name"),
        opening_balance: money::decimal(row.get::<i64, _>("opening_balance_cents") as i128),
        active: row.get("active"),
        version: row.get("version"),
    }
}
fn history(row: PgRow) -> History {
    History {
        id: row.get::<Uuid, _>("id").to_string(),
        code: row.get("code"),
        description: row.get("description"),
        direction: row.get("direction"),
        active: row.get("active"),
        version: row.get("version"),
    }
}
pub async fn accounts(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<Value>> {
    let (mut tx, _) = auth::authorized(&state, &headers).await?;
    let rows=sqlx::query("SELECT id,code,number,name,opening_balance_cents,active,version FROM accounts ORDER BY code,name").fetch_all(&mut *tx).await?;
    let total: i128 = rows
        .iter()
        .filter(|r| r.get::<bool, _>("active"))
        .map(|r| r.get::<i64, _>("opening_balance_cents") as i128)
        .sum();
    let items: Vec<Account> = rows.into_iter().map(account).collect();
    tx.commit().await?;
    Ok(Json(
        json!({"items":items,"active_opening_balance":money::decimal(total)}),
    ))
}
pub async fn get_account(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Account>> {
    let (mut tx, _) = auth::authorized(&state, &headers).await?;
    let row = sqlx::query(
        "SELECT id,code,number,name,opening_balance_cents,active,version FROM accounts WHERE id=$1",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(missing)?;
    tx.commit().await?;
    Ok(Json(account(row)))
}
pub async fn create_account(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<AccountInput>,
) -> Result<(StatusCode, Json<Account>)> {
    Ok((
        StatusCode::CREATED,
        Json(save_account(&state, &headers, None, input).await?),
    ))
}
pub async fn update_account(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(input): Json<AccountInput>,
) -> Result<Json<Account>> {
    Ok(Json(save_account(&state, &headers, Some(id), input).await?))
}
async fn save_account(
    state: &AppState,
    headers: &HeaderMap,
    id: Option<Uuid>,
    input: AccountInput,
) -> Result<Account> {
    auth::write_allowed(state, headers)?;
    let (mut tx, identity) = auth::authorized(state, headers).await?;
    owner(&identity.role)?;
    crate::entries::lock(&mut tx, identity.tenant).await?;
    let code = code(&input.code)?;
    let number = text(&input.number, 40, false)?;
    let name = text(&input.name, 100, true)?;
    let cents = money::parse_cents(&input.opening_balance).map_err(bad)?;
    let row = if let Some(id) = id {
        let version = input
            .expected_version
            .filter(|v| *v > 0)
            .ok_or_else(|| bad("Informe a versão do registro para editar."))?;
        let current: Option<i64> =
            sqlx::query_scalar("SELECT version FROM accounts WHERE id=$1 FOR UPDATE")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        if current.ok_or_else(missing)? != version {
            return Err(conflict());
        }
        let used: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM entries WHERE account_id=$1)")
                .bind(id)
                .fetch_one(&mut *tx)
                .await?;
        let old: i64 = sqlx::query_scalar("SELECT opening_balance_cents FROM accounts WHERE id=$1")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
        if used && old != cents {
            return Err(ApiError(StatusCode::CONFLICT,"Conta com lançamentos: saldo inicial não pode ser alterado. Registre um lançamento de ajuste."));
        }
        sqlx::query("UPDATE accounts SET code=$1,number=$2,name=$3,opening_balance_cents=$4,active=$5,version=version+1,updated_by=$6,updated_at=now() WHERE id=$7 RETURNING id,code,number,name,opening_balance_cents,active,version")
            .bind(code).bind(number).bind(name).bind(cents).bind(input.active).bind(identity.user).bind(id).fetch_one(&mut *tx).await.map_err(db)?
    } else {
        if input.expected_version.is_some() {
            return Err(bad("Cadastro novo não deve informar versão."));
        }
        sqlx::query("INSERT INTO accounts(id,tenant_id,code,number,name,opening_balance_cents,active,updated_by) VALUES($1,$2,$3,$4,$5,$6,$7,$8) RETURNING id,code,number,name,opening_balance_cents,active,version")
            .bind(Uuid::new_v4()).bind(identity.tenant).bind(code).bind(number).bind(name).bind(cents).bind(input.active).bind(identity.user).fetch_one(&mut *tx).await.map_err(db)?
    };
    tx.commit().await?;
    Ok(account(row))
}
pub async fn histories(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<History>>> {
    let (mut tx, _) = auth::authorized(&state, &headers).await?;
    let rows=sqlx::query("SELECT id,code,description,direction,active,version FROM histories ORDER BY code,description").fetch_all(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(rows.into_iter().map(history).collect()))
}
pub async fn get_history(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<History>> {
    let (mut tx, _) = auth::authorized(&state, &headers).await?;
    let row = sqlx::query(
        "SELECT id,code,description,direction,active,version FROM histories WHERE id=$1",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(missing)?;
    tx.commit().await?;
    Ok(Json(history(row)))
}
pub async fn create_history(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<HistoryInput>,
) -> Result<(StatusCode, Json<History>)> {
    Ok((
        StatusCode::CREATED,
        Json(save_history(&state, &headers, None, input).await?),
    ))
}
pub async fn update_history(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(input): Json<HistoryInput>,
) -> Result<Json<History>> {
    Ok(Json(save_history(&state, &headers, Some(id), input).await?))
}
async fn save_history(
    state: &AppState,
    headers: &HeaderMap,
    id: Option<Uuid>,
    input: HistoryInput,
) -> Result<History> {
    auth::write_allowed(state, headers)?;
    let (mut tx, identity) = auth::authorized(state, headers).await?;
    owner(&identity.role)?;
    crate::entries::lock(&mut tx, identity.tenant).await?;
    let code = code(&input.code)?;
    let description = text(&input.description, 100, true)?;
    if ![-1, 1].contains(&input.direction) {
        return Err(bad("Selecione Entrada ou Saída."));
    }
    let row = if let Some(id) = id {
        let version = input
            .expected_version
            .filter(|v| *v > 0)
            .ok_or_else(|| bad("Informe a versão do registro para editar."))?;
        let current: Option<i64> =
            sqlx::query_scalar("SELECT version FROM histories WHERE id=$1 FOR UPDATE")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        if current.ok_or_else(missing)? != version {
            return Err(conflict());
        }
        let used: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM entries WHERE history_id=$1)")
                .bind(id)
                .fetch_one(&mut *tx)
                .await?;
        let old: i16 = sqlx::query_scalar("SELECT direction FROM histories WHERE id=$1")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
        if used && old != input.direction {
            return Err(ApiError(
                StatusCode::CONFLICT,
                "Histórico com lançamentos: o sentido não pode ser alterado.",
            ));
        }
        sqlx::query("UPDATE histories SET code=$1,description=$2,direction=$3,active=$4,version=version+1,updated_by=$5,updated_at=now() WHERE id=$6 RETURNING id,code,description,direction,active,version")
            .bind(code).bind(description).bind(input.direction).bind(input.active).bind(identity.user).bind(id).fetch_one(&mut *tx).await.map_err(db)?
    } else {
        if input.expected_version.is_some() {
            return Err(bad("Cadastro novo não deve informar versão."));
        }
        sqlx::query("INSERT INTO histories(id,tenant_id,code,description,direction,active,updated_by) VALUES($1,$2,$3,$4,$5,$6,$7) RETURNING id,code,description,direction,active,version")
            .bind(Uuid::new_v4()).bind(identity.tenant).bind(code).bind(description).bind(input.direction).bind(input.active).bind(identity.user).fetch_one(&mut *tx).await.map_err(db)?
    };
    tx.commit().await?;
    Ok(history(row))
}
