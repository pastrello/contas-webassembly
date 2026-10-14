use super::*;
use contas_contracts::{Account, Entry, EntryInput, History, Ledger};
use std::cell::RefCell;
#[derive(Default)]
struct State {
    owner: bool,
    edit: Option<Entry>,
    dirty: bool,
    key: Option<String>,
    items: Vec<Entry>,
    offset: usize,
    next: Option<usize>,
}
thread_local! {static DATA:RefCell<State>=RefCell::new(State::default());}
pub fn dirty() -> bool {
    DATA.with(|d| d.borrow().dirty)
}
fn select(id: &str) -> web_sys::HtmlSelectElement {
    element(id).dyn_into().unwrap()
}
fn set(id: &str, value: &str) {
    element(id)
        .dyn_into::<web_sys::HtmlInputElement>()
        .unwrap()
        .set_value(value);
}
fn check(id: &str, value: bool) {
    element(id)
        .dyn_into::<web_sys::HtmlInputElement>()
        .unwrap()
        .set_checked(value);
}
fn checked(id: &str) -> bool {
    element(id)
        .dyn_into::<web_sys::HtmlInputElement>()
        .unwrap()
        .checked()
}
fn bind(id: &str, event: &str, cb: impl FnMut(web_sys::Event) + 'static) {
    let cb = Closure::<dyn FnMut(web_sys::Event)>::new(cb);
    element(id)
        .add_event_listener_with_callback(event, cb.as_ref().unchecked_ref())
        .unwrap();
    cb.forget();
}
fn option(id: &str, value: &str, label: &str) {
    let o = document().create_element("option").unwrap();
    o.set_attribute("value", value).unwrap();
    o.set_text_content(Some(label));
    element(id).append_child(&o).unwrap();
}
fn message(s: &str) {
    show("message", s);
}
fn reset() {
    element("entry-form")
        .dyn_into::<web_sys::HtmlFormElement>()
        .unwrap()
        .reset();
    select("entry-account").set_value(&select("ledger-account").value());
    DATA.with(|d| {
        let mut d = d.borrow_mut();
        d.edit = None;
        d.dirty = false;
        d.key = None;
    });
    show("entry-title", "Novo lançamento");
}
pub fn clear() {
    DATA.with(|d| *d.borrow_mut() = State::default());
    show("ledger-context", "");
    reset();
    for id in [
        "entries-body",
        "ledger-totals",
        "ledger-count",
        "ledger-account",
        "entry-account",
        "entry-history",
    ] {
        element(id).set_text_content(None);
    }
}
pub async fn load(owner: bool, accounts: &[Account], histories: &[History]) -> Result<(), String> {
    if dirty() {
        return Err(
            "Há um lançamento em edição. Salve ou descarte antes de atualizar as listas.".into(),
        );
    }
    DATA.with(|d| {
        let mut d = d.borrow_mut();
        d.owner = owner;
        d.offset = 0;
    });
    visible("entry-editor", owner);
    let old = select("ledger-account").value();
    for id in ["ledger-account", "entry-account", "entry-history"] {
        element(id).set_text_content(None);
    }
    for a in accounts {
        let label = format!(
            "{} · {}{}",
            a.code,
            a.name,
            if a.active { "" } else { " (inativa)" }
        );
        option("ledger-account", &a.id, &label);
        option("entry-account", &a.id, &label);
    }
    for h in histories {
        option(
            "entry-history",
            &h.id,
            &format!(
                "{} · {} ({}){}",
                h.code,
                h.description,
                if h.direction == 1 {
                    "Entrada"
                } else {
                    "Saída"
                },
                if h.active { "" } else { " — inativo" }
            ),
        );
    }
    if accounts.iter().any(|a| a.id == old) {
        select("ledger-account").set_value(&old);
    }
    reset();
    reload().await
}
async fn reload() -> Result<(), String> {
    let account = select("ledger-account").value();
    if account.is_empty() {
        element("entries-body").set_text_content(None);
        show("ledger-count", "Cadastre uma conta para começar.");
        show("ledger-totals", "");
        show("ledger-context", "");
        return Ok(());
    }
    let mut url = format!(
        "/api/entries?account_id={}&include_archived={}&include_cancelled={}&offset={}",
        account,
        checked("ledger-archived"),
        checked("ledger-cancelled"),
        DATA.with(|d| d.borrow().offset)
    );
    for (id, name) in [("ledger-from", "from"), ("ledger-to", "to")] {
        let date = input(id);
        if !date.is_empty() {
            if !contas_contracts::valid_date(&date) {
                return Err("Data inválida.".into());
            }
            url.push_str(&format!("&{name}={date}"));
        }
    }
    let value = request("GET", &url, None).await?;
    let ledger: Ledger = serde_json::from_value(value).map_err(|_| "Extrato inválido.")?;
    let label = element("ledger-account")
        .query_selector("option:checked")
        .ok()
        .flatten()
        .and_then(|e| e.text_content())
        .unwrap_or_default();
    show(
        "ledger-context",
        &format!(
            "Consulta exibida: {} · De {} até {}. Novos filtros são aplicados ao clicar Consultar.",
            label,
            if input("ledger-from").is_empty() {
                "o início".into()
            } else {
                input("ledger-from")
            },
            if input("ledger-to").is_empty() {
                "o fim".into()
            } else {
                input("ledger-to")
            }
        ),
    );
    let brl = |s: &str| format!("R$ {}", s.replace('.', ","));
    show("ledger-totals",&format!("Saldo inicial: {}\nAnterior ao período: {}\nEntradas no período: {} · Saídas: {}\nSaldo ao fim do período: {}\nSaldo real (todas as datas): {} · Bancário (compensados): {}",brl(&ledger.initial_balance),brl(&ledger.previous_balance),brl(&ledger.period_income),brl(&ledger.period_expense),brl(&ledger.period_balance),brl(&ledger.current_balance),brl(&ledger.bank_balance)));
    DATA.with(|d| {
        let mut d = d.borrow_mut();
        d.items = ledger.items;
        d.next = ledger.next_offset;
    });
    let body = element("entries-body");
    body.set_text_content(None);
    DATA.with(|d| {
        let d = d.borrow();
        show(
            "ledger-count",
            &format!(
                "{} lançamento(s) neste filtro. Página {}.",
                ledger.total,
                d.offset / 200 + 1
            ),
        );
        for e in &d.items {
            let row = document().create_element("tr").unwrap();
            for value in [
                e.date.as_str(),
                &e.history_label,
                &e.document,
                &e.description,
                &format!(
                    "{} {}",
                    if e.direction == 1 { "+" } else { "−" },
                    brl(&e.amount)
                ),
                if e.cancelled {
                    "Cancelado"
                } else if e.archived {
                    "Arquivado"
                } else {
                    "Ativo"
                },
                if e.cleared { "Sim" } else { "Não" },
            ] {
                let td = document().create_element("td").unwrap();
                td.set_text_content(Some(value));
                row.append_child(&td).unwrap();
            }
            let td = document().create_element("td").unwrap();
            if d.owner && !e.cancelled {
                for (action, label) in [("edit", "Editar"), ("cancel", "Cancelar lançamento")] {
                    let button = document().create_element("button").unwrap();
                    button.set_attribute("type", "button").unwrap();
                    button.set_attribute("data-entry", &e.id).unwrap();
                    button.set_attribute("data-action", action).unwrap();
                    button.set_text_content(Some(label));
                    td.append_child(&button).unwrap();
                }
            }
            row.append_child(&td).unwrap();
            body.append_child(&row).unwrap();
        }
    });
    Ok(())
}
fn edit(e: Entry) {
    select("entry-account").set_value(&e.account_id);
    select("entry-history").set_value(&e.history_id);
    for (id, value) in [
        ("entry-date", &e.date),
        ("entry-document", &e.document),
        ("entry-description", &e.description),
        ("entry-amount", &e.amount.replace('.', ",")),
    ] {
        set(id, value);
    }
    check("entry-cleared", e.cleared);
    check("entry-archived", e.archived);
    show(
        "entry-title",
        "Editar lançamento (alterações somente ao salvar)",
    );
    DATA.with(|d| {
        let mut d = d.borrow_mut();
        d.edit = Some(e);
        d.dirty = false;
        d.key = None;
    });
}
fn new_key() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    web_sys::window()
        .unwrap()
        .crypto()
        .map_err(|_| "Criptografia indisponível.")?
        .get_random_values_with_u8_array(&mut bytes)
        .map_err(|_| "Falha ao preparar envio.")?;
    bytes[6] = (bytes[6] & 15) | 64;
    bytes[8] = (bytes[8] & 63) | 128;
    let h: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &h[..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..]
    ))
}
async fn save() -> Result<(), String> {
    let editing = DATA.with(|d| d.borrow().edit.clone());
    let key = if editing.is_none() {
        let existing = DATA.with(|d| d.borrow().key.clone());
        let key = match existing {
            Some(k) => k,
            None => new_key()?,
        };
        DATA.with(|d| d.borrow_mut().key = Some(key.clone()));
        Some(key)
    } else {
        None
    };
    let body = EntryInput {
        account_id: select("entry-account").value(),
        history_id: select("entry-history").value(),
        date: input("entry-date"),
        document: input("entry-document"),
        description: input("entry-description"),
        amount: input("entry-amount"),
        cleared: checked("entry-cleared"),
        archived: checked("entry-archived"),
        request_id: key,
        expected_version: editing.as_ref().map(|e| e.version),
    };
    let url = editing.map(|e| format!("/api/entries/{}", e.id));
    request(
        if url.is_some() { "PUT" } else { "POST" },
        url.as_deref().unwrap_or("/api/entries"),
        Some(serde_json::to_value(body).unwrap()),
    )
    .await?;
    reset();
    message("Lançamento salvo.");
    if reload().await.is_err() {
        message("Lançamento salvo. A lista não pôde ser atualizada; use Consultar antes de iniciar outro.");
    }
    Ok(())
}
fn may_leave() -> bool {
    if dirty() {
        message("Salve ou descarte a edição antes de consultar ou selecionar outro lançamento.");
        false
    } else {
        true
    }
}
pub fn init() {
    bind("entry-form", "input", |_| {
        DATA.with(|d| d.borrow_mut().dirty = true)
    });
    bind("entry-form", "submit", |event| {
        event.prevent_default();
        if BUSY.with(|b| b.get()) {
            return;
        }
        busy(true);
        message("");
        spawn_local(async {
            if let Err(e) = save().await {
                message(&e);
            }
            busy(false);
        });
    });
    bind("entry-reset", "click", |_| {
        if dirty()&&!web_sys::window().unwrap().confirm_with_message("Descartar as alterações não salvas? Se houve falha de conexão, consulte a lista antes de criar outro lançamento.").unwrap_or(false){return;}
        reset();
        message("");
    });
    bind("ledger-form", "submit", |event| {
        event.prevent_default();
        if BUSY.with(|b| b.get()) || !may_leave() {
            return;
        }
        DATA.with(|d| d.borrow_mut().offset = 0);
        reset();
        busy(true);
        spawn_local(async {
            if let Err(e) = reload().await {
                message(&e);
            } else {
                message("");
            }
            busy(false);
        });
    });
    for (id, next) in [("entries-prev", false), ("entries-next", true)] {
        bind(id, "click", move |_| {
            if BUSY.with(|b| b.get()) || !may_leave() {
                return;
            }
            let go = DATA.with(|d| {
                let mut d = d.borrow_mut();
                if next {
                    if let Some(n) = d.next {
                        d.offset = n;
                        true
                    } else {
                        false
                    }
                } else if d.offset > 0 {
                    d.offset = d.offset.saturating_sub(200);
                    true
                } else {
                    false
                }
            });
            if !go {
                return;
            }
            busy(true);
            spawn_local(async {
                if let Err(e) = reload().await {
                    message(&e);
                }
                busy(false);
            });
        });
    }
    bind("entries-body", "click", |event| {
        if BUSY.with(|b| b.get()) || !may_leave() {
            return;
        }
        let Some(button) = event
            .target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
            .and_then(|e| e.closest("button[data-entry]").ok().flatten())
        else {
            return;
        };
        let id = button.get_attribute("data-entry").unwrap();
        let Some(entry) = DATA.with(|d| d.borrow().items.iter().find(|e| e.id == id).cloned())
        else {
            return;
        };
        if button.get_attribute("data-action").as_deref() == Some("edit") {
            edit(entry);
            return;
        }
        if !web_sys::window().unwrap().confirm_with_message("Cancelar este lançamento? Ele deixará de compor os saldos e ficará preservado como cancelado. Esta ação não pode ser desfeita.").unwrap_or(false){return;}
        busy(true);
        spawn_local(async move {
            match request(
                "POST",
                &format!("/api/entries/{}/cancel", entry.id),
                Some(json!({"expected_version":entry.version})),
            )
            .await
            {
                Ok(_) => {
                    reset();
                    message("Lançamento cancelado.");
                    if reload().await.is_err() {
                        message("Cancelamento salvo. Atualize a consulta.");
                    }
                }
                Err(e) => message(&e),
            }
            busy(false);
        });
    });
}
