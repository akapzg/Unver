use std::sync::Arc;
use std::sync::RwLock;
use std::collections::HashMap;
use std::time::Duration;
use axum::http::{
    header::{
        CONNECTION, HOST, LOCATION, PROXY_AUTHENTICATE, PROXY_AUTHORIZATION, TE, TRAILER,
        TRANSFER_ENCODING, UPGRADE, SEC_WEBSOCKET_ACCEPT, SEC_WEBSOCKET_KEY,
        SEC_WEBSOCKET_PROTOCOL, SEC_WEBSOCKET_EXTENSIONS, SEC_WEBSOCKET_VERSION,
        STRICT_TRANSPORT_SECURITY,
    },
    HeaderMap, HeaderName, Request, Response, StatusCode,
};
use bytes::Bytes;
use futures_util::StreamExt;
use http_body_util::{BodyExt, Empty, Full, StreamBody, combinators::UnsyncBoxBody};
use hyper::body::Incoming;
use http_body::Frame;
use hyper::server::conn::{http1, http2};
use hyper::service::service_fn;
use hyper_util::client::legacy::{connect::HttpConnector, Client};
use hyper_util::rt::{TokioExecutor, TokioIo};
use tokio::io::{AsyncBufReadExt, BufReader};
use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::crypto::ring;
use rustls::sign::CertifiedKey;
use rustls::ServerConfig;
use tokio_rustls::TlsAcceptor;
use crate::logger;

use crate::{errors::AppResult, state::AppState, errors::AppError};

// ── Types ─────────────────────────────────────────────────────────────────

type BoxError = Box<dyn std::error::Error + Send + Sync>;
type ProxyBody = UnsyncBoxBody<Bytes, BoxError>;
type ProxyClient = Client<hyper_rustls::HttpsConnector<HttpConnector>, ProxyBody>;

/// Idle timeout: close connection when no frame transferred for this duration.
const IDLE_TIMEOUT: Duration = Duration::from_secs(300);

/// Resolve real client IP: if peer is a trusted proxy, read from X-Forwarded-For
fn real_client_ip(peer: std::net::SocketAddr, headers: &HeaderMap, trusted: &[std::net::IpAddr]) -> std::net::IpAddr {
    let peer_ip = peer.ip();
    if trusted.contains(&peer_ip) {
        if let Some(xff) = headers.get("x-forwarded-for") {
            if let Ok(val) = xff.to_str() {
                if let Some(first) = val.split(',').next() {
                    if let Ok(ip) = first.trim().parse::<std::net::IpAddr>() {
                        return ip;
                    }
                }
            }
        }
    }
    peer_ip
}

// ── Shared client pool ────────────────────────────────────────────────────

fn build_verified_client() -> ProxyClient {
    let mut connector = HttpConnector::new();
    connector.set_connect_timeout(Some(Duration::from_secs(5)));
    connector.set_nodelay(true);

    // Load system root certificates
    let mut root_store = rustls::RootCertStore::empty();
    for cert in rustls_native_certs::load_native_certs().certs {
        let _ = root_store.add(cert);
    }

    let tls_config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    let https = hyper_rustls::HttpsConnectorBuilder::new()
        .with_tls_config(tls_config)
        .https_or_http()
        .enable_http1()
        .wrap_connector(connector);

    Client::builder(hyper_util::rt::TokioExecutor::new())
        .pool_idle_timeout(Duration::from_secs(90))
        .pool_max_idle_per_host(4)
        .build(https)
}

fn build_skip_verify_client() -> ProxyClient {
    let mut connector = HttpConnector::new();
    connector.set_connect_timeout(Some(Duration::from_secs(5)));
    connector.set_nodelay(true);

    let verifier = Arc::new(SkipCertVerifier);
    let tls_config = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();

    let https = hyper_rustls::HttpsConnectorBuilder::new()
        .with_tls_config(tls_config)
        .https_or_http()
        .enable_http1()
        .wrap_connector(connector);

    Client::builder(hyper_util::rt::TokioExecutor::new())
        .pool_idle_timeout(Duration::from_secs(90))
        .pool_max_idle_per_host(4)
        .build(https)
}

