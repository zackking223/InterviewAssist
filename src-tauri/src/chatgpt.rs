use std::{process::Command, time::{Duration, SystemTime, UNIX_EPOCH}};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use jsonwebtoken::{decode, decode_header, jwk::JwkSet, Algorithm, DecodingKey, Validation};
use rand::{distributions::Alphanumeric, Rng};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpListener};
use url::Url;

const AUTHORIZE_URL: &str = "https://auth.openai.com/api/accounts/authorize";
const TOKEN_URL: &str = "https://auth.openai.com/api/accounts/oauth/token";
const RESOURCE: &str = "https://api.openai.com/v1";
const SCOPES: &str = "openid profile email offline_access resource.invoke chatgpt.tokens.use.direct";

#[derive(Clone, Serialize, Deserialize)]
pub struct ChatGptProfile {
    pub email: String,
    pub subject: String,
    pub client_id: String,
    pub host_id: String,
    pub id_token: String,
    pub access_token: String,
    pub refresh_token: String,
    pub scope: String,
    pub expires_at: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatGptStatus {
    pub connected: bool,
    pub email: Option<String>,
    pub plan_usage_enabled: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ChatGptModel {
    pub slug: String,
    pub display_name: String,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    id_token: Option<String>,
    expires_in: u64,
    scope: Option<String>,
}

#[derive(Deserialize)]
struct IdClaims {
    sub: String,
    email: Option<String>,
    nonce: Option<String>,
}

pub fn status() -> ChatGptStatus {
    crate::secret_store::repair_incomplete_chatgpt_profile();
    match crate::secret_store::get_chatgpt_profile() {
        Ok(profile) => ChatGptStatus {
            connected: true,
            email: Some(profile.email),
            plan_usage_enabled: profile.scope.split_whitespace().any(|s| s == "chatgpt.tokens.use.direct"),
        },
        Err(_) => ChatGptStatus { connected: false, email: None, plan_usage_enabled: false },
    }
}

pub async fn sign_in() -> Result<ChatGptStatus, String> {
    let existing = crate::secret_store::get_chatgpt_profile().ok();
    let host_id = existing.as_ref().map(|p| p.host_id.clone()).unwrap_or_else(|| format!("urn:uuid:{}", uuid::Uuid::new_v4()));
    let requested_client = existing.as_ref().map(|p| p.client_id.as_str()).unwrap_or("dynamic_agent_client");
    let state = random_string(48);
    let nonce = random_string(48);
    let verifier = random_string(96);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let listener = TcpListener::bind("127.0.0.1:0").await.map_err(|e| format!("Could not start sign-in callback: {e}"))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}/auth/callback");

    let mut url = Url::parse(AUTHORIZE_URL).map_err(|e| e.to_string())?;
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("client_id", requested_client)
            .append_pair("ext_agent_host_id", &host_id)
            .append_pair("response_type", "code")
            .append_pair("redirect_uri", &redirect_uri)
            .append_pair("scope", SCOPES)
            .append_pair("resource", RESOURCE)
            .append_pair("state", &state)
            .append_pair("nonce", &nonce)
            .append_pair("code_challenge_method", "S256")
            .append_pair("code_challenge", &challenge);
        if existing.is_none() { query.append_pair("agent_name_hint", "InterviewAssist"); }
        if let Some(profile) = &existing {
            query.append_pair("id_token_hint", &profile.id_token).append_pair("login_hint", &profile.email);
        }
    }
    open_browser(url.as_str())?;

    let accepted = tokio::time::timeout(Duration::from_secs(300), listener.accept()).await
        .map_err(|_| "ChatGPT sign-in timed out".to_string())?
        .map_err(|e| e.to_string())?;
    let (mut stream, _) = accepted;
    let mut buffer = vec![0u8; 16 * 1024];
    let count = stream.read(&mut buffer).await.map_err(|e| e.to_string())?;
    let request = String::from_utf8_lossy(&buffer[..count]);
    let target = request.lines().next().and_then(|line| line.split_whitespace().nth(1)).ok_or("Invalid sign-in callback")?;
    let callback = Url::parse(&format!("http://127.0.0.1{target}")).map_err(|e| e.to_string())?;
    let params: std::collections::HashMap<_, _> = callback.query_pairs().into_owned().collect();
    let response_body = if params.get("error").is_some() { "ChatGPT sign-in was cancelled. You can close this tab." } else { "Authorization received. Return to InterviewAssist while it finishes connecting." };
    let http = format!("HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", response_body.len(), response_body);
    let _ = stream.write_all(http.as_bytes()).await;
    if params.get("state") != Some(&state) { return Err("ChatGPT sign-in state did not match".into()); }
    if let Some(error) = params.get("error") { return Err(format!("ChatGPT sign-in failed: {error}")); }
    let code = params.get("code").ok_or("ChatGPT did not return an authorization code")?;
    let issued_client = params.get("client_id").cloned().or_else(|| existing.as_ref().map(|p| p.client_id.clone())).ok_or("ChatGPT registration did not return a client ID")?;
    if let Some(profile) = &existing { if issued_client != profile.client_id { return Err("ChatGPT returned a different client registration".into()); } }

