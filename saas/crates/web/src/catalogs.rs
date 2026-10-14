use super::*;
use contas_contracts::{money, Account, AccountInput, History, HistoryInput};
use std::cell::RefCell;
#[derive(Default)]
struct CatalogState {
    accounts: Vec<Account>,
    histories: Vec<History>,
    owner: bool,
    account_edit: Option<Account>,
    history_edit: Option<History>,
    account_dirty: bool,
    history_dirty: bool,
}
thread_local! {static DATA:RefCell<CatalogState>=RefCell::new(CatalogState::default());}
fn set_input(id: &str, value: &str) {
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
fn brl(value: &str) -> String {
    format!("R$ {}", value.replace('.', ","))
}
pub fn clear() {
    DATA.with(|d| *d.borrow_mut() = CatalogState::default());
    element("accounts-body").set_text_content(None);
    element("histories-body").set_text_content(None);
    reset_account();
    reset_history();
    show("opening-total", "R$ 0,00");
}
pub async fn load(owner: bool) -> Result<(), String> {
    DATA.with(|d| d.borrow_mut().owner = owner);
    visible("account-editor", owner);
    visible("history-editor", owner);
    let accounts = request("GET", "/api/accounts", None).await?;
    let histories = request("GET", "/api/histories", None).await?;
    let items: Vec<Account> = serde_json::from_value(accounts["items"].clone())
        .map_err(|_| "Resposta de contas inválida.")?;
    let history_items: Vec<History> =
        serde_json::from_value(histories).map_err(|_| "Resposta de históricos inválida.")?;
    DATA.with(|d| {
        let mut d = d.borrow_mut();
        d.accounts = items;
        d.histories = history_items;
        d.owner = owner;
    });
    show(
        "opening-total",
        &brl(accounts["active_opening_balance"]
            .as_str()
            .unwrap_or("0.00")),
    );
    visible("account-editor", owner);
    visible("history-editor", owner);
    render();
    Ok(())
}
fn cell(row: &web_sys::Element, value: &str) {
    let td = document().create_element("td").unwrap();
    td.set_text_content(Some(value));
    row.append_child(&td).unwrap();
}
fn edit_button(row: &web_sys::Element, id: &str, label: &str) {
    let td = document().create_element("td").unwrap();
    let button = document().create_element("button").unwrap();
    button.set_attribute("type", "button").unwrap();
    button.set_attribute("data-edit", id).unwrap();
    button.set_attribute("aria-label", label).unwrap();
    button.set_text_content(Some("Editar"));
    td.append_child(&button).unwrap();
    row.append_child(&td).unwrap();
}
fn render() {
    let all_accounts = checked("accounts-inactive");
    let all_histories = checked("histories-inactive");
    let account_body = element("accounts-body");
    let history_body = element("histories-body");
    account_body.set_text_content(None);
    history_body.set_text_content(None);
    DATA.with(|d| {
        let d = d.borrow();
        let mut count = 0;
        for a in d.accounts.iter().filter(|a| all_accounts || a.active) {
            let row = document().create_element("tr").unwrap();
            cell(&row, &a.code);
            cell(&row, &a.number);
            cell(&row, &a.name);
            cell(&row, &brl(&a.opening_balance));
            cell(&row, if a.active { "Ativa" } else { "Inativa" });
            if d.owner {
                edit_button(&row, &a.id, &format!("Editar conta {}", a.code));
            } else {
                cell(&row, "Consulta");
            }
            account_body.append_child(&row).unwrap();
            count += 1;
        }
        show(
            "accounts-empty",
            if count == 0 {
                "Nenhuma conta neste filtro."
            } else {
                ""
            },
        );
        count = 0;
        for h in d.histories.iter().filter(|h| all_histories || h.active) {
            let row = document().create_element("tr").unwrap();
            cell(&row, &h.code);
            cell(&row, &h.description);
            cell(
                &row,
                if h.direction == 1 {
                    "Entrada"
                } else {
                    "Saída"
                },
            );
            cell(&row, if h.active { "Ativo" } else { "Inativo" });
            if d.owner {
                edit_button(&row, &h.id, &format!("Editar histórico {}", h.code));
            } else {
                cell(&row, "Consulta");
            }
            history_body.append_child(&row).unwrap();
            count += 1;
        }
        show(
            "histories-empty",
            if count == 0 {
                "Nenhum histórico neste filtro."
            } else {
                ""
            },
        );
    });
}
fn reset_account() {
    element("account-form")
        .dyn_into::<web_sys::HtmlFormElement>()
        .unwrap()
        .reset();
    set_input("account-balance", "0,00");
    check("account-active", true);
    DATA.with(|d| {
        let mut d = d.borrow_mut();
        d.account_edit = None;
        d.account_dirty = false;
    });
    show("account-title", "Nova conta");
}
fn reset_history() {
    element("history-form")
        .dyn_into::<web_sys::HtmlFormElement>()
        .unwrap()
        .reset();
    check("history-active", true);
    DATA.with(|d| {
        let mut d = d.borrow_mut();
        d.history_edit = None;
        d.history_dirty = false;
    });
    show("history-title", "Novo histórico");
}
fn edit(kind: &str, id: &str) {
    if BUSY.with(|b| b.get()) {
        return;
    }
    let dirty = DATA.with(|d| {
        if kind == "account" {
            d.borrow().account_dirty
        } else {
            d.borrow().history_dirty
        }
    });
    if dirty {
        show(
            "message",
            "Salve ou descarte a edição atual antes de selecionar outro registro.",
        );
        return;
    }
    show("message", "");
    if kind == "account" {
        let item = DATA.with(|d| d.borrow().accounts.iter().find(|a| a.id == id).cloned());
        if let Some(a) = item {
            set_input("account-code", &a.code);
            set_input("account-number", &a.number);
            set_input("account-name", &a.name);
            set_input("account-balance", &a.opening_balance.replace('.', ","));
            check("account-active", a.active);
            show("account-title", &format!("Editar conta {}", a.code));
            DATA.with(|d| d.borrow_mut().account_edit = Some(a));
        }
    } else {
        let item = DATA.with(|d| d.borrow().histories.iter().find(|a| a.id == id).cloned());
        if let Some(h) = item {
            set_input("history-code", &h.code);
            set_input("history-description", &h.description);
            element("history-direction")
                .dyn_into::<web_sys::HtmlSelectElement>()
                .unwrap()
                .set_value(&h.direction.to_string());
            check("history-active", h.active);
            show("history-title", &format!("Editar histórico {}", h.code));
            DATA.with(|d| d.borrow_mut().history_edit = Some(h));
        }
    }
}
async fn save(kind: &str) -> Result<(), String> {
    if kind == "account" {
        let editing = DATA.with(|d| d.borrow().account_edit.clone());
        let cents = money::parse_cents(&input("account-balance")).map_err(str::to_owned)?;
        let body = AccountInput {
            code: input("account-code"),
            number: input("account-number"),
            name: input("account-name"),
            opening_balance: money::decimal(cents as i128),
            active: checked("account-active"),
            expected_version: editing.as_ref().map(|e| e.version),
        };
        let url = editing.map(|e| format!("/api/accounts/{}", e.id));
        request(
            if url.is_some() { "PUT" } else { "POST" },
            url.as_deref().unwrap_or("/api/accounts"),
            Some(serde_json::to_value(body).unwrap()),
        )
        .await?;
        reset_account();
    } else {
        let editing = DATA.with(|d| d.borrow().history_edit.clone());
        let direction = element("history-direction")
            .dyn_into::<web_sys::HtmlSelectElement>()
            .unwrap()
            .value()
            .parse()
            .map_err(|_| "Selecione Entrada ou Saída.")?;
        let body = HistoryInput {
            code: input("history-code"),
            description: input("history-description"),
            direction,
            active: checked("history-active"),
            expected_version: editing.as_ref().map(|e| e.version),
        };
        let url = editing.map(|e| format!("/api/histories/{}", e.id));
        request(
            if url.is_some() { "PUT" } else { "POST" },
            url.as_deref().unwrap_or("/api/histories"),
            Some(serde_json::to_value(body).unwrap()),
        )
        .await?;
        reset_history();
    }
    show("message", "Cadastro salvo.");
    if load(true).await.is_err() {
        show("message","Cadastro salvo, mas a lista não pôde ser atualizada. Use Atualizar listas; não envie o cadastro novamente.");
    }
    Ok(())
}
fn bind(id: &str, event: &str, callback: impl FnMut(web_sys::Event) + 'static) {
    let handler = Closure::<dyn FnMut(web_sys::Event)>::new(callback);
    element(id)
        .add_event_listener_with_callback(event, handler.as_ref().unchecked_ref())
        .unwrap();
    handler.forget();
}
pub fn init() {
    for kind in ["account", "history"] {
        bind(&format!("{kind}-form"), "input", move |_| {
            DATA.with(|d| {
                let mut d = d.borrow_mut();
                if kind == "account" {
                    d.account_dirty = true
                } else {
                    d.history_dirty = true
                }
            });
        });
        bind(&format!("{kind}-form"), "submit", move |event| {
            event.prevent_default();
            if BUSY.with(|b| b.get()) {
                return;
            }
            busy(true);
            show("message", "");
            spawn_local(async move {
                if let Err(message) = save(kind).await {
                    show("message", &message);
                }
                busy(false);
            });
        });
        bind(&format!("{kind}-cancel"), "click", move |_| {
            if kind == "account" {
                reset_account();
            } else {
                reset_history();
            }
            show("message", "");
        });
        bind(
            if kind == "account" {
                "accounts-body"
            } else {
                "histories-body"
            },
            "click",
            move |event| {
                if let Some(button) = event
                    .target()
                    .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
                    .and_then(|e| e.closest("button[data-edit]").ok().flatten())
                {
                    if let Some(id) = button.get_attribute("data-edit") {
                        edit(kind, &id);
                    }
                }
            },
        );
    }
    for id in ["accounts-inactive", "histories-inactive"] {
        bind(id, "change", |_| render());
    }
    for section in ["accounts", "histories", "settings"] {
        bind(&format!("tab-{section}"), "click", move |_| {
            for target in ["accounts", "histories", "settings"] {
                visible(&format!("panel-{target}"), target == section);
                element(&format!("tab-{target}"))
                    .set_attribute(
                        "aria-pressed",
                        if target == section { "true" } else { "false" },
                    )
                    .unwrap();
            }
        });
    }
    bind("catalog-refresh", "click", |_| {
        if BUSY.with(|b| b.get()) {
            return;
        }
        if DATA.with(|d| d.borrow().account_dirty || d.borrow().history_dirty) {
            show(
                "message",
                "Salve ou descarte as edições antes de atualizar as listas.",
            );
            return;
        }
        busy(true);
        let owner = DATA.with(|d| d.borrow().owner);
        spawn_local(async move {
            match load(owner).await {
                Ok(()) => {
                    reset_account();
                    reset_history();
                    show("message", "Listas atualizadas.");
                }
                Err(e) => show("message", &e),
            }
            busy(false);
        });
    });
}