// ── TLS cert verifier (skip for upstream) ─────────────────────────────────

#[derive(Debug)]
struct SkipCertVerifier;

impl rustls::client::danger::ServerCertVerifier for SkipCertVerifier {
    fn verify_server_cert(&self, _: &rustls::pki_types::CertificateDer<'_>, _: &[rustls::pki_types::CertificateDer<'_>], _: &rustls::pki_types::ServerName<'_>, _: &[u8], _: rustls::pki_types::UnixTime) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(&self, _: &[u8], _: &rustls::pki_types::CertificateDer<'_>, _: &rustls::DigitallySignedStruct) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn verify_tls13_signature(&self, _: &[u8], _: &rustls::pki_types::CertificateDer<'_>, _: &rustls::DigitallySignedStruct) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![rustls::SignatureScheme::RSA_PKCS1_SHA256, rustls::SignatureScheme::ECDSA_NISTP256_SHA256, rustls::SignatureScheme::RSA_PKCS1_SHA384, rustls::SignatureScheme::ECDSA_NISTP384_SHA384]
    }
}

// ── SNI extraction from raw ClientHello bytes ────────────────────────────

/// Parse the SNI hostname from raw TLS ClientHello bytes without full handshake.
/// Returns `None` if the ClientHello is malformed or has no SNI extension.
fn extract_sni(buf: &[u8]) -> Option<String> {
    // Minimum: TLS record(5) + handshake header(4) + version(2) + random(32)
    if buf.len() < 43 || buf[0] != 0x16 { return None; }

    // Read TLS record length (big-endian, bytes 3-4)
    let record_len = u16::from_be_bytes([buf[3], buf[4]]) as usize;
    if record_len + 5 > buf.len() { return None; }

    // Handshake type must be ClientHello (0x01)
    if buf[5] != 0x01 { return None; }

    // Handshake length (3 bytes, starting at 6)
    let hs_len = ((buf[6] as usize) << 16) | ((buf[7] as usize) << 8) | (buf[8] as usize);
    if hs_len + 9 > buf.len() { return None; }

    // Skip version(2) + random(32) + session_id
    let mut pos = 9 + 2 + 32;
    if pos >= buf.len() { return None; }
    let sid_len = buf[pos] as usize;
    pos += 1 + sid_len;
    if pos + 2 > buf.len() { return None; }

    // Cipher suites
    let cs_len = u16::from_be_bytes([buf[pos], buf[pos + 1]]) as usize;
    pos += 2 + cs_len;
    if pos + 1 > buf.len() { return None; }

    // Compression methods
    let cm_len = buf[pos] as usize;
    pos += 1 + cm_len;
    if pos + 2 > buf.len() { return None; }

    // Extensions
    let ext_total = u16::from_be_bytes([buf[pos], buf[pos + 1]]) as usize;
    pos += 2;
    let ext_end = pos + ext_total;
    if ext_end > buf.len() { return None; }

    // Iterate extensions looking for SNI (type 0x0000)
    while pos + 4 <= ext_end {
        let ext_type = u16::from_be_bytes([buf[pos], buf[pos + 1]]);
        let ext_len = u16::from_be_bytes([buf[pos + 2], buf[pos + 3]]) as usize;
        pos += 4;
        if pos + ext_len > ext_end { return None; }

        if ext_type == 0x0000 {
            // SNI extension
            if pos + 2 > ext_end { return None; }
            let list_len = u16::from_be_bytes([buf[pos], buf[pos + 1]]) as usize;
            pos += 2;
            let list_end = pos + list_len;
            if list_end > ext_end { return None; }

            while pos + 3 <= list_end {
                let name_type = buf[pos];
                let name_len = u16::from_be_bytes([buf[pos + 1], buf[pos + 2]]) as usize;
                pos += 3;
                if pos + name_len > list_end { return None; }
                if name_type == 0 {
                    return String::from_utf8(buf[pos..pos + name_len].to_vec()).ok();
                }
                pos += name_len;
            }
            return None;
        }
        pos += ext_len;
    }
    None
}

// ── SNI-based TLS cert resolver ──────────────────────────────────────────

