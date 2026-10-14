use crate::{
    auth::{self, ApiError},
    AppState,
};
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use contas_contracts::{money, valid_date, Entry, EntryInput, Ledger};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{postgres::PgRow, Postgres, Row, Transaction};
use uuid::Uuid;
type Result<T> = std::result::Result<T, ApiError>;
fn bad(s: &'static str) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, s)
}
fn conflict(s: &'static str) -> ApiError {
    ApiError(StatusCode::CONFLICT, s)
}
fn missing() -> ApiError {
    ApiError(StatusCode::NOT_FOUND, "Registro não encontrado.")
}
fn owner(role: &str) -> Result<()> {
    if role != "owner" {
        return Err(ApiError(
            StatusCode::FORBIDDEN,
            "Somente o administrador pode alterar lançamentos.",
        ));
    }
    Ok(())
}
fn uuid(s: &str) -> Result<Uuid> {
    Uuid::parse_str(s).map_err(|_| bad("Identificador inválido."))
}
pub async fn lock(tx: &mut Transaction<'_, Postgres>, tenant: Uuid) -> Result<()> {
    // Same transaction lock for entries and catalogs: prevents direction/deactivation races.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(tenant.to_string())
        .execute(&mut **tx)
        .await?;
    Ok(())
}
fn entry(r: &PgRow) -> Entry {
    Entry {
        id: r.get::<Uuid, _>("id").to_string(),
        account_id: r.get::<Uuid, _>("account_id").to_string(),
        history_id: r.get::<Uuid, _>("history_id").to_string(),
        date: r.get("date"),
        document: r.get("document"),
        description: r.get("description"),
        amount: money::decimal(r.get::<i64, _>("amount_cents") as i128),
        direction: r.get("direction"),
        cleared: r.get("cleared"),
        archived: r.get("archived"),
        cancelled: r.get("cancelled"),
        version: r.get("version"),
        history_label: r.get("history_label"),
    }
}
const SELECT:&str="SELECT e.*,to_char(e.entry_date,'YYYY-MM-DD') AS date,h.code||' · '||h.description AS history_label FROM entries e JOIN histories h ON h.id=e.history_id AND h.tenant_id=e.tenant_id";
async fn fetch(tx: &mut Transaction<'_, Postgres>, id: Uuid) -> Result<Entry> {
    let r = sqlx::query(&format!("{SELECT} WHERE e.id=$1"))
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(missing)?;
    Ok(entry(&r))
}
pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Entry>> {
    let (mut tx, _) = auth::authorized(&state, &headers).await?;
    let e = fetch(&mut tx, id).await?;
    tx.commit().await?;
    Ok(Json(e))
}
pub async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<EntryInput>,
) -> Result<(StatusCode, Json<Entry>)> {
    let e = save(&state, &headers, None, input).await?;
    Ok((StatusCode::CREATED, Json(e)))
}
pub async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(input): Json<EntryInput>,
) -> Result<Json<Entry>> {
    Ok(Json(save(&state, &headers, Some(id), input).await?))
}
async fn save(
    state: &AppState,
    headers: &HeaderMap,
    id: Option<Uuid>,
    input: EntryInput,
) -> Result<Entry> {
    auth::write_allowed(state, headers)?;
    let (mut tx, identity) = auth::authorized(state, headers).await?;
    owner(&identity.role)?;
    lock(&mut tx, identity.tenant).await?;
    let account = uuid(&input.account_id)?;
    let history = uuid(&input.history_id)?;
    if !valid_date(&input.date) {
        return Err(bad("Data inválida. Use uma data de calendário válida."));
    }
    let cents = money::parse_cents(&input.amount).map_err(bad)?;
    if cents <= 0 {
        return Err(bad(
            "O valor deve ser maior que zero. O sentido vem do histórico.",
        ));
    }
    if input.document.chars().count() > 60 || input.description.chars().count() > 500 {
        return Err(bad("Documento: até 60 caracteres. Complemento: até 500."));
    }
    let payload = json!({"account":account,"history":history,"date":input.date,"document":input.document.trim(),"description":input.description.trim(),"cents":cents,"cleared":input.cleared,"archived":input.archived});
    let request = if id.is_none() {
        if input.expected_version.is_some() {
            return Err(bad("Novo lançamento não deve informar versão."));
        }
        let request = uuid(
            input
                .request_id
                .as_deref()
                .ok_or_else(|| bad("Identificador de envio obrigatório."))?,
        )?;
        if let Some(existing) =
            sqlx::query("SELECT id,request_payload FROM entries WHERE request_id=$1")
                .bind(request)
                .fetch_optional(&mut *tx)
                .await?
        {
            if existing.get::<Value, _>("request_payload") != payload {
                return Err(conflict("Este envio já foi usado com outros dados. Atualize a lista antes de iniciar um novo lançamento."));
            }
            let e = fetch(&mut tx, existing.get("id")).await?;
            tx.commit().await?;
            return Ok(e);
        }
        Some(request)
    } else {
        if input.request_id.is_some() {
            return Err(bad("Edição não deve informar identificador de envio."));
        }
        None
    };
    let old = if let Some(id) = id {
        let old = fetch(&mut tx, id).await?;
        if old.cancelled {
            return Err(conflict("Lançamento cancelado não pode ser alterado."));
        }
        if input.expected_version != Some(old.version) {
            return Err(conflict(
                "Lançamento alterado em outra sessão. Descarte a edição e atualize a lista.",
            ));
        }
        Some(old)
    } else {
        None
    };
    let active: bool = sqlx::query_scalar("SELECT active FROM accounts WHERE id=$1")
        .bind(account)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(missing)?;
    let h = sqlx::query("SELECT active,direction FROM histories WHERE id=$1")
        .bind(history)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(missing)?;
    // Existing inactive references may be retained while correcting an old entry.
    if (!active
        && old
            .as_ref()
            .is_none_or(|e| e.account_id != input.account_id))
        || (!h.get::<bool, _>("active")
            && old
                .as_ref()
                .is_none_or(|e| e.history_id != input.history_id))
    {
        return Err(conflict("Novos vínculos exigem conta e histórico ativos."));
    }
    let direction: i16 = h.get("direction");
    let saved = if let Some(id) = id {
        sqlx::query("UPDATE entries SET account_id=$1,history_id=$2,entry_date=$3::text::date,document=$4,description=$5,amount_cents=$6,direction=$7,cleared=$8,archived=$9,version=version+1,updated_by=$10,updated_at=now() WHERE id=$11")
        .bind(account).bind(history).bind(&input.date).bind(input.document.trim()).bind(input.description.trim()).bind(cents).bind(direction).bind(input.cleared).bind(input.archived).bind(identity.user).bind(id).execute(&mut *tx).await?;
        id
    } else {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO entries(id,tenant_id,account_id,history_id,entry_date,document,description,amount_cents,direction,cleared,archived,request_id,request_payload,updated_by) VALUES($1,$2,$3,$4,$5::text::date,$6,$7,$8,$9,$10,$11,$12,$13,$14)")
        .bind(id).bind(identity.tenant).bind(account).bind(history).bind(&input.date).bind(input.document.trim()).bind(input.description.trim()).bind(cents).bind(direction).bind(input.cleared).bind(input.archived).bind(request.unwrap()).bind(payload).bind(identity.user).execute(&mut *tx).await?;
        id
    };
    let result = fetch(&mut tx, saved).await?;
    tx.commit().await?;
    Ok(result)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cancel {
    expected_version: i64,
}
pub async fn cancel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(input): Json<Cancel>,
) -> Result<Json<Entry>> {
    auth::write_allowed(&state, &headers)?;
    let (mut tx, identity) = auth::authorized(&state, &headers).await?;
    owner(&identity.role)?;
    lock(&mut tx, identity.tenant).await?;
    let old = fetch(&mut tx, id).await?;
    if old.version != input.expected_version {
        return Err(conflict(
            "Lançamento alterado em outra sessão. Atualize a lista.",
        ));
    }
    if !old.cancelled {
        sqlx::query("UPDATE entries SET cancelled=true,version=version+1,updated_by=$1,updated_at=now() WHERE id=$2").bind(identity.user).bind(id).execute(&mut *tx).await?;
    }
    let e = fetch(&mut tx, id).await?;
    tx.commit().await?;
    Ok(Json(e))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Filter {
    pub account_id: Uuid,
    pub from: Option<String>,
    pub to: Option<String>,
    #[serde(default)]
    pub include_archived: bool,
    #[serde(default)]
    pub include_cancelled: bool,
    #[serde(default)]
    pub offset: usize,
}
pub async fn ledger(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filter): Query<Filter>,
) -> Result<Json<Ledger>> {
    let from = filter.from.as_deref().unwrap_or("0001-01-01");
    let to = filter.to.as_deref().unwrap_or("9999-12-31");
    if !valid_date(from) || !valid_date(to) || from > to {
        return Err(bad(
            "Período inválido. A data inicial deve ser anterior ou igual à final.",
        ));
    }
    let (mut tx, _) = auth::authorized(&state, &headers).await?;
    // Totals and the limited page share one PostgreSQL statement/snapshot.
    let offset = i64::try_from(filter.offset).map_err(|_| bad("Página inválida."))?;
    let row = sqlx::query(include_str!("ledger.sql"))
        .bind(filter.account_id)
        .bind(from)
        .bind(to)
        .bind(filter.include_archived)
        .bind(filter.include_cancelled)
        .bind(offset)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(missing)?;
    let initial = row.get::<i64, _>("opening_balance_cents") as i128;
    let current = initial + number(&row, "movement")?;
    let bank = initial + number(&row, "bank")?;
    let previous = initial + number(&row, "previous")?;
    let income = number(&row, "income")?;
    let expense = number(&row, "expense")?;
    let total = row.get::<i64, _>("total") as usize;
    let items: Vec<Entry> = serde_json::from_value(row.get("items"))
        .map_err(|_| ApiError(StatusCode::SERVICE_UNAVAILABLE, "Extrato indisponível."))?;
    tx.commit().await?;
    let next = filter.offset.saturating_add(items.len());
    Ok(Json(Ledger {
        items,
        initial_balance: money::decimal(initial),
        previous_balance: money::decimal(previous),
        period_income: money::decimal(income),
        period_expense: money::decimal(expense),
        period_balance: money::decimal(previous + income - expense),
        current_balance: money::decimal(current),
        bank_balance: money::decimal(bank),
        total,
        next_offset: if next < total { Some(next) } else { None },
    }))
}

pub(crate) fn number(row: &PgRow, name: &str) -> Result<i128> {
    row.get::<String, _>(name).parse().map_err(|_| {
        ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "Total fora do limite permitido.",
        )
    })
}
