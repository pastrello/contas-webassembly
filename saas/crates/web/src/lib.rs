use contas_contracts::ServiceStatus;
use wasm_bindgen::{prelude::*, JsCast};
use wasm_bindgen_futures::{spawn_local, JsFuture};

fn show(id: &str, message: &str) {
    if let Some(element) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id(id))
    {
        element.set_text_content(Some(message));
    }
}

async fn load_status() -> Result<ServiceStatus, JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("Navegador indisponível"))?;
    let response: web_sys::Response = JsFuture::from(window.fetch_with_str("/api/status"))
        .await?
        .dyn_into()?;
    if !response.ok() {
        return Err(JsValue::from_str("Serviço indisponível"));
    }
    let json = JsFuture::from(response.json()?).await?;
    serde_wasm_bindgen::from_value(json).map_err(|e| JsValue::from_str(&e.to_string()))
}

#[wasm_bindgen(start)]
pub fn start() {
    show("wasm-status", "WebAssembly carregado");
    spawn_local(async {
        match load_status().await {
            Ok(status) => {
                show(
                    "api-status",
                    &format!("{} · versão {}", status.service, status.version),
                );
                show(
                    "database-status",
                    if status.database == "connected" {
                        "PostgreSQL conectado"
                    } else {
                        "PostgreSQL indisponível"
                    },
                );
                show("overall-status", "Ambiente conectado");
            }
            Err(_) => {
                show(
                    "api-status",
                    "Não foi possível confirmar a conexão com o serviço e o banco.",
                );
                show("database-status", "Conexão não confirmada");
                show(
                    "overall-status",
                    "Serviço indisponível. Tente atualizar a página.",
                );
            }
        }
    });
}