/// Resolves server certificates by SNI hostname using the in-memory cert cache.
/// The SSL manager keeps this cache in sync with issued Let's Encrypt certificates.
#[derive(Debug)]
struct CertResolver {
    cert_cache: Arc<RwLock<HashMap<String, Arc<CertifiedKey>>>>,
}

impl CertResolver {
    fn new(cert_cache: Arc<RwLock<HashMap<String, Arc<CertifiedKey>>>>) -> Self {
        Self { cert_cache }
    }
}

impl ResolvesServerCert for CertResolver {
    fn resolve(&self, client_hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        let sni = client_hello.server_name()?;
        let cache = self.cert_cache.read().ok()?;
        cache.get(sni).cloned()
    }
}

fn build_tls_server_config(
    cert_cache: Arc<RwLock<HashMap<String, Arc<CertifiedKey>>>>,
) -> Arc<ServerConfig> {
    let resolver = Arc::new(CertResolver::new(cert_cache));
    let provider = ring::default_provider();
    ServerConfig::builder_with_provider(provider.into())
        .with_safe_default_protocol_versions()
        .expect("Default TLS 1.2/1.3 versions should be valid")
        .with_no_client_auth()
        .with_cert_resolver(resolver)
        .into()
}

// ── Proxy engine entry point ──────────────────────────────────────────────

pub async fn run_proxy_engine(state: Arc<AppState>) {
    let verified_client = Arc::new(build_verified_client());
    let skip_verify_client = Arc::new(build_skip_verify_client());

    // Load enabled port groups
    let groups = match sqlx::query!(
        "SELECT id, name, listen_port, skip_tls_verify, force_https FROM port_groups WHERE enabled = 1 ORDER BY listen_port"
    )
    .fetch_all(&state.db)
    .await
    {
        Ok(rows) => rows,
        Err(e) => { tracing::error!("Failed to load port groups: {e}"); return; }
    };

    if groups.is_empty() {
        tracing::warn!("No port groups configured — proxy engine idle");
        return;
    }

    for g in groups {
        let pg_id = g.id.unwrap_or_default();
        let port = g.listen_port as u16;
        let skip_tls = g.skip_tls_verify != 0;
        let force_https = g.force_https != 0;

        let listener_state = Arc::clone(&state);
        let listener_client = if skip_tls {
            Arc::clone(&skip_verify_client)
        } else {
            Arc::clone(&verified_client)
        };
        let tls_config = build_tls_server_config(Arc::clone(&state.cert_cache));

        tokio::spawn(async move {
            let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
            let listener = match tokio::net::TcpListener::bind(addr).await {
                Ok(l) => l,
                Err(e) => {
                    tracing::warn!("Port group {pg_id} — cannot bind :{port}: {e}");
                    return;
                }
            };
            tracing::info!("Proxy listening on :{port} (group {pg_id})");

            loop {
                let (stream, peer) = match listener.accept().await {
                    Ok(v) => v,
                    Err(e) => { tracing::error!("Accept :{port}: {e}"); continue; }
                };
                let s = Arc::clone(&listener_state);
                let c = Arc::clone(&listener_client);
                let gid = pg_id.clone();
                let tls_cfg = tls_config.clone();
                let fh = force_https;

                tokio::spawn(async move {
                    if let Err(e) = serve_connection(stream, peer, s, c, &gid, tls_cfg, fh).await {
                        tracing::debug!("Proxy error: {e}");
                    }
                });
            }
        });
    }
}

// ── Per-connection handler (TLS detection + dispatch) ────────────────────

