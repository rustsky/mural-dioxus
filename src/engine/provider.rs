#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderFailureKind { Authentication, ModelAccess, Quota, RateLimit, Unavailable, InvalidRequest, Unknown }

impl ProviderFailureKind {
    pub fn classify(status: u16, code: Option<&str>) -> Self {
        match status {
            401 => Self::Authentication,
            403 | 404 => Self::ModelAccess,
            429 => if code == Some("insufficient_quota") { Self::Quota } else { Self::RateLimit },
            408 => Self::Unavailable,
            s if s >= 500 => Self::Unavailable,
            400 | 422 => Self::InvalidRequest,
            _ => Self::Unknown,
        }
    }
}

/// Keeps a provider's safe category and support reference, never its message or response body.
#[derive(Clone, Debug, PartialEq)]
pub struct ProviderFailure {
    pub status: u16,
    pub code: Option<String>,
    pub reference: Option<String>,
}

impl ProviderFailure {
    pub fn new(status: u16, body: &[u8], reference: Option<&str>) -> Self {
        let code = if body.len() <= 16_384 {
            serde_json::from_slice::<serde_json::Value>(body).ok()
                .and_then(|j| j.get("error")?.get("code")?.as_str().map(String::from))
        } else { None };
        Self { status, code: Self::safe_code(code.as_deref()), reference: Self::safe_reference(reference) }
    }
    pub fn kind(&self) -> ProviderFailureKind { ProviderFailureKind::classify(self.status, self.code.as_deref()) }
    pub fn safe_code(value: Option<&str>) -> Option<String> {
        let v = value?;
        ["invalid_api_key", "insufficient_quota", "rate_limit_exceeded", "model_not_found", "permission_denied", "server_error"]
            .contains(&v).then(|| v.to_string())
    }
    pub fn safe_reference(value: Option<&str>) -> Option<String> {
        let v = value?;
        (!v.is_empty() && v.len() <= 128 && v.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')).then(|| v.to_string())
    }
}

impl std::fmt::Display for ProviderFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self.kind() {
            ProviderFailureKind::Authentication => "Your OpenAI key wasn’t accepted. Check it in Settings.",
            ProviderFailureKind::ModelAccess => "This API key may not have access to the requested model. Check your OpenAI project.",
            ProviderFailureKind::Quota => "Your OpenAI project has no available API credit. Check its billing and usage limit before trying again.",
            ProviderFailureKind::RateLimit => "OpenAI is limiting requests. Wait briefly and try again. If this continues, check your project’s billing and limits.",
            ProviderFailureKind::Unavailable => "The voice or teaching service is temporarily unavailable. Please try again shortly.",
            ProviderFailureKind::InvalidRequest => "The service could not accept this request. If this continues, contact support.",
            ProviderFailureKind::Unknown => "The service could not complete this request. Please try again later.",
        };
        match &self.reference {
            Some(r) => write!(f, "{message}\n\nOpenAI reference: {r}"),
            None => f.write_str(message),
        }
    }
}
