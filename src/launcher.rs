//! Foreground process supervision: the proxy lives exactly as long as the child.
use crate::{
    installation,
    proxy::{Proxy, policy::Policy},
};
use anyhow::{Context, Result, ensure};
use nix::{
    sys::signal::{Signal, kill},
    unistd::Pid,
};
use std::{
    path::{Path, PathBuf},
    process::ExitStatus,
    time::Duration,
};
use tokio::{
    process::Command,
    signal::unix::{SignalKind, signal},
    time::timeout,
};

pub async fn run(root: &Path, allow_hosts: Vec<String>, args: Vec<String>) -> Result<i32> {
    ensure!(
        std::env::consts::OS == "android" && std::env::consts::ARCH == "aarch64",
        "execution currently targets aarch64 Termux only"
    );
    ensure!(
        !requests_background_entry_point(&args),
        "background services are not yet supported; use a foreground session"
    );
    let _session = installation::lock(root, false)?;
    let manifest = installation::verify_active(root)?;
    let release = installation::release_path(root, &manifest.claude_version)?;
    let proxy = Proxy::start(Policy::new(allow_hosts)?).await?;
    let proxy_url = proxy.url();
    let home = PathBuf::from(std::env::var_os("HOME").context("HOME is not set")?);
    let config = std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config/claude-termux/claude"));
    ensure!(config.is_absolute(), "CLAUDE_CONFIG_DIR must be absolute");
    installation::private_directory(&config)?;
    let temp = config.join("tmp");
    installation::private_directory(&temp)?;
    let mut command = Command::new(release.join("claude"));
    command
        .args(&args)
        .kill_on_drop(true)
        .env_remove("LD_PRELOAD")
        .env_remove("LD_LIBRARY_PATH")
        .env("CLAUDE_CONFIG_DIR", &config)
        .env(
            "CLAUDE_CODE_SHELL",
            "/data/data/com.termux/files/usr/bin/bash",
        )
        .env("TMPDIR", &temp)
        .env("CLAUDE_CODE_TMPDIR", &temp)
        .env("DISABLE_UPDATES", "1")
        .env("DISABLE_AUTOUPDATER", "1")
        .env("CLAUDE_CODE_PROXY_RESOLVES_HOSTS", "1")
        .env("NO_PROXY", "localhost,127.0.0.1,::1")
        .env("no_proxy", "localhost,127.0.0.1,::1");
    for name in [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "http_proxy",
        "https_proxy",
        "all_proxy",
    ] {
        command.env(name, &proxy_url);
    }
    // Preserve TLS validation. Do not accept an ambient setting that disables it.
    ensure!(
        std::env::var("NODE_TLS_REJECT_UNAUTHORIZED").as_deref() != Ok("0"),
        "refusing disabled TLS certificate validation"
    );
    let cert = "/data/data/com.termux/files/usr/etc/tls/cert.pem";
    if Path::new(cert).is_file() {
        command.env("SSL_CERT_FILE", cert);
    }
    let mut interrupt = signal(SignalKind::interrupt())?;
    let mut terminate = signal(SignalKind::terminate())?;
    let mut child = command
        .spawn()
        .context("failed to start verified Claude binary")?;
    let status: ExitStatus = loop {
        tokio::select! {
            status=child.wait()=>break status?,
            _=interrupt.recv()=>{
                // Claude shares the terminal process group and receives Ctrl-C
                // itself. It uses Ctrl-C to cancel a turn without ending a session.
                continue;
            }
            _=terminate.recv()=>{
                if let Some(id)=child.id(){let _=kill(Pid::from_raw(i32::try_from(id)?),Signal::SIGTERM);}
                break match timeout(Duration::from_secs(5),child.wait()).await {
                    Ok(status)=>status?,
                    Err(_)=>{child.start_kill()?;child.wait().await?}
                };
            }
        }
    };
    proxy.stop().await?;
    use std::os::unix::process::ExitStatusExt;
    Ok(status
        .code()
        .unwrap_or_else(|| 128 + status.signal().unwrap_or(1)))
}

// Forward ordinary argument values unchanged: a prompt can legitimately be
// "agents" or "daemon". This is only an early error for obvious entry points,
// not an attempt to duplicate Claude's complete, evolving argument parser.
fn requests_background_entry_point(args: &[String]) -> bool {
    args.first().is_some_and(|argument| {
        ["--bg", "agents", "daemon", "remote-control"].contains(&argument.as_str())
            || argument.starts_with("--bg=")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forwards_prompt_values_that_look_like_subcommands() {
        for prompt in ["agents", "daemon", "remote-control", "--bg"] {
            assert!(!requests_background_entry_point(&[
                "-p".into(),
                prompt.into()
            ]));
        }
    }

    #[test]
    fn reports_obvious_unsupported_background_entry_points() {
        for command in ["agents", "daemon", "remote-control", "--bg", "--bg=task"] {
            assert!(requests_background_entry_point(&[command.into()]));
        }
    }
}