/// Detects TLS ClientHello by peeking the first byte, performs TLS handshake
/// if needed, then serves HTTP/1.1 on the resulting stream.
async fn serve_connection(
    stream: tokio::net::TcpStream,
    peer: std::net::SocketAddr,
    state: Arc<AppState>,
    client: Arc<ProxyClient>,
    port_group_id: &str,
    tls_config: Arc<ServerConfig>,
    force_https: bool,
) -> AppResult<()> {
    stream
        .set_nodelay(true)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("TCP_NODELAY: {e}")))?;

    let mut buf_stream = BufReader::new(stream);

    // Peek first byte: 0x16 = TLS ClientHello
    let is_tls = buf_stream
        .fill_buf()
        .await
        .map(|buf| buf.first() == Some(&0x16))
        .unwrap_or(false);

    // ── Non-TLS TCP tunnel (SSH, raw TCP, etc.) ────────────────────────
    // For non-TLS connections, check if this port group has any TCP rule.
    // No SNI available, so use the first matching TCP rule in the group.
    if !is_tls {
        let tcp_rule = sqlx::query!(
            "SELECT target_url FROM proxy_rules WHERE port_group_id = ? AND rule_type = 'tcp' AND enabled = 1 LIMIT 1",
            port_group_id
        ).fetch_optional(&state.db).await;

        if let Ok(Some(r)) = tcp_rule {
            use tokio::io::AsyncWriteExt;
            let buffered = buf_stream.buffer().to_vec();
            let backend_addr = r.target_url.clone();
            let mut client = buf_stream.into_inner();

            match tokio::net::TcpStream::connect(&backend_addr).await {
                Ok(mut backend) => {
                    let _ = backend.write_all(&buffered).await;
                    crate::logger::info(&state.db,
                        &format!("TCP tunnel (plain): {} → {backend_addr}", peer.ip())).await;
                    let _ = tokio::time::timeout(
                        tokio::time::Duration::from_secs(300),
                        tokio::io::copy_bidirectional(&mut client, &mut backend),
                    ).await;
                }
                Err(e) => {
                    crate::logger::error(&state.db,
                        &format!("TCP backend {backend_addr}: {e}")).await;
                }
            }
            return Ok(());
        }
    }

    if is_tls {
        // Extract SNI from buffered ClientHello for TCP rule matching
        let sni = buf_stream
            .fill_buf()
            .await
            .ok()
            .and_then(|buf| extract_sni(buf));

        // Check for TCP rule matching this SNI + port group
        if let Some(ref sni_domain) = sni {
            let tcp_rule = sqlx::query!(
                "SELECT target_url FROM proxy_rules WHERE domain = ? AND port_group_id = ? AND rule_type = 'tcp' AND enabled = 1",
                sni_domain, port_group_id
            ).fetch_optional(&state.db).await;

            if let Ok(Some(r)) = tcp_rule {
                // Raw TCP tunnel — forward buffered ClientHello + bidirectional pipe
                use tokio::io::AsyncWriteExt;
                let client_hello = buf_stream.buffer().to_vec();
                let backend_addr = r.target_url.clone();
                let mut client = buf_stream.into_inner();

                match tokio::net::TcpStream::connect(&backend_addr).await {
                    Ok(mut backend) => {
                        let _ = backend.write_all(&client_hello).await;
                        tracing::info!("TCP tunnel: {} → {backend_addr}", peer.ip());
                        let tcp_timeout = tokio::time::Duration::from_secs(300);
                        let _ = tokio::time::timeout(tcp_timeout, tokio::io::copy_bidirectional(&mut client, &mut backend)).await;
                    }
                    Err(e) => {
                        tracing::warn!("TCP backend {backend_addr}: {e}");
                    }
                }
                return Ok(());
            }
        }

        // Not a TCP rule — proceed with TLS termination + HTTP proxy
        let acceptor = TlsAcceptor::from(tls_config);
        match acceptor.accept(buf_stream).await {
            Ok(tls_stream) => {
                let mut io = BufReader::new(tls_stream);
                // Peek for HTTP/2 preface: "PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n"
                let is_h2 = io.fill_buf().await
                    .map(|buf| buf.len() >= 24 && &buf[..24] == b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n")
                    .unwrap_or(false);
                let tokio_io = TokioIo::new(io);
                if is_h2 {
                    handle_http2(tokio_io, peer, state, client, true, port_group_id).await
                } else {
                    handle_http1(tokio_io, peer, state, client, true, port_group_id).await
                }
            }
            Err(e) => {
                tracing::debug!("TLS handshake failed (SNI may not match any cert): {e}");
                Ok(())
            }
        }
    } else if force_https {
        // Plain HTTP not allowed — parse Host header, redirect to HTTPS
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
        let mut headers = Vec::new();
        // Read until end of HTTP headers (\r\n\r\n)
        loop {
            let available = buf_stream.fill_buf().await.unwrap_or(&[]);
            if available.is_empty() { break; }
            let pos = available.windows(4).position(|w| w == b"\r\n\r\n");
            let len = pos.map(|p| p + 4).unwrap_or(available.len());
            headers.extend_from_slice(&available[..len]);
            buf_stream.consume(len);
            if pos.is_some() { break; }
        }
        let header_str = String::from_utf8_lossy(&headers);
        let host = header_str
            .lines()
            .find(|l| l.to_lowercase().starts_with("host:"))
            .and_then(|l| l.split_once(':').map(|(_, v)| v.trim()))
            .unwrap_or("localhost");
        let path = header_str
            .lines()
            .next()
            .and_then(|l| l.split_whitespace().nth(1))
            .unwrap_or("/");
        let redirect = format!("Location: https://{host}{path}\r\n");
        let mut inner = buf_stream.into_inner();
        let _ = inner.write_all(
            format!("HTTP/1.1 301 Moved Permanently\r\n{redirect}Connection: close\r\nContent-Length: 0\r\n\r\n").as_bytes()
        ).await;
        Ok(())
    } else {
        let io = TokioIo::new(buf_stream);
        handle_http1(io, peer, state, client, false, port_group_id).await
    }
}

