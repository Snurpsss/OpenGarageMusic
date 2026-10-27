//! Signing in to a site that asks for an account, by the device flow of
//! OAuth 2 (RFC 8628): the program asks the site for a code, the user
//! enters it on the site's page in their browser, signed in to their
//! account, and the program polls the site until it gives a token. The
//! token is kept in the settings; a refresh token renews it when it
//! expires. Nothing here runs unless the user asks to sign in.

use std::sync::mpsc;
use std::time::Duration;

use og_engine::sites::Login;
use serde::{Deserialize, Serialize};

use crate::downloads::agent;
use crate::i18n::tr;

/// The token a site gave for the user's account.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SiteToken {
    pub access: String,
    pub refresh: String,
    /// When the access token expires, in seconds since the epoch.
    pub expires: u64,
}

impl SiteToken {
    /// Whether the token is still good for a minute at least.
    pub fn fresh(&self) -> bool {
        !self.access.is_empty() && self.expires > crate::sessions::now() + 60
    }
}

/// What the site answered when asked for a device code.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct TokenAnswer {
    access_token: String,
    refresh_token: String,
    expires_in: u64,
    error: String,
    error_description: String,
}

/// Posts a form to an endpoint of the site and reads its JSON answer, or the
/// error it gives.
fn post(url: &str, form: &[(&str, &str)]) -> Result<String, String> {
    if !url.starts_with("https://") {
        return Err(tf!("the description asks for “{url}”, which is not a secure address" [url]));
    }
    match agent(30)?.post(url).set("Accept", "application/json").send_form(form) {
        Ok(response) => response.into_string().map_err(|e| tf!("unreadable answer ({e})" [e])),
        // The site says why in the body of an error, as the flow wants.
        Err(ureq::Error::Status(_, response)) => Ok(response.into_string().unwrap_or_default()),
        Err(e) => Err(tf!("the site did not answer ({e})" [e])),
    }
}

/// Asks the site for a code the user will enter on its page.
pub fn device_code(login: &Login, client_id: &str) -> Result<DeviceCode, String> {
    let scope = if login.scope.is_empty() { "read" } else { login.scope.as_str() };
    let text = post(&login.device, &[("client_id", client_id), ("scope", scope)])?;
    let code: DeviceCode = serde_json::from_str(&text).map_err(|e| tf!("unreadable answer ({e})" [e]))?;
    if code.device_code.is_empty() || code.user_code.is_empty() {
        let answer: TokenAnswer = serde_json::from_str(&text).unwrap_or_default();
        let why = if answer.error_description.is_empty() { answer.error } else { answer.error_description };
        return Err(if why.is_empty() { tr("the site gave no code").to_owned() } else { why });
    }
    Ok(code)
}

fn token_of(answer: TokenAnswer, fallback_refresh: &str) -> SiteToken {
    SiteToken {
        access: answer.access_token,
        refresh: if answer.refresh_token.is_empty() { fallback_refresh.to_owned() } else { answer.refresh_token },
        expires: crate::sessions::now() + answer.expires_in.max(60),
    }
}

/// Polls the site until the user has entered the code, the code expires or
/// the site refuses: the token, or why there is none.
pub fn wait_for_token(login: &Login, client_id: &str, code: &DeviceCode) -> Result<SiteToken, String> {
    let mut interval = code.interval.clamp(1, 60);
    let until = crate::sessions::now() + code.expires_in.clamp(30, 1800);
    loop {
        std::thread::sleep(Duration::from_secs(interval));
        if crate::sessions::now() > until {
            return Err(tr("the code expired before it was entered").to_owned());
        }
        let text = post(&login.token, &[("grant_type", "urn:ietf:params:oauth:grant-type:device_code"), ("device_code", &code.device_code), ("client_id", client_id)])?;
        let answer: TokenAnswer = serde_json::from_str(&text).map_err(|e| tf!("unreadable answer ({e})" [e]))?;
        if !answer.access_token.is_empty() {
            return Ok(token_of(answer, ""));
        }
        match answer.error.as_str() {
            "authorization_pending" => {}
            "slow_down" => interval += 5,
            "" => return Err(tr("the site gave no token").to_owned()),
            other => return Err(if answer.error_description.is_empty() { other.to_owned() } else { answer.error_description }),
        }
    }
}

/// A new access token from the refresh token.
pub fn refresh(login: &Login, client_id: &str, token: &SiteToken) -> Result<SiteToken, String> {
    if token.refresh.is_empty() {
        return Err(tr("the site gave no token").to_owned());
    }
    let text = post(&login.token, &[("grant_type", "refresh_token"), ("refresh_token", &token.refresh), ("client_id", client_id)])?;
    let answer: TokenAnswer = serde_json::from_str(&text).map_err(|e| tf!("unreadable answer ({e})" [e]))?;
    if answer.access_token.is_empty() {
        return Err(if answer.error_description.is_empty() { answer.error } else { answer.error_description });
    }
    Ok(token_of(answer, &token.refresh))
}

pub fn start_device_code(login: Login, client_id: String) -> mpsc::Receiver<Result<DeviceCode, String>> {
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = send.send(device_code(&login, &client_id));
    });
    receive
}

pub fn start_waiting(login: Login, client_id: String, code: DeviceCode) -> mpsc::Receiver<Result<SiteToken, String>> {
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = send.send(wait_for_token(&login, &client_id, &code));
    });
    receive
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_token_knows_when_it_is_fresh_and_only_secure_sites_are_asked() {
        let now = crate::sessions::now();
        assert!(SiteToken { access: "a".into(), refresh: String::new(), expires: now + 3600 }.fresh());
        assert!(!SiteToken { access: "a".into(), refresh: String::new(), expires: now + 30 }.fresh());
        assert!(!SiteToken { access: String::new(), refresh: String::new(), expires: now + 3600 }.fresh());
        let login = Login { device: "http://a.example/device".into(), token: "http://a.example/token".into(), ..Login::default() };
        assert!(device_code(&login, "key").unwrap_err().contains("http://a.example/device"));
        assert!(refresh(&login, "key", &SiteToken { refresh: "r".into(), ..SiteToken::default() }).unwrap_err().contains("http://a.example/token"));
        assert_eq!(refresh(&login, "key", &SiteToken::default()).unwrap_err(), tr("the site gave no token"));
        // The answers of the site, read as the flow writes them.
        let answer: TokenAnswer = serde_json::from_str(r#"{"error": "slow_down"}"#).unwrap();
        assert_eq!((answer.error.as_str(), answer.access_token.as_str()), ("slow_down", ""));
        let token = token_of(serde_json::from_str(r#"{"access_token": "x", "expires_in": 3600, "token_type": "Bearer"}"#).unwrap(), "kept");
        assert_eq!((token.access.as_str(), token.refresh.as_str()), ("x", "kept"));
        assert!(token.fresh());
    }
}
