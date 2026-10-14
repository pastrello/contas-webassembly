use super::*;
use contas_contracts::Report;
pub fn clear() {
    for id in ["report-head", "report-body", "report-context"] {
        element(id).set_text_content(None);
    }
    element("report-csv").remove_attribute("href").unwrap();
    visible("report-csv", false);
}
async fn load() -> Result<(), String> {
    let kind = element("report-kind")
        .dyn_into::<web_sys::HtmlSelectElement>()
        .unwrap()
        .value();
    let mut query = format!("kind={kind}");
    for (id, name) in [("report-from", "from"), ("report-to", "to")] {
        let date = input(id);
        if !date.is_empty() {
            if !contas_contracts::valid_date(&date) {
                return Err("Data inválida.".into());
            }
            query.push_str(&format!("&{name}={date}"));
        }
    }
    let report: Report =
        serde_json::from_value(request("GET", &format!("/api/reports?{query}"), None).await?)
            .map_err(|_| "Relatório inválido.")?;
    clear();
    show("report-context",&format!("{} · {} a {} · Todos os cadastros, incluindo inativos. Arquivados incluídos; cancelados excluídos.",if report.kind=="accounts"{"Resumo por conta"}else{"Totais por histórico"},report.from,report.to));
    let row = document().create_element("tr").unwrap();
    for column in report.columns {
        let th = document().create_element("th").unwrap();
        th.set_text_content(Some(&column));
        row.append_child(&th).unwrap();
    }
    element("report-head").append_child(&row).unwrap();
    for values in report.rows {
        let tr = document().create_element("tr").unwrap();
        for value in values {
            let td = document().create_element("td").unwrap();
            td.set_text_content(Some(&value));
            tr.append_child(&td).unwrap();
        }
        element("report-body").append_child(&tr).unwrap();
    }
    element("report-csv")
        .set_attribute("href", &format!("/api/reports/export?{query}"))
        .unwrap();
    visible("report-csv", true);
    Ok(())
}
pub fn init() {
    let handler = Closure::<dyn FnMut(web_sys::Event)>::new(|event: web_sys::Event| {
        event.prevent_default();
        if BUSY.with(|b| b.get()) {
            return;
        }
        busy(true);
        show("message", "");
        spawn_local(async {
            if let Err(error) = load().await {
                show("message", &error);
            }
            busy(false);
        });
    });
    element("report-form")
        .add_event_listener_with_callback("submit", handler.as_ref().unchecked_ref())
        .unwrap();
    handler.forget();
}