/// Serve HTTP/1.1 on an already-prepared async I/O stream.
async fn handle_http1<IO>(
    io: IO,
    peer: std::net::SocketAddr,
    state: Arc<AppState>,
    client: Arc<ProxyClient>,
    is_tls: bool,
    port_group_id: &str,
) -> AppResult<()>
where
    IO: hyper::rt::Read + hyper::rt::Write + Unpin + Send + 'static,
{
    http1::Builder::new()
        .serve_connection(io, service_fn(move |req| {
            let s = Arc::clone(&state);
            let c = Arc::clone(&client);
            let pg = port_group_id.to_string();
            async move { proxy_request(req, peer, s, c, is_tls, &pg).await }
        }))
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("{e}")))?;
    Ok(())
}

/// Serve HTTP/2 on an already-prepared async I/O stream.
async fn handle_http2<IO>(
    io: IO,
    peer: std::net::SocketAddr,
    state: Arc<AppState>,
    client: Arc<ProxyClient>,
    is_tls: bool,
    port_group_id: &str,
) -> AppResult<()>
where
    IO: hyper::rt::Read + hyper::rt::Write + Unpin + Send + 'static,
{
    http2::Builder::new(TokioExecutor::new())
        .serve_connection(io, service_fn(move |req| {
            let s = Arc::clone(&state);
            let c = Arc::clone(&client);
            let pg = port_group_id.to_string();
            async move { proxy_request(req, peer, s, c, is_tls, &pg).await }
        }))
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("{e}")))?;
    Ok(())
}

// ── Body helpers ──────────────────────────────────────────────────────────

fn bytes_body(s: &str) -> ProxyBody {
    let body = Full::new(Bytes::from(s.to_string()))
        .map_err(|e: std::convert::Infallible| match e {});
    UnsyncBoxBody::new(body)
}

fn empty() -> ProxyBody {
    let body = Empty::<Bytes>::new()
        .map_err(|e: std::convert::Infallible| match e {});
    UnsyncBoxBody::new(body)
}

/// Convert hyper `Incoming` body to streaming `ProxyBody` (zero buffering).
fn into_proxy_body(incoming: Incoming) -> ProxyBody {
    UnsyncBoxBody::new(incoming.map_err(|e| Box::new(e) as BoxError))
}

