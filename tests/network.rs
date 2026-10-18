//! Opt-in tests: no account credentials, no API key, and no inference requests.
use anyhow::Result;
use claude_termux::proxy::{Proxy, policy::Policy};
use std::time::Duration;

#[tokio::test]
#[ignore = "requires outbound HTTPS; run explicitly"]
async fn reaches_anthropic_through_authenticated_tunnel() -> Result<()> {
    let proxy = Proxy::start(Policy::new([])?).await?;
    let client = reqwest::Client::builder()
        .proxy(reqwest::Proxy::all(proxy.url())?)
        .timeout(Duration::from_secs(20))
        .build()?;
    let response = client
        .get("https://api.anthropic.com/v1/models")
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
    proxy.stop().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires outbound HTTPS to a deliberately expired-certificate test server"]
async fn rejects_invalid_destination_certificate() -> Result<()> {
    let proxy = Proxy::start(Policy::new(["expired.badssl.com".to_owned()])?).await?;
    let client = reqwest::Client::builder()
        .proxy(reqwest::Proxy::all(proxy.url())?)
        .timeout(Duration::from_secs(20))
        .build()?;
    let result = client.get("https://expired.badssl.com/").send().await;
    let error = result
        .err()
        .ok_or_else(|| anyhow::anyhow!("an expired TLS certificate was accepted"))?;
    let detail = format!("{error:?}").to_lowercase();
    assert!(
        detail.contains("certificate") && detail.contains("expired"),
        "expected certificate-expiry rejection, got: {detail}"
    );
    proxy.stop().await?;
    Ok(())
}
