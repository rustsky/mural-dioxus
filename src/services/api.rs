use std::sync::OnceLock;
use std::time::Duration;

use serde_json::{json, Value};

use crate::engine::languages::LanguageModule;
use crate::engine::models::SourceLink;
use crate::engine::provider::ProviderFailure;

use super::credentials;

pub const TEACHER_MODEL: &str = "gpt-5.6-luna";
pub const VOICE_MODEL: &str = "gpt-live-1";

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Usage { pub input: i64, pub output: i64, pub searches: i64 }

#[derive(Clone, Debug, PartialEq)]
pub struct ApiResult { pub text: String, pub sources: Vec<SourceLink>, pub usage: Usage }

#[derive(Clone, Debug, PartialEq)]
pub enum ApiError { MissingKey, InvalidResponse, Incomplete, Refused, Network, Provider(ProviderFailure) }

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::MissingKey => f.write_str("Add your OpenAI key in Settings to begin."),
            ApiError::InvalidResponse | ApiError::Incomplete => f.write_str("OpenAI returned an incomplete response. Please try again."),
            ApiError::Refused => f.write_str("Mural couldn’t complete that request. Try a different topic."),
            ApiError::Network => f.write_str("Mural couldn’t reach OpenAI. Check your connection and try again."),
            ApiError::Provider(p) => write!(f, "{p}"),
        }
    }
}

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(45))
            .read_timeout(Duration::from_secs(45))
            .timeout(Duration::from_secs(60))
            .build()
            .expect("HTTP client")
    })
}

pub async fn post(path: &str, body: &Value) -> Result<Value, ApiError> {
    let key = credentials::read().ok_or(ApiError::MissingKey)?;
    let response = client()
        .post(format!("https://api.openai.com/v1/{path}"))
        .bearer_auth(key)
        .json(body)
        .send()
        .await
        .map_err(|_| ApiError::Network)?;
    let status = response.status().as_u16();
    let reference = response.headers().get("x-request-id").and_then(|v| v.to_str().ok()).map(String::from);
    let bytes = response.bytes().await.map_err(|_| ApiError::Network)?;
    if !(200..300).contains(&status) {
        return Err(ApiError::Provider(ProviderFailure::new(status, &bytes, reference.as_deref())));
    }
    let json: Value = serde_json::from_slice(&bytes).map_err(|_| ApiError::InvalidResponse)?;
    if !json.is_object() { return Err(ApiError::InvalidResponse); }
    Ok(json)
}

pub async fn respond(instructions: &str, input: &str, schema: Option<Value>, search: bool) -> Result<ApiResult, ApiError> {
    let mut body = json!({
        "model": TEACHER_MODEL, "store": false, "instructions": instructions,
        "input": [{"role": "user", "content": input}],
        "max_output_tokens": if schema.is_none() { 1400 } else { 2200 },
        "reasoning": {"effort": "low"},
    });
    if let Some(schema) = schema {
        body["text"] = json!({"format": {"type": "json_schema", "name": "mural_result", "strict": true, "schema": schema}});
    }
    if search {
        body["tools"] = json!([{"type": "web_search"}]);
        body["tool_choice"] = json!("auto");
        body["max_tool_calls"] = json!(1);
    }
    let json = post("responses", &body).await?;
    parse_response(&json)
}

pub fn parse_response(json: &Value) -> Result<ApiResult, ApiError> {
    if json["status"].as_str() != Some("completed") { return Err(ApiError::Incomplete); }
    let mut text = String::new();
    let mut sources: Vec<SourceLink> = vec![];
    let mut usage = Usage::default();
    for item in json["output"].as_array().into_iter().flatten() {
        if item["type"].as_str() == Some("web_search_call") { usage.searches += 1; }
        for content in item["content"].as_array().into_iter().flatten() {
            match content["type"].as_str() {
                Some("refusal") => return Err(ApiError::Refused),
                Some("output_text") => text.push_str(content["text"].as_str().unwrap_or("")),
                _ => {}
            }
            for citation in content["annotations"].as_array().into_iter().flatten() {
                let (Some("url_citation"), Some(url)) = (citation["type"].as_str(), citation["url"].as_str()) else { continue };
                let source = SourceLink { title: citation["title"].as_str().unwrap_or("Source").into(), url: url.into() };
                if source.safe_url().is_some() && !sources.iter().any(|s| s.url == url) { sources.push(source); }
            }
        }
    }
    usage.input = json["usage"]["input_tokens"].as_i64().unwrap_or(0);
    usage.output = json["usage"]["output_tokens"].as_i64().unwrap_or(0);
    if text.is_empty() { return Err(ApiError::Incomplete); }
    Ok(ApiResult { text, sources, usage })
}

