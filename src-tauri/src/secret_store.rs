const SERVICE: &str = "InterviewAssist";
const ACCOUNT: &str = "openai-api-key";
const OPENROUTER_ACCOUNT: &str = "openrouter-api-key";
const GOOGLE_ACCOUNT: &str = "google-ai-studio-api-key";
const CHATGPT_ACCOUNT: &str = "chatgpt-plan-profile";
const CHATGPT_ACCESS_TOKEN: &str = "chatgpt-plan-access-token";
const CHATGPT_REFRESH_TOKEN: &str = "chatgpt-plan-refresh-token";
const CHATGPT_ID_TOKEN: &str = "chatgpt-plan-id-token";

fn entry(account: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, account).map_err(|error| error.to_string())
}

pub fn save_api_key(value: &str) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("API key cannot be empty".into());
    }
    entry(ACCOUNT)?
        .set_password(trimmed)
        .map_err(|error| error.to_string())
}

pub fn get_api_key() -> Result<String, String> {
    entry(ACCOUNT)?
        .get_password()
        .map_err(|_| "OpenAI API key is not configured".into())
}

pub fn has_api_key() -> bool {
    get_api_key().is_ok()
}

pub fn delete_api_key() -> Result<(), String> {
    match entry(ACCOUNT)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

pub fn save_openrouter_key(value: &str) -> Result<(), String> {
    let trimmed = value.trim();
    let trimmed = if trimmed.eq_ignore_ascii_case("Bearer") {
        ""
    } else if trimmed
        .get(..7)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("Bearer "))
    {
        trimmed[7..].trim()
    } else {
        trimmed
    };
    if trimmed.is_empty() {
        return Err("OpenRouter API key cannot be empty".into());
    }
    entry(OPENROUTER_ACCOUNT)?
        .set_password(trimmed)
        .map_err(|error| error.to_string())
}

pub fn get_openrouter_key() -> Result<String, String> {
    entry(OPENROUTER_ACCOUNT)?
        .get_password()
        .map_err(|_| "OpenRouter API key is not configured".into())
}

pub fn has_openrouter_key() -> bool {
    get_openrouter_key().is_ok()
}

pub fn delete_openrouter_key() -> Result<(), String> {
    match entry(OPENROUTER_ACCOUNT)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

pub fn save_google_key(value: &str) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("Google AI Studio API key cannot be empty".into());
    }
    entry(GOOGLE_ACCOUNT)?
        .set_password(trimmed)
        .map_err(|error| error.to_string())
}

pub fn get_google_key() -> Result<String, String> {
    entry(GOOGLE_ACCOUNT)?
        .get_password()
        .map_err(|_| "Google AI Studio API key is not configured".into())
}

pub fn has_google_key() -> bool {
    get_google_key().is_ok()
}

pub fn delete_google_key() -> Result<(), String> {
    match entry(GOOGLE_ACCOUNT)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

pub fn save_chatgpt_profile(profile: &crate::chatgpt::ChatGptProfile) -> Result<(), String> {
    let mut metadata = profile.clone();
    metadata.access_token.clear();
    metadata.refresh_token.clear();
    metadata.id_token.clear();
    let value = serde_json::to_string(&metadata).map_err(|error| error.to_string())?;
    // Write the completion marker last. If Windows rejects a token, the app
    // must not mistake a partially saved connection for a usable account.
    save_chunked(CHATGPT_ACCESS_TOKEN, &profile.access_token)?;
    save_chunked(CHATGPT_REFRESH_TOKEN, &profile.refresh_token)?;
    save_chunked(CHATGPT_ID_TOKEN, &profile.id_token)?;
    entry(CHATGPT_ACCOUNT)?.set_password(&value).map_err(|error| error.to_string())
}

pub fn get_chatgpt_profile() -> Result<crate::chatgpt::ChatGptProfile, String> {
    let value = entry(CHATGPT_ACCOUNT)?.get_password().map_err(|_| "ChatGPT account is not connected".to_string())?;
    let mut profile: crate::chatgpt::ChatGptProfile = serde_json::from_str(&value).map_err(|error| error.to_string())?;
    profile.access_token = get_chunked(CHATGPT_ACCESS_TOKEN).map_err(|_| "ChatGPT access token is missing".to_string())?;
    profile.refresh_token = get_chunked(CHATGPT_REFRESH_TOKEN).map_err(|_| "ChatGPT refresh token is missing".to_string())?;
    profile.id_token = get_chunked(CHATGPT_ID_TOKEN).map_err(|_| "ChatGPT identity token is missing".to_string())?;
    Ok(profile)
}

pub fn repair_incomplete_chatgpt_profile() {
    if get_chatgpt_profile().is_err() {
        let _ = delete_chatgpt_profile();
    }
}

pub fn delete_chatgpt_profile() -> Result<(), String> {
    for account in [CHATGPT_ACCOUNT, CHATGPT_ACCESS_TOKEN, CHATGPT_REFRESH_TOKEN, CHATGPT_ID_TOKEN] {
        match entry(account)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(error) => return Err(error.to_string()),
        }
    }
    for base in [CHATGPT_ACCESS_TOKEN, CHATGPT_REFRESH_TOKEN, CHATGPT_ID_TOKEN] {
        for index in 0..16 {
            let account = format!("{base}-{index}");
            match entry(&account)?.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => {}
                Err(error) => return Err(error.to_string()),
            }
        }
    }
    Ok(())
}

fn save_chunked(base: &str, value: &str) -> Result<(), String> {
    // Credential Manager limits a generic credential blob to 2560 bytes.
    // OAuth JWT sizes vary, so keep every UTF-16 password comfortably below it.
    for index in 0..16 {
        let account = format!("{base}-{index}");
        let _ = entry(&account)?.delete_credential();
    }
    for (index, chunk) in value.as_bytes().chunks(1000).enumerate() {
        let text = std::str::from_utf8(chunk).map_err(|error| error.to_string())?;
        entry(&format!("{base}-{index}"))?.set_password(text).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn get_chunked(base: &str) -> Result<String, String> {
    let mut value = String::new();
    for index in 0..16 {
        match entry(&format!("{base}-{index}"))?.get_password() {
            Ok(chunk) => value.push_str(&chunk),
            Err(keyring::Error::NoEntry) if index == 0 => {
                return entry(base)?.get_password().map_err(|error| error.to_string());
            }
            Err(keyring::Error::NoEntry) => break,
            Err(error) => return Err(error.to_string()),
        }
    }
    if value.is_empty() { Err("credential is empty".into()) } else { Ok(value) }
}
