//! Consulta de CEP por proxy local (SEC-063 / CLI-004).
//!
//! O WebView não fala direto com ViaCEP/BrasilAPI por causa da CSP; o servidor
//! faz a chamada com timeout curto, valida o formato e guarda um cache limitado
//! em memória para funcionar offline em consultas repetidas.

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};
use std::time::Duration;

use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};

/// Teto do cache em memória. Um CEP tem 8 dígitos; 512 entradas são poucos KB.
const LIMITE_CACHE: usize = 512;
const TIMEOUT: Duration = Duration::from_secs(3);

type Resposta = (StatusCode, Json<serde_json::Value>);

#[derive(Clone, Serialize, Deserialize)]
pub struct CepInfo {
    pub cep: String,
    pub logradouro: String,
    pub bairro: String,
    pub cidade: String,
    pub uf: String,
}

static CEP_CACHE: OnceLock<RwLock<HashMap<String, CepInfo>>> = OnceLock::new();
static CLIENTE_HTTP: OnceLock<Result<reqwest::Client, String>> = OnceLock::new();

fn cache() -> &'static RwLock<HashMap<String, CepInfo>> {
    CEP_CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

fn cliente() -> Option<&'static reqwest::Client> {
    CLIENTE_HTTP
        .get_or_init(|| {
            reqwest::Client::builder()
                .timeout(TIMEOUT)
                .build()
                .map_err(|e| e.to_string())
        })
        .as_ref()
        .ok()
}

fn ler_cache(cep: &str) -> Option<CepInfo> {
    cache().read().ok()?.get(cep).cloned()
}

fn gravar_cache(info: &CepInfo) {
    let Ok(mut guarda) = cache().write() else {
        return;
    };
    // Cache simples: ao encher, esvazia para não crescer sem limite.
    if guarda.len() >= LIMITE_CACHE {
        guarda.clear();
    }
    guarda.insert(info.cep.clone(), info.clone());
}

fn corpo_ok(info: &CepInfo, provedor: Option<&str>) -> Resposta {
    let mut corpo = serde_json::json!({
        "ok": true,
        "cep": info.cep,
        "logradouro": info.logradouro,
        "bairro": info.bairro,
        "cidade": info.cidade,
        "uf": info.uf,
    });
    if let Some(provedor) = provedor {
        corpo["provedor"] = serde_json::json!(provedor);
    } else {
        corpo["cache"] = serde_json::json!(true);
    }
    (StatusCode::OK, Json(corpo))
}

fn texto(body: &serde_json::Value, chave: &str) -> String {
    body.get(chave)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

async fn buscar_viacep(client: &reqwest::Client, cep: &str) -> Option<CepInfo> {
    let url = format!("https://viacep.com.br/ws/{cep}/json/");
    let resposta = client.get(url).send().await.ok()?;
    if !resposta.status().is_success() {
        return None;
    }
    let body = resposta.json::<serde_json::Value>().await.ok()?;
    if body.get("erro").and_then(|v| v.as_bool()).unwrap_or(false) {
        return None;
    }
    Some(CepInfo {
        cep: cep.to_string(),
        logradouro: texto(&body, "logradouro"),
        bairro: texto(&body, "bairro"),
        cidade: texto(&body, "localidade"),
        uf: texto(&body, "uf").to_ascii_uppercase(),
    })
}

async fn buscar_brasilapi(client: &reqwest::Client, cep: &str) -> Option<CepInfo> {
    let url = format!("https://brasilapi.com.br/api/cep/v1/{cep}");
    let resposta = client.get(url).send().await.ok()?;
    if !resposta.status().is_success() {
        return None;
    }
    let body = resposta.json::<serde_json::Value>().await.ok()?;
    Some(CepInfo {
        cep: cep.to_string(),
        logradouro: texto(&body, "street"),
        bairro: texto(&body, "neighborhood"),
        cidade: texto(&body, "city"),
        uf: texto(&body, "state").to_ascii_uppercase(),
    })
}

/// `GET /v1/sistema/cep/:cep` — consulta de endereço por CEP (proxy local seguro com fallback e cache local).
///
/// Bypassa restrições de CSP do WebView desktop (SEG-063), usa ViaCEP como primário e BrasilAPI como reserva (CLI-004).
pub async fn consultar_cep(Path(cep): Path<String>) -> impl IntoResponse {
    let cep_limpo: String = cep.chars().filter(|c| c.is_ascii_digit()).collect();
    if cep_limpo.len() != 8 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "erro": "CEP deve conter exatamente 8 dígitos" })),
        );
    }

    if let Some(item) = ler_cache(&cep_limpo) {
        return corpo_ok(&item, None);
    }

    let Some(client) = cliente() else {
        tracing::error!("falha ao inicializar client HTTP da consulta de CEP");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "erro": "falha interna de rede" })),
        );
    };

    if let Some(info) = buscar_viacep(client, &cep_limpo).await {
        gravar_cache(&info);
        return corpo_ok(&info, Some("viacep"));
    }

    if let Some(info) = buscar_brasilapi(client, &cep_limpo).await {
        gravar_cache(&info);
        return corpo_ok(&info, Some("brasilapi"));
    }

    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({ "erro": "CEP não encontrado. Preencha o endereço manualmente." })),
    )
}
