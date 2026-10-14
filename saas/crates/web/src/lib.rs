use serde_json::{json, Value};
use std::cell::Cell;
use wasm_bindgen::{prelude::*, JsCast};
use wasm_bindgen_futures::{spawn_local, JsFuture};
mod catalogs;
mod entries;
thread_local! { static BUSY:Cell<bool>=const {Cell::new(false)}; }
fn document() -> web_sys::Document {
    web_sys::window().unwrap().document().unwrap()
}
fn element(id: &str) -> web_sys::Element {
    document()
        .get_element_by_id(id)
        .expect("elemento da pagina")
}
fn show(id: &str, text: &str) {
    element(id).set_text_content(Some(text));
}
fn visible(id: &str, yes: bool) {
    if yes {
        let _ = element(id).remove_attribute("hidden");
    } else {
        let _ = element(id).set_attribute("hidden", "");
    }
}
fn input(id: &str) -> String {
    element(id)
        .dyn_into::<web_sys::HtmlInputElement>()
        .unwrap()
        .value()
}
fn busy(value: bool) {
    BUSY.with(|b| b.set(value));
    let buttons = document()
        .query_selector_all("button,input,select")
        .unwrap();
    for i in 0..buttons.length() {
        if let Some(node) = buttons.item(i) {
            if let Ok(button) = node.dyn_into::<web_sys::Element>() {
                if value {
                    let _ = button.set_attribute("disabled", "");
                } else {
                    let _ = button.remove_attribute("disabled");
                }
            }
        }
    }
}
async fn request(method: &str, path: &str, body: Option<Value>) -> Result<Value, String> {
    let init = web_sys::RequestInit::new();
    init.set_method(method);
    init.set_credentials(web_sys::RequestCredentials::SameOrigin);
    if let Some(body) = body {
        init.set_body(&JsValue::from_str(&body.to_string()));
    }
    let req = web_sys::Request::new_with_str_and_init(path, &init)
        .map_err(|_| "Falha ao preparar solicitação.")?;
    req.headers()
        .set("Content-Type", "application/json")
        .map_err(|_| "Falha nos cabeçalhos.")?;
    req.headers()
        .set("X-Contas-Request", "1")
        .map_err(|_| "Falha nos cabeçalhos.")?;
    let response: web_sys::Response =
        JsFuture::from(web_sys::window().unwrap().fetch_with_request(&req))
            .await
            .map_err(|_| "Sem conexão com o serviço.")?
            .dyn_into()
            .map_err(|_| "Resposta inválida.")?;
    if response.status() == 401 && path != "/api/auth/login" {
        logged_out();
    }
    if response.status() == 204 {
        return Ok(Value::Null);
    }
    let data = JsFuture::from(response.json().map_err(|_| "Resposta inválida.")?)
        .await
        .map_err(|_| "Resposta inválida.")?;
    let value: Value = serde_wasm_bindgen::from_value(data).map_err(|_| "Resposta inválida.")?;
    if response.ok() {
        Ok(value)
    } else {
        Err(value["error"]
            .as_str()
            .unwrap_or("Não foi possível concluir a operação.")
            .to_owned())
    }
}
async fn refresh() -> Result<(), String> {
    let me = request("GET", "/api/auth/me", None).await?;
    show("user-name", me["name"].as_str().unwrap_or(""));
    show("tenant-name", me["tenant_name"].as_str().unwrap_or(""));
    show("tenant-id", me["tenant_id"].as_str().unwrap_or(""));
    let owner = me["role"] == "owner";
    show("role", if owner { "Administrador" } else { "Consulta" });
    element("rename-name")
        .dyn_into::<web_sys::HtmlInputElement>()
        .unwrap()
        .set_value(me["tenant_name"].as_str().unwrap_or(""));
    visible("auth", false);
    visible("workspace", true);
    visible("owner-controls", owner);
    show(
        "status",
        "Sessão ativa. Você está no ambiente indicado abaixo.",
    );
    if let Err(message) = catalogs::load(owner).await {
        show("message", &message);
    }
    Ok(())
}
fn logged_out() {
    catalogs::clear();
    entries::clear();
    visible("auth", true);
    visible("workspace", false);
    show("tenant-id", "");
    show("tenant-name", "");
    show("user-name", "");
    show("status", "Entre ou crie seu ambiente de estudo.");
}
async fn submit(action: &str) -> Result<(), String> {
    match action {
        "login" => {
            request(
                "POST",
                "/api/auth/login",
                Some(json!({"email":input("login-email"),"password":input("login-password")})),
            )
            .await?;
            refresh().await?;
        }
        "register" => {
            if input("register-password") != input("register-confirm") {
                return Err("A confirmação de senha não confere.".into());
            }
            request("POST","/api/auth/register",Some(json!({"email":input("register-email"),"password":input("register-password"),"password_confirmation":input("register-confirm"),"name":input("register-name"),"tenant_name":input("register-tenant")}))).await?;
            refresh().await?;
        }
        "rename" => {
            let id = element("tenant-id").text_content().unwrap_or_default();
            request(
                "PUT",
                &format!("/api/tenants/{id}"),
                Some(json!({"name":input("rename-name")})),
            )
            .await?;
            refresh().await?;
        }
        "member" => {
            if input("member-password") != input("member-confirm") {
                return Err("A confirmação de senha não confere.".into());
            }
            request("POST","/api/members",Some(json!({"email":input("member-email"),"password":input("member-password"),"password_confirmation":input("member-confirm"),"name":input("member-name")}))).await?;
            show("message", "Usuário de consulta criado neste ambiente.");
        }
        "logout" => {
            request("POST", "/api/auth/logout", Some(json!({}))).await?;
            logged_out();
        }
        _ => {}
    };
    Ok(())
}
#[wasm_bindgen(start)]
pub fn start() {
    catalogs::init();
    entries::init();
    for action in ["login", "register", "rename", "member", "logout"] {
        let handler = Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
            event.prevent_default();
            if BUSY.with(|b| b.get()) {
                return;
            }
            busy(true);
            show("message", "");
            spawn_local(async move {
                match submit(action).await {
                    Ok(()) => {
                        if action != "rename" {
                            element(&format!("{action}-form"))
                                .dyn_into::<web_sys::HtmlFormElement>()
                                .unwrap()
                                .reset();
                        }
                    }
                    Err(message) => show("message", &message),
                }
                busy(false);
            });
        });
        element(&format!("{action}-form"))
            .add_event_listener_with_callback("submit", handler.as_ref().unchecked_ref())
            .unwrap();
        handler.forget();
    }
    busy(true);
    spawn_local(async {
        if refresh().await.is_err() {
            logged_out();
        }
        busy(false);
    });
}