/// Convert `Incoming` body to `ProxyBody` with **idle timeout**.
///
/// If no frame arrives within `idle`, the stream closes silently.
/// Uses two heap allocations: `Box::pin` for the stream + `UnsyncBoxBody`.
fn into_proxy_body_idle(incoming: Incoming, idle: Duration) -> ProxyBody {
    let body_stream = Box::pin(incoming.into_data_stream());

    let timed = futures_util::stream::unfold(
        (body_stream, idle),
        |(mut stream, timeout)| async move {
            match tokio::time::timeout(timeout, stream.next()).await {
                Ok(Some(Ok(bytes))) => Some((Ok(Frame::data(bytes)), (stream, timeout))),
                Ok(Some(Err(e))) => Some((Err(Box::new(e) as BoxError), (stream, timeout))),
                Ok(None) => None,
                Err(_elapsed) => {
                    tracing::debug!("Upstream response idle timeout ({:.0}s)", timeout.as_secs());
                    None
                }
            }
        },
    );

    UnsyncBoxBody::new(StreamBody::new(timed))
}

// ── Core proxy request handler ────────────────────────────────────────────

async fn proxy_request(
    req: Request<Incoming>,
    peer: std::net::SocketAddr,
    state: Arc<AppState>,
    client: Arc<ProxyClient>,
    is_tls: bool,
    port_group_id: &str,
) -> Result<Response<ProxyBody>, hyper::Error> {
    let path = req.uri().path().to_string();

    // ACME HTTP-01 challenge
    if path.starts_with("/.well-known/acme-challenge/") {
        let token = path.strip_prefix("/.well-known/acme-challenge/").unwrap_or("");
        let auth = crate::state::get_setting(&state.db, &format!("acme_challenge_{}", token)).await.unwrap_or_default();
        if !auth.is_empty() {
            return Ok(Response::builder().status(StatusCode::OK).body(bytes_body(&auth)).unwrap());
        }
    }

    // Extract host
    let host = req.headers().get(HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
        .to_string();

    // Trusted proxies (needed for real IP in redirect/proxy logging)
    let trusted_str = crate::state::get_setting(&state.db, "trusted_proxy").await.unwrap_or_default();
    let trusted: Vec<std::net::IpAddr> = trusted_str.split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    // Match proxy rule by domain AND port group
    let rule = sqlx::query!(
        "SELECT target_url, rule_type, redirect_code FROM proxy_rules WHERE domain = ? AND port_group_id = ? AND enabled = 1",
        host, port_group_id
    ).fetch_optional(&state.db).await;

    let rule = match rule {
        Ok(Some(r)) => r,
        _ => return Ok(Response::builder()
            .status(StatusCode::BAD_GATEWAY)
            .body(bytes_body("No proxy rule found"))
            .unwrap()),
    };

    // ── Redirect rule: 301/302 ──────────────────────────────────────────
    if rule.rule_type == "redirect" {
        let code = rule.redirect_code.unwrap_or(301);
        let status = if code == 302 { StatusCode::FOUND }
            else if code == 307 { StatusCode::TEMPORARY_REDIRECT }
            else if code == 308 { StatusCode::PERMANENT_REDIRECT }
            else { StatusCode::MOVED_PERMANENTLY };
        let logger_host = host.clone();
        let logger_path = path.clone();
        let logger_target = rule.target_url.clone();
        let logger_code = code;
        let real_ip_clone = real_client_ip(peer, req.headers(), &trusted);
        tokio::spawn(async move {
            logger::info(&state.db, &format!("REDIRECT: {real_ip_clone} {logger_host}{logger_path} → {logger_code} {logger_target}")).await;
        });
        return Ok(Response::builder()
            .status(status)
            .header(LOCATION, &rule.target_url)
            .body(empty())
            .unwrap());
    }

    // ── TCP rule: forwarded in serve_connection, not here ───────────────
    if rule.rule_type == "tcp" {
        return Ok(Response::builder()
            .status(StatusCode::BAD_GATEWAY)
            .body(bytes_body("TCP rules are handled at connection level"))
            .unwrap());
    }

    // ── TCP rule: forwarded in serve_connection, not here ───────────────
    let uri_str = format!(
        "{}{}",
        rule.target_url.trim_end_matches('/'),
        req.uri().path_and_query().map(|p| p.as_str()).unwrap_or("/")
    );

    let method = req.method().clone();
    let headers = req.headers().clone();
    let is_ws = is_websocket_upgrade(&headers);

    // Split request into parts + body
    let (_parts, body) = req.into_parts();

    // Build upstream request (hop-by-hop cleaned)
    let mut upstream_req = Request::builder().method(&method).uri(&uri_str);
    for (name, value) in &headers {
        if should_keep_header_request(name, is_ws) && name != HOST {
            upstream_req = upstream_req.header(name, value);
        }
    }
    let proto = if is_tls { "https" } else { "http" };
    let real_ip = real_client_ip(peer, &headers, &trusted);
    upstream_req = upstream_req
        .header("x-forwarded-host", &host)
        .header("x-forwarded-proto", proto)
        .header("x-forwarded-for", real_ip.to_string());

    // Stream request body upstream (zero buffering)
    let upstream_body = into_proxy_body(body);
    let upstream_req = upstream_req.body(upstream_body).unwrap();

    // Receive response from upstream
    match client.request(upstream_req).await {
        Ok(resp) => {
            let status = resp.status();
            let upstream_headers = resp.headers().clone();
            let is_ws_resp = status == StatusCode::SWITCHING_PROTOCOLS;

            // Log proxy access
            let proto = if is_tls { "https" } else { "http" };
            tracing::debug!("PROXY: {real_ip} {proto}://{host} {method} {path} → {status}");

            // Build downstream response (hop-by-hop cleaned)
            let mut response = Response::builder().status(status);
            for (name, value) in &upstream_headers {
                if should_keep_header_response(name, is_ws || is_ws_resp) {
                    response = response.header(name, value);
                }
            }

            // HSTS: tell browsers to always use HTTPS for this domain
            if is_tls {
                response = response.header(
                    STRICT_TRANSPORT_SECURITY,
                    "max-age=31536000; includeSubDomains; preload",
                );
            }

            // Stream response body back with idle timeout
            let resp_body = into_proxy_body_idle(resp.into_body(), IDLE_TIMEOUT);
            Ok(response.body(resp_body).unwrap())
        }
        Err(e) => {
            tracing::warn!("Upstream connect failed for {host}: {e}");
            logger::error(&state.db, &format!("PROXY: {host}{path} → UPSTREAM FAILED: {e}")).await;
            Ok(Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(bytes_body("Bad Gateway"))
                .unwrap())
        }
    }
}

// ── Hop-by-hop header handling ────────────────────────────────────────────

/// RFC 9110 §7.6.1 — headers that MUST be stripped by proxies.
fn is_hop_by_hop_header(name: &HeaderName) -> bool {
    name == CONNECTION
        || name == PROXY_AUTHENTICATE
        || name == PROXY_AUTHORIZATION
        || name == TE
        || name == TRAILER
        || name == TRANSFER_ENCODING
        || name == UPGRADE
}

/// Check if request is a WebSocket upgrade.
fn is_websocket_upgrade(headers: &HeaderMap) -> bool {
    headers.get(UPGRADE)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("websocket"))
        .unwrap_or(false)
        && headers.get(CONNECTION)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.to_lowercase().contains("upgrade"))
            .unwrap_or(false)
}

/// WebSocket headers to preserve in upstream request.
fn is_websocket_request_header(name: &HeaderName) -> bool {
    name == UPGRADE
        || name == CONNECTION
        || name == SEC_WEBSOCKET_KEY
        || name == SEC_WEBSOCKET_VERSION
        || name == SEC_WEBSOCKET_PROTOCOL
        || name == SEC_WEBSOCKET_EXTENSIONS
}

/// WebSocket headers to preserve in downstream response.
fn is_websocket_response_header(name: &HeaderName) -> bool {
    name == UPGRADE
        || name == CONNECTION
        || name == SEC_WEBSOCKET_ACCEPT
        || name == SEC_WEBSOCKET_PROTOCOL
        || name == SEC_WEBSOCKET_EXTENSIONS
}

fn should_keep_header_request(name: &HeaderName, is_ws: bool) -> bool {
    if is_ws && is_websocket_request_header(name) {
        return true;
    }
    !is_hop_by_hop_header(name)
}

fn should_keep_header_response(name: &HeaderName, is_ws: bool) -> bool {
    if is_ws && is_websocket_response_header(name) {
        return true;
    }
    !is_hop_by_hop_header(name)
}
