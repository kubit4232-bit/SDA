//! HTTP-клиент для SDA

use reqwest::{Client, Proxy};
use std::time::Duration;

/// Овершоп над HTTP-клиентом с настройками таймаутов, User-Agent, прокси и Basic Auth.
#[derive(Clone)]
pub struct HttpClient {
    inner: Client,
    basic_user: Option<String>,
    basic_pass: Option<String>,
    cookie_header: Option<String>,
    proxy_url: Option<String>,
    user_agent: String,
    connect_timeout: Duration,
    read_timeout: Duration,
}

impl HttpClient {
    pub fn new(user_agent: &str, connect_timeout_secs: u64, read_timeout_secs: u64) -> Self {
        Self::with_config(user_agent, connect_timeout_secs, read_timeout_secs, None)
    }

    pub fn with_config(
        user_agent: &str,
        connect_timeout_secs: u64,
        read_timeout_secs: u64,
        proxy_url: Option<&str>,
    ) -> Self {
        let mut builder = Client::builder()
            .user_agent(user_agent)
            .connect_timeout(Duration::from_secs(connect_timeout_secs))
            .read_timeout(Duration::from_secs(read_timeout_secs))
            .timeout(Duration::from_secs(read_timeout_secs));

        if let Some(proxy) = proxy_url {
            if let Ok(parsed) = reqwest::Url::parse(proxy) {
                if let Ok(proxy) = Proxy::all(parsed) {
                    builder = builder.proxy(proxy);
                }
            }
        }

        Self {
            inner: builder.build().expect("Failed to create HTTP client"),
            basic_user: None,
            basic_pass: None,
            cookie_header: None,
            proxy_url: proxy_url.map(str::to_string),
            user_agent: user_agent.to_string(),
            connect_timeout: Duration::from_secs(connect_timeout_secs),
            read_timeout: Duration::from_secs(read_timeout_secs),
        }
    }

    /// Авторизация на прокси-сервере (вкладка «Прокси» IDM).
    pub fn with_proxy_auth(self, username: &str, password: &str) -> Self {
        let Some(proxy_url) = self.proxy_url.clone() else {
            return self;
        };
        let Ok(parsed) = reqwest::Url::parse(&proxy_url) else {
            return self;
        };
        let mut proxy = Proxy::all(parsed).expect("Invalid proxy URL");
        proxy = proxy.basic_auth(username, password);
        // Сохраняем User-Agent и таймауты — иначе авторизованный прокси
        // пересоздавал клиент с "SDA" и без таймаутов из настроек.
        let inner = Client::builder()
            .user_agent(self.user_agent.clone())
            .connect_timeout(self.connect_timeout)
            .read_timeout(self.read_timeout)
            .timeout(self.read_timeout)
            .proxy(proxy)
            .build()
            .expect("Failed to create authenticated proxy client");
        Self { inner, ..self }
    }

    pub fn with_basic_auth(mut self, username: Option<&str>, password: Option<&str>) -> Self {
        self.basic_user = username.map(|s| s.to_string());
        self.basic_pass = password.map(|s| s.to_string());
        self
    }

    /// Cookie-цепочка (строка заголовка `Cookie`) для запросов к серверу.
    pub fn with_cookie_header(mut self, cookie_header: Option<&str>) -> Self {
        self.cookie_header = cookie_header.map(|s| s.trim().to_string());
        self
    }

    pub fn basic_auth(&self) -> Option<(&str, &str)> {
        self.basic_user
            .as_deref()
            .zip(self.basic_pass.as_deref())
            .map(|(u, p)| (u, p))
    }

    fn auth(&self) -> Option<(String, String)> {
        self.basic_user
            .as_deref()
            .map(|u| (u.to_string(), self.basic_pass.clone().unwrap_or_default()))
            .filter(|_| !self.basic_pass.as_deref().unwrap_or("").is_empty())
    }

    pub async fn head(&self, url: &str) -> Result<reqwest::Response, anyhow::Error> {
        let mut req = self.inner.head(url);
        if let Some((u, p)) = self.auth() {
            req = req.basic_auth(u, Some(p));
        }
        if let Some(cookie) = &self.cookie_header {
            if !cookie.is_empty() {
                req = req.header(reqwest::header::COOKIE, cookie);
            }
        }
        Ok(req.send().await?.error_for_status()?)
    }

    pub async fn get(&self, url: &str) -> Result<reqwest::Response, anyhow::Error> {
        let mut req = self.inner.get(url);
        if let Some((u, p)) = self.auth() {
            req = req.basic_auth(u, Some(p));
        }
        if let Some(cookie) = &self.cookie_header {
            if !cookie.is_empty() {
                req = req.header(reqwest::header::COOKIE, cookie);
            }
        }
        Ok(req.send().await?.error_for_status()?)
    }

    pub async fn get_ranged(
        &self,
        url: &str,
        range: &str,
    ) -> Result<reqwest::Response, anyhow::Error> {
        let mut req = self.inner.get(url).header(reqwest::header::RANGE, range);
        if let Some((u, p)) = self.auth() {
            req = req.basic_auth(u, Some(p));
        }
        if let Some(cookie) = &self.cookie_header {
            if !cookie.is_empty() {
                req = req.header(reqwest::header::COOKIE, cookie);
            }
        }
        Ok(req.send().await?.error_for_status()?)
    }
}
