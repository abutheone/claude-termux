//! Explicit network probes use public endpoints, no account tokens or inference.
use super::{Check, Status};
use crate::{
    artifacts::{alpine, download, npm},
    proxy::{Proxy, policy::Policy},
};
use anyhow::{Result, ensure};
use std::time::Duration;
use tokio::time::timeout;

const DEADLINE: Duration = Duration::from_secs(15);

pub(super) async fn examine(version: &str) -> Vec<Check> {
    let (npm, alpine, anthropic) = tokio::join!(
        probe(
            "network_npm",
            "Installer HTTPS: npm metadata signature verified; the runtime proxy allowlist is unchanged.",
            async {
                let client = download::client()?;
                npm::metadata(&client, version).await?;
                Ok(())
            }
        ),
        probe(
            "network_alpine",
            "Pinned Alpine loader signature and payload verified (small in-memory download).",
            async {
                let client = download::client()?;
                alpine::fetch(&client).await?;
                Ok(())
            }
        ),
        probe(
            "network_anthropic",
            "Authenticated local proxy reached Anthropic over verified HTTPS; no account credential sent.",
            anthropic()
        ),
    );
    vec![npm, alpine, anthropic]
}

async fn probe(
    id: &'static str,
    success: &str,
    future: impl std::future::Future<Output = Result<()>>,
) -> Check {
    match timeout(DEADLINE, future).await {
        Ok(Ok(())) => Check::new(id, Status::Ok, success, None),
        Ok(Err(error)) => Check::new(
            id,
            Status::Error,
            error.to_string(),
            Some(
                "Check your internet connection and trusted certificates. A signature failure requires reviewing the pinned upstream release/key; never bypass verification.",
            ),
        ),
        Err(_) => Check::new(
            id,
            Status::Error,
            "Network check timed out after 15 seconds.",
            Some(
                "Check your connection and retry doctor --network. Local checks work without network access.",
            ),
        ),
    }
}

async fn anthropic() -> Result<()> {
    let proxy = Proxy::start(Policy::new([])?).await?;
    let result = request_anthropic(&proxy).await;
    proxy.stop().await?;
    result
}

async fn request_anthropic(proxy: &Proxy) -> Result<()> {
    let proxy_setting = reqwest::Proxy::all(proxy.url())
        .map_err(|_| anyhow::anyhow!("Could not configure the local proxy."))?;
    let client = reqwest::Client::builder()
        .https_only(true)
        .no_proxy()
        .proxy(proxy_setting)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(DEADLINE)
        .build()
        .map_err(|_| anyhow::anyhow!("Could not initialize verified HTTPS."))?;
    let response = client
        .get("https://api.anthropic.com/v1/models")
        .send()
        .await
        .map_err(|_| {
            anyhow::anyhow!(
                "Could not reach Anthropic through the local proxy with certificate validation enabled."
            )
        })?;
    ensure!(
        response.status() == reqwest::StatusCode::UNAUTHORIZED,
        "Anthropic connectivity probe returned HTTP {}; expected 401 without credentials.",
        response.status().as_u16()
    );
    Ok(())
}
