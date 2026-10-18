use anyhow::{Context, Result, ensure};
use reqwest::{Client, Url, redirect::Policy};
use std::{path::Path, time::Duration};
use tokio::io::AsyncWriteExt;

pub fn client() -> Result<Client> {
    Ok(Client::builder()
        .https_only(true)
        .redirect(Policy::none())
        .no_proxy()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(600))
        .user_agent(concat!("claude-termux/", env!("CARGO_PKG_VERSION")))
        .build()?)
}

pub fn validate_url(url: &str, host: &str) -> Result<Url> {
    let parsed = Url::parse(url)?;
    ensure!(
        parsed.scheme() == "https" && parsed.host_str() == Some(host),
        "unexpected artifact origin"
    );
    ensure!(
        parsed.username().is_empty() && parsed.password().is_none() && parsed.port().is_none(),
        "unexpected artifact URL credentials or port"
    );
    ensure!(
        parsed.query().is_none() && parsed.fragment().is_none(),
        "unexpected artifact URL parameters"
    );
    Ok(parsed)
}

pub async fn bytes(client: &Client, url: &str, maximum: usize) -> Result<Vec<u8>> {
    let mut response = client.get(url).send().await?.error_for_status()?;
    let mut output = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        ensure!(
            output.len().saturating_add(chunk.len()) <= maximum,
            "download exceeds size limit"
        );
        output.extend_from_slice(&chunk);
    }
    Ok(output)
}

pub async fn file(client: &Client, url: &str, path: &Path, maximum: u64) -> Result<()> {
    let mut response = client.get(url).send().await?.error_for_status()?;
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .await?;
    let mut received = 0_u64;
    while let Some(chunk) = response
        .chunk()
        .await
        .context("artifact download interrupted")?
    {
        received = received.saturating_add(chunk.len() as u64);
        ensure!(received <= maximum, "download exceeds size limit");
        file.write_all(&chunk).await?;
    }
    file.sync_all().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_origin_confusion() {
        for url in [
            "http://registry.npmjs.org/a",
            "https://registry.npmjs.org.evil.test/a",
            "https://evil.test@registry.npmjs.org/a",
            "https://registry.npmjs.org:8443/a",
            "https://registry.npmjs.org/a?redirect=evil",
        ] {
            assert!(validate_url(url, "registry.npmjs.org").is_err(), "{url}");
        }
        assert!(validate_url("https://registry.npmjs.org/a", "registry.npmjs.org").is_ok());
    }
}
