//! `copilot-core` LLM adapter over the app's configured summary model.
//!
//! The question summary uses whichever model the user already set up for summaries
//! (built-in, Ollama, OpenAI-compatible, ...) by going through the same
//! `generate_summary` dispatcher, so there is one model configuration, not two.

use crate::database::repositories::setting::SettingsRepository;
use crate::summary::llm_client::{generate_summary, LLMProvider};
use copilot_core::LlmClient;
use sqlx::SqlitePool;
use std::path::PathBuf;
use tokio::runtime::Handle;

pub struct AppLlm {
    handle: Handle,
    http: reqwest::Client,
    provider: LLMProvider,
    model: String,
    api_key: String,
    ollama_endpoint: Option<String>,
    custom_openai_endpoint: Option<String>,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
    top_p: Option<f32>,
    app_data_dir: Option<PathBuf>,
}

impl AppLlm {
    /// Human-readable engine label stored with the analysis ("provider/model").
    pub fn label(&self) -> String {
        format!("{}/{}", provider_label(&self.provider), self.model)
    }

    /// Read the saved summary-model settings. `Err` means no usable model is
    /// configured; the caller then falls back to the rule-based analyser.
    pub async fn resolve(pool: &SqlitePool, app_data_dir: Option<PathBuf>) -> Result<Self, String> {
        let setting = SettingsRepository::get_model_config(pool)
            .await
            .map_err(|e| e.to_string())?
            .ok_or("no summary model is configured")?;
        let provider = LLMProvider::from_str(&setting.provider)?;
        let mut model = setting.model.clone();
        if model.trim().is_empty() {
            return Err("no summary model is configured".into());
        }

        let mut api_key = String::new();
        let (mut custom_endpoint, mut max_tokens, mut temperature, mut top_p) = (None, None, None, None);
        match provider {
            LLMProvider::Ollama | LLMProvider::BuiltInAI => {}
            LLMProvider::CustomOpenAI => {
                let cfg = SettingsRepository::get_custom_openai_config(pool)
                    .await
                    .map_err(|e| e.to_string())?
                    .ok_or("custom OpenAI provider selected but not configured")?;
                custom_endpoint = Some(cfg.endpoint);
                api_key = cfg.api_key.unwrap_or_default();
                if !cfg.model.trim().is_empty() {
                    model = cfg.model;
                }
                max_tokens = cfg.max_tokens.map(|t| t as u32);
                temperature = cfg.temperature;
                top_p = cfg.top_p;
            }
            _ => {
                api_key = SettingsRepository::get_api_key(pool, &setting.provider)
                    .await
                    .map_err(|e| e.to_string())?
                    .filter(|k| !k.is_empty())
                    .ok_or_else(|| format!("no API key stored for {}", setting.provider))?;
            }
        }

        Ok(Self {
            handle: Handle::current(),
            http: reqwest::Client::new(),
            ollama_endpoint: if provider == LLMProvider::Ollama { setting.ollama_endpoint.clone() } else { None },
            provider,
            model,
            api_key,
            custom_openai_endpoint: custom_endpoint,
            max_tokens: max_tokens.or(Some(1024)),
            temperature: temperature.or(Some(0.0)),
            top_p,
            app_data_dir,
        })
    }
}

fn provider_label(p: &LLMProvider) -> &'static str {
    match p {
        LLMProvider::Ollama => "ollama",
        LLMProvider::BuiltInAI => "built-in",
        LLMProvider::CustomOpenAI => "custom-openai",
        _ => "cloud",
    }
}

/// Models often wrap JSON in prose or code fences; keep the outermost object.
pub fn extract_json_object(text: &str) -> &str {
    match (text.find('{'), text.rfind('}')) {
        (Some(a), Some(b)) if b > a => &text[a..=b],
        _ => text.trim(),
    }
}

impl LlmClient for AppLlm {
    fn complete(&self, system: &str, user: &str, schema: &str) -> Result<String, String> {
        let system = format!(
            "{system}\n\nRespond with ONLY one JSON object that matches this JSON schema. \
             No prose, no markdown, no code fences.\nSCHEMA: {schema}"
        );
        // Called from `spawn_blocking`, so blocking on the runtime handle is safe.
        let completion = self.handle.block_on(generate_summary(
            &self.http,
            &self.provider,
            &self.model,
            &self.api_key,
            &system,
            user,
            self.ollama_endpoint.as_deref(),
            self.custom_openai_endpoint.as_deref(),
            self.max_tokens,
            self.temperature,
            self.top_p,
            self.app_data_dir.as_ref(),
            None,
        ))?;
        Ok(extract_json_object(&completion.content).to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_the_json_object_from_fenced_or_chatty_output() {
        assert_eq!(extract_json_object("```json\n{\"a\":1}\n```"), "{\"a\":1}");
        assert_eq!(extract_json_object("Sure! {\"a\":{\"b\":2}} done"), "{\"a\":{\"b\":2}}");
        assert_eq!(extract_json_object("  no json  "), "no json");
    }
}