fn object(fields: Vec<(&str, Value)>) -> Value {
    let mut required: Vec<&str> = fields.iter().map(|(k, _)| *k).collect();
    required.sort();
    let properties: serde_json::Map<String, Value> = fields.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
    json!({"type": "object", "properties": properties, "required": required, "additionalProperties": false})
}

pub fn assessment_schema(language: &LanguageModule) -> Value {
    let string = || json!({"type": "string"});
    let mut languages = vec![language.id, "en", "mixed", "uncertain"];
    languages.sort();
    languages.dedup();
    object(vec![
        ("outcome", json!({"type": "string", "enum": ["success", "partial", "breakdown", "uncertain"]})),
        ("suggestedLevel", json!({"type": "integer", "minimum": 0, "maximum": 5})),
        ("nextGoal", string()),
        ("capability", string()),
        ("words", json!({"type": "array", "maxItems": 12, "items": object(vec![
            ("lemma", string()), ("meaning", string()), ("form", string()), ("quote", string()),
            ("language", json!({"type": "string", "enum": languages})),
            ("kind", json!({"type": "string", "enum": ["exposure", "understanding", "assisted", "independent", "lapse"]})),
            ("confidence", json!({"type": "number", "minimum": 0, "maximum": 1})),
            ("sourceIDs", json!({"type": "array", "items": string()})),
        ])})),
    ])
}

/// Starts a live voice session: the browser's SDP offer goes out, the provider's answer comes back.
pub async fn create_live_session(instructions: &str, sdp: &str) -> Result<(String, Option<Value>), ApiError> {
    let body = json!({
        "session": {"model": VOICE_MODEL, "instructions": instructions, "input": [], "store": false,
                    "delegation": {"type": "client"}, "audio": {"output": {"voice": "marin"}}},
        "transport": {"type": "webrtc", "sdp": sdp},
    });
    let result = post("live/sessions", &body).await?;
    let answer = result["transport"]["sdp"].as_str().ok_or(ApiError::InvalidResponse)?.to_string();
    Ok((answer, result.get("session").cloned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_text_citations_and_usage() {
        let json = json!({
            "status": "completed",
            "output": [
                {"type": "web_search_call"},
                {"type": "message", "content": [{"type": "output_text", "text": "Hola", "annotations": [
                    {"type": "url_citation", "url": "https://example.com/a", "title": "A"},
                    {"type": "url_citation", "url": "https://example.com/a", "title": "A again"},
                    {"type": "url_citation", "url": "http://insecure.test/", "title": "B"}
                ]}]}
            ],
            "usage": {"input_tokens": 12, "output_tokens": 3}
        });
        let r = parse_response(&json).unwrap();
        assert_eq!(r.text, "Hola");
        assert_eq!(r.sources.len(), 1);
        assert_eq!(r.usage, Usage { input: 12, output: 3, searches: 1 });
        assert_eq!(parse_response(&json!({"status": "incomplete"})), Err(ApiError::Incomplete));
        let refused = json!({"status": "completed", "output": [{"content": [{"type": "refusal"}]}]});
        assert_eq!(parse_response(&refused), Err(ApiError::Refused));
    }

    #[test]
    fn schema_requires_every_field() {
        let schema = assessment_schema(crate::engine::languages::module("es").unwrap());
        assert_eq!(schema["required"], json!(["capability", "nextGoal", "outcome", "suggestedLevel", "words"]));
        assert_eq!(schema["properties"]["words"]["items"]["properties"]["language"]["enum"], json!(["en", "es", "mixed", "uncertain"]));
    }
}
