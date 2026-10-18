//! Authenticated CONNECT proxy. Payloads are opaque TLS bytes, never decrypted.
pub mod policy;
mod protocol;

use anyhow::{Context, Result, bail, ensure};
use policy::Policy;
use rand::{RngCore, rngs::OsRng};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{Semaphore, watch},
    task::{JoinHandle, JoinSet},
    time::timeout,
};

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const IDLE_TIMEOUT: Duration = Duration::from_secs(300);
const WRITE_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_CONNECTIONS: usize = 64;

pub struct Proxy {
    address: SocketAddr,
    token: String,
    shutdown: watch::Sender<bool>,
    task: Option<JoinHandle<Result<()>>>,
}
impl Proxy {
    pub async fn start(policy: Policy) -> Result<Self> {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await?;
        let address = listener.local_addr()?;
        let mut random = [0_u8; 32];
        OsRng
            .try_fill_bytes(&mut random)
            .context("secure random source unavailable")?;
        let token = hex::encode(random);
        let expected = protocol::authorization(&token);
        let (shutdown, receiver) = watch::channel(false);
        let task = tokio::spawn(serve(listener, policy, expected, receiver));
        Ok(Self {
            address,
            token,
            shutdown,
            task: Some(task),
        })
    }
    /// Secret-bearing URL: callers must not log or persist this value.
    pub fn url(&self) -> String {
        format!("http://claude:{}@{}", self.token, self.address)
    }
    pub fn address(&self) -> SocketAddr {
        self.address
    }
    pub async fn stop(mut self) -> Result<()> {
        let _ = self.shutdown.send(true);
        if let Some(task) = self.task.take() {
            task.await.context("proxy task failed")??;
        }
        Ok(())
    }
}
impl Drop for Proxy {
    fn drop(&mut self) {
        let _ = self.shutdown.send(true);
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}

async fn serve(
    listener: TcpListener,
    policy: Policy,
    expected: String,
    mut shutdown: watch::Receiver<bool>,
) -> Result<()> {
    let limit = Arc::new(Semaphore::new(MAX_CONNECTIONS));
    let policy = Arc::new(policy);
    let expected = Arc::new(expected);
    let mut connections = JoinSet::new();
    loop {
        tokio::select! {
            _ = shutdown.changed() => break,
            accepted = listener.accept() => {
                let (socket,_) = accepted?;
                let Ok(permit) = limit.clone().try_acquire_owned() else { drop(socket); continue; };
                let policy=policy.clone(); let expected=expected.clone();
                connections.spawn(async move {
                    let _permit=permit;
                    // Do not log authorities, auth headers, proxy URLs or payloads.
                    let _ = handle(socket,&policy,&expected).await;
                });
            }
            Some(_) = connections.join_next(), if !connections.is_empty() => {}
        }
    }
    connections.abort_all();
    while connections.join_next().await.is_some() {}
    Ok(())
}

async fn reject(socket: &mut TcpStream, status: &str) -> Result<()> {
    let response = format!(
        "HTTP/1.1 {status}\r\nProxy-Authenticate: Basic realm=\"claude-termux\"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    );
    timeout(WRITE_TIMEOUT, socket.write_all(response.as_bytes())).await??;
    Ok(())
}

async fn handle(mut socket: TcpStream, policy: &Policy, expected: &str) -> Result<()> {
    let handshake = timeout(HANDSHAKE_TIMEOUT, read_request(&mut socket, expected)).await;
    let (hostname, early) = match handshake {
        Ok(Ok(Some(request))) => request,
        Ok(Ok(None)) => {
            reject(&mut socket, "407 Proxy Authentication Required").await?;
            return Ok(());
        }
        _ => {
            reject(&mut socket, "400 Bad Request").await?;
            return Ok(());
        }
    };
    if !policy.permits(&hostname) {
        reject(&mut socket, "403 Forbidden").await?;
        return Ok(());
    }
    let upstream = timeout(CONNECT_TIMEOUT, connect_public(&hostname)).await;
    let mut upstream = match upstream {
        Ok(Ok(socket)) => socket,
        _ => {
            reject(&mut socket, "502 Bad Gateway").await?;
            return Ok(());
        }
    };
    timeout(
        WRITE_TIMEOUT,
        socket.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n"),
    )
    .await??;
    timeout(WRITE_TIMEOUT, upstream.write_all(&early)).await??;
    tunnel(&mut socket, &mut upstream).await
}

async fn read_request(socket: &mut TcpStream, expected: &str) -> Result<Option<(String, Vec<u8>)>> {
    let mut bytes = Vec::with_capacity(protocol::MAX_HEADERS);
    loop {
        let mut chunk = [0_u8; 2048];
        let n = socket.read(&mut chunk).await?;
        ensure!(n > 0, "client closed during handshake");
        bytes.extend_from_slice(&chunk[..n]);
        match protocol::parse(&bytes, expected.as_bytes())? {
            protocol::Request::Incomplete => {}
            protocol::Request::Unauthorized => return Ok(None),
            protocol::Request::Tunnel { hostname, consumed } => {
                return Ok(Some((hostname.to_owned(), bytes[consumed..].to_vec())));
            }
        }
    }
}

async fn connect_public(hostname: &str) -> Result<TcpStream> {
    let addresses: Vec<_> = tokio::net::lookup_host((hostname, 443)).await?.collect();
    ensure!(
        !addresses.is_empty() && addresses.iter().all(|a| policy::public_address(a.ip())),
        "non-public destination rejected"
    );
    // Connect to the checked IP, never resolve the hostname a second time.
    for address in addresses {
        if let Ok(Ok(socket)) = timeout(Duration::from_secs(4), TcpStream::connect(address)).await {
            return Ok(socket);
        }
    }
    bail!("no reachable public destination")
}

/// Reset the idle deadline for traffic in either direction; preserve half-closes.
async fn tunnel(client: &mut TcpStream, upstream: &mut TcpStream) -> Result<()> {
    let mut client_buffer = [0_u8; 8192];
    let mut upstream_buffer = [0_u8; 8192];
    let mut client_open = true;
    let mut upstream_open = true;
    while client_open || upstream_open {
        timeout(IDLE_TIMEOUT, async {
            tokio::select! {
                result=client.read(&mut client_buffer), if client_open=>{
                    let n=result?;
                    if n==0 {client_open=false;upstream.shutdown().await?;}
                    else {timeout(WRITE_TIMEOUT,upstream.write_all(&client_buffer[..n])).await??;}
                }
                result=upstream.read(&mut upstream_buffer), if upstream_open=>{
                    let n=result?;
                    if n==0 {upstream_open=false;client.shutdown().await?;}
                    else {timeout(WRITE_TIMEOUT,client.write_all(&upstream_buffer[..n])).await??;}
                }
            }
            Ok::<_, anyhow::Error>(())
        })
        .await??;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn unauthenticated_connection_is_rejected_and_shutdown_closes_port() -> Result<()> {
        let proxy = Proxy::start(Policy::new([])?).await?;
        let address = proxy.address();
        let mut socket = TcpStream::connect(address).await?;
        socket
            .write_all(b"CONNECT api.anthropic.com:443 HTTP/1.1\r\n\r\n")
            .await?;
        let mut response = Vec::new();
        timeout(Duration::from_secs(2), socket.read_to_end(&mut response)).await??;
        assert!(response.starts_with(b"HTTP/1.1 407"));
        proxy.stop().await?;
        assert!(TcpStream::connect(address).await.is_err());
        Ok(())
    }
    #[tokio::test]
    async fn authenticated_unlisted_host_is_rejected_without_connecting() -> Result<()> {
        let proxy = Proxy::start(Policy::new([])?).await?;
        let auth = protocol::authorization(&proxy.token);
        let mut socket = TcpStream::connect(proxy.address()).await?;
        socket
            .write_all(
                format!(
                    "CONNECT forbidden.invalid:443 HTTP/1.1\r\nProxy-Authorization: {auth}\r\n\r\n"
                )
                .as_bytes(),
            )
            .await?;
        let mut response = Vec::new();
        timeout(Duration::from_secs(2), socket.read_to_end(&mut response)).await??;
        assert!(response.starts_with(b"HTTP/1.1 403"));
        proxy.stop().await?;
        Ok(())
    }
    #[tokio::test]
    async fn tunnel_preserves_response_after_client_half_close() -> Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let mut client = TcpStream::connect(listener.local_addr()?).await?;
        let (mut relay_client, _) = listener.accept().await?;
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let mut relay_upstream = TcpStream::connect(listener.local_addr()?).await?;
        let (mut server, _) = listener.accept().await?;
        let relay =
            tokio::spawn(async move { tunnel(&mut relay_client, &mut relay_upstream).await });
        client.write_all(b"request").await?;
        client.shutdown().await?;
        let mut request = Vec::new();
        server.read_to_end(&mut request).await?;
        assert_eq!(request, b"request");
        server.write_all(b"response").await?;
        server.shutdown().await?;
        let mut response = Vec::new();
        client.read_to_end(&mut response).await?;
        assert_eq!(response, b"response");
        relay.await??;
        Ok(())
    }
}