    let token = token_request(&[
        ("grant_type", "authorization_code"), ("client_id", &issued_client), ("code", code),
        ("code_verifier", &verifier), ("redirect_uri", &redirect_uri), ("resource", RESOURCE),
    ]).await?;
    let id_token = token.id_token.ok_or("ChatGPT token response did not include identity")?;
    let claims = validate_identity(&id_token, &issued_client, &nonce).await?;
    if let Some(profile) = &existing { if claims.sub != profile.subject { return Err("ChatGPT account does not match the saved connection".into()); } }
    let scope = token.scope.unwrap_or_else(|| params.get("scope").cloned().unwrap_or_default());
    if !scope.split_whitespace().any(|s| s == "chatgpt.tokens.use.direct") { return Err("ChatGPT plan usage permission was not granted".into()); }
    let profile = ChatGptProfile {
        email: claims.email.unwrap_or_else(|| "ChatGPT account".into()), subject: claims.sub,
        client_id: issued_client, host_id, id_token, access_token: token.access_token,
        refresh_token: token.refresh_token.ok_or("ChatGPT did not return a refresh token")?, scope,
        expires_at: now() + token.expires_in,
    };
    crate::secret_store::save_chatgpt_profile(&profile)?;
    Ok(status())
}

pub async fn sign_out() -> Result<(), String> {
    if let Ok(profile) = crate::secret_store::get_chatgpt_profile() {
        let _ = reqwest::Client::new().post("https://auth.openai.com/api/accounts/oauth/revoke")
            .form(&[("token", profile.refresh_token), ("token_type_hint", "refresh_token".into()), ("client_id", profile.client_id)])
            .send().await;
    }
    crate::secret_store::delete_chatgpt_profile()
}

pub async fn access_token() -> Result<String, String> {
    let mut profile = crate::secret_store::get_chatgpt_profile()?;
    if profile.expires_at > now() + 60 { return Ok(profile.access_token); }
    let token = token_request(&[("grant_type", "refresh_token"), ("client_id", &profile.client_id), ("refresh_token", &profile.refresh_token), ("resource", RESOURCE)]).await?;
    profile.access_token = token.access_token;
    profile.refresh_token = token.refresh_token.ok_or("ChatGPT refresh did not return a replacement token")?;
    profile.expires_at = now() + token.expires_in;
    if let Some(value) = token.id_token { profile.id_token = value; }
    if let Some(value) = token.scope { profile.scope = value; }
    crate::secret_store::save_chatgpt_profile(&profile)?;
    Ok(profile.access_token)
}

pub async fn models() -> Result<Vec<ChatGptModel>, String> {
    let response = reqwest::Client::new().get("https://api.openai.com/v1/models")
        .header(AUTHORIZATION, format!("Bearer {}", access_token().await?)).send().await.map_err(|e| e.to_string())?;
    if !response.status().is_success() { return Err(api_error(response).await); }
    let value: Value = response.json().await.map_err(|e| e.to_string())?;
    let models = value.get("models").and_then(Value::as_array).ok_or("ChatGPT returned an invalid model catalog")?
        .iter().filter(|m| m.get("visibility").and_then(Value::as_str) == Some("list"))
        .filter_map(|m| Some(ChatGptModel { slug: m.get("slug")?.as_str()?.to_owned(), display_name: m.get("display_name")?.as_str()?.to_owned() })).collect();
    Ok(models)
}

async fn token_request(fields: &[(&str, &str)]) -> Result<TokenResponse, String> {
    let response = reqwest::Client::new().post(TOKEN_URL).header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .form(fields).send().await.map_err(|e| e.to_string())?;
    if !response.status().is_success() { return Err(api_error(response).await); }
    response.json().await.map_err(|e| e.to_string())
}

async fn validate_identity(token: &str, client_id: &str, nonce: &str) -> Result<IdClaims, String> {
    let header = decode_header(token).map_err(|e| format!("Invalid ChatGPT identity token: {e}"))?;
    let kid = header.kid.ok_or("ChatGPT identity token has no key ID")?;
    let config: Value = reqwest::get("https://auth.openai.com/.well-known/openid-configuration").await.map_err(|e| e.to_string())?.json().await.map_err(|e| e.to_string())?;
    let jwks_uri = config.get("jwks_uri").and_then(Value::as_str).ok_or("OpenAI identity configuration has no JWKS URI")?;
    let keys: JwkSet = reqwest::get(jwks_uri).await.map_err(|e| e.to_string())?.json().await.map_err(|e| e.to_string())?;
    let jwk = keys.find(&kid).ok_or("ChatGPT identity signing key was not found")?;
    let key = DecodingKey::from_jwk(jwk).map_err(|e| e.to_string())?;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(&[client_id]);
    validation.set_issuer(&["https://auth.openai.com"]);
    let claims = decode::<IdClaims>(token, &key, &validation).map_err(|e| format!("ChatGPT identity verification failed: {e}"))?.claims;
    if claims.nonce.as_deref() != Some(nonce) { return Err("ChatGPT identity nonce did not match".into()); }
    Ok(claims)
}

fn open_browser(url: &str) -> Result<(), String> {
    Command::new("rundll32.exe").arg("url.dll,FileProtocolHandler").arg(url).spawn().map(|_| ()).map_err(|e| format!("Could not open browser: {e}"))
}

fn random_string(length: usize) -> String { rand::thread_rng().sample_iter(&Alphanumeric).take(length).map(char::from).collect() }
fn now() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() }

async fn api_error(response: reqwest::Response) -> String {
    let status = response.status();
    let body = response.json::<Value>().await.ok();
    body.as_ref().and_then(|v| v.pointer("/error/message").or_else(|| v.get("detail"))).and_then(Value::as_str).map(str::to_owned).unwrap_or_else(|| format!("ChatGPT request failed with HTTP {status}"))
}
