use crate::{
    auth::{self, ApiError},
    entries::number,
    AppState,
};
use axum::{
    extract::{Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use contas_contracts::{money, valid_date, Report};
use serde::Deserialize;
use sqlx::Row;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Filter {
    kind: String,
    from: Option<String>,
    to: Option<String>,
}
type Result<T> = std::result::Result<T, ApiError>;
pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filter): Query<Filter>,
) -> Result<Json<Report>> {
    Ok(Json(generate(&state, &headers, filter).await?))
}
pub async fn export(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filter): Query<Filter>,
) -> Result<Response> {
    let report = generate(&state, &headers, filter).await?;
    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"contas-relatorio.csv\"",
            ),
        ],
        csv(&report),
    )
        .into_response())
}
fn money(cents: i128) -> String {
    money::decimal(cents).replace('.', ",")
}
async fn generate(state: &AppState, headers: &HeaderMap, filter: Filter) -> Result<Report> {
    let from = filter.from.unwrap_or_else(|| "0001-01-01".into());
    let to = filter.to.unwrap_or_else(|| "9999-12-31".into());
    if !valid_date(&from) || !valid_date(&to) || from > to {
        return Err(ApiError(StatusCode::BAD_REQUEST, "Período inválido."));
    }
    let (sql, columns, fields) = match filter.kind.as_str() {
        "accounts" => (
            include_str!("report_accounts.sql"),
            vec![
                "Código",
                "Conta",
                "Situação",
                "Inicial (R$)",
                "Anterior (R$)",
                "Entradas (R$)",
                "Saídas (R$)",
                "Final (R$)",
                "Bancário até o fim (R$)",
            ],
            vec![
                "initial", "previous", "income", "expense", "closing", "bank",
            ],
        ),
        "histories" => (
            include_str!("report_histories.sql"),
            vec![
                "Código",
                "Histórico",
                "Situação",
                "Entradas (R$)",
                "Saídas (R$)",
                "Resultado (R$)",
            ],
            vec!["income", "expense", "net"],
        ),
        _ => {
            return Err(ApiError(
                StatusCode::BAD_REQUEST,
                "Selecione Contas ou Históricos.",
            ))
        }
    };
    let (mut tx, _) = auth::authorized(state, headers).await?;
    let records = sqlx::query(sql)
        .bind(&from)
        .bind(&to)
        .fetch_all(&mut *tx)
        .await?;
    let mut rows = Vec::new();
    let mut totals = vec![0i128; fields.len()];
    for row in records {
        let mut values = vec![
            row.get("code"),
            row.get("label"),
            if row.get::<bool, _>("active") {
                "Ativo".into()
            } else {
                "Inativo".into()
            },
        ];
        for (i, field) in fields.iter().enumerate() {
            let value = number(&row, field)?;
            totals[i] += value;
            values.push(money(value));
        }
        rows.push(values);
    }
    let mut total = vec!["TOTAL".into(), "".into(), "".into()];
    total.extend(totals.into_iter().map(money));
    rows.push(total);
    tx.commit().await?;
    Ok(Report {
        kind: filter.kind,
        from,
        to,
        columns: columns.into_iter().map(str::to_owned).collect(),
        rows,
    })
}
fn field(value: &str) -> String {
    // Spreadsheet formula injection: whitespace may precede a formula marker.
    let trimmed = value.trim_start();
    let dangerous =
        trimmed.starts_with(['=', '+', '-', '@']) || value.starts_with(['\t', '\r', '\n']);
    let prefix = if dangerous { "'" } else { "" };
    format!("\"{}{}\"", prefix, value.replace('"', "\"\""))
}
fn csv(report: &Report) -> String {
    let mut out = String::from("\u{feff}");
    for row in [
        vec!["Relatório".into(), report.kind.clone()],
        vec![
            "De".into(),
            report.from.clone(),
            "Até".into(),
            report.to.clone(),
        ],
        report.columns.clone(),
    ]
    .into_iter()
    .chain(report.rows.clone())
    {
        out.push_str(&row.iter().map(|s| field(s)).collect::<Vec<_>>().join(";"));
        out.push_str("\r\n");
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn csv_escapes_labels_and_formula_prefixes() {
        assert_eq!(field("Banco \"A\"; Sul"), "\"Banco \"\"A\"\"; Sul\"");
        for value in ["=1+1", " +SUM(A1)", "-2", "@cmd", "\t=1", "\n=1"] {
            assert!(field(value).starts_with("\"'"));
        }
        assert_eq!(field("123,45"), "\"123,45\"");
        let r = Report {
            kind: "accounts".into(),
            from: "2026-01-01".into(),
            to: "2026-01-31".into(),
            columns: vec!["Nome".into()],
            rows: vec![vec!["São Paulo\nBanco".into()]],
        };
        assert!(csv(&r).starts_with('\u{feff}'));
        assert!(csv(&r).contains("\"São Paulo\nBanco\"\r\n"));
    }
}
