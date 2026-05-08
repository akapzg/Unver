pub mod auth;
pub mod port_groups;
pub mod proxies;
pub mod settings;

use std::net::SocketAddr;
use std::sync::Arc;
use std::future::Future;
use axum::{
    http::{HeaderValue, Method},
    middleware,
    response::{Html, IntoResponse},
    routing::{delete, get, patch, post},
    Extension, Router,
};
use tower_http::{
    cors::{AllowHeaders, AllowOrigin, CorsLayer},
    set_header::SetResponseHeaderLayer,
};

use crate::{config::Config, middleware::require_auth, state::AppState, state::get_setting};

pub async fn serve(
    state: Arc<AppState>,
    config: &Config,
    shutdown_signal: impl Future<Output = ()> + Send + 'static,
    ssl_worker: Arc<crate::ssl_worker::SslWorkerHandle>,
) -> anyhow::Result<()> {
    let iface = get_setting(&state.db, "web_interface").await.unwrap_or_else(|_| "0.0.0.0".to_string());
    let lan_only = get_setting(&state.db, "panel_lan_only").await.unwrap_or_default() == "true";

    let bind_addr = if lan_only {
        detect_lan_ip().unwrap_or(iface)
    } else if iface.is_empty() || iface == "0.0.0.0" {
        "0.0.0.0".to_string()
    } else {
        iface
    };

    let security_headers = (
        SetResponseHeaderLayer::if_not_present(
            axum::http::header::X_FRAME_OPTIONS,
            HeaderValue::from_static("DENY"),
        ),
        SetResponseHeaderLayer::if_not_present(
            axum::http::header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ),
        SetResponseHeaderLayer::if_not_present(
            axum::http::header::HeaderName::from_static("referrer-policy"),
            HeaderValue::from_static("strict-origin-when-cross-origin"),
        ),
        SetResponseHeaderLayer::if_not_present(
            axum::http::header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(
                "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline' fonts.googleapis.com; font-src fonts.gstatic.com; img-src 'self' data:;",
            ),
        ),
    );

    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::mirror_request())
        .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
        .allow_headers(AllowHeaders::mirror_request())
        .allow_credentials(true);

    let protected = Router::new()
        .route("/settings",           get(settings::get_settings).patch(settings::update_settings))
        .route("/settings/api-keys",  get(settings::list_api_keys).post(settings::create_api_key))
        .route("/settings/api-keys/:id", delete(settings::delete_api_key))
        .route("/port-groups",        get(port_groups::list).post(port_groups::create))
        .route("/port-groups/:id",    patch(port_groups::update).delete(port_groups::delete))
        .route("/proxies",            get(proxies::list).post(proxies::create))
        .route("/proxies/:id",       patch(proxies::update).delete(proxies::delete))
        .route("/system/stats",       get(settings::system_stats))
        .route("/system/network",     get(settings::network_stats))
        .route("/system/logs",        get(settings::list_logs))
        .route("/system/logs/:category", get(settings::list_logs_by_category))
        .route("/system/backup",      get(settings::export_config))
        .route("/system/restore",     post(settings::import_config))
        .route("/system/restart",     post(settings::restart_service))
        .route("/system/renew-ssl",   post(settings::renew_ssl))
        .route("/system/check-update", get(settings::check_update))
        .route("/system/update",      post(settings::perform_update))
        .route("/ddns/status",        get(settings::ddns_status))
        .route("/ddns/toggle/:domain", patch(settings::ddns_toggle))
        .route("/ddns/test",          post(settings::ddns_test))
        .route("/ddns/zones",         get(settings::ddns_list_zones))
        .route("/ddns/domain/:domain", delete(settings::ddns_delete_domain))
        .route("/system/public-ip",   get(settings::public_ip))
        .route("/certificates",       get(settings::list_certificates).post(settings::issue_certificate))
        .route("/certificates/upload", post(settings::upload_certificate))
        .route("/certificates/status/:job_id", get(settings::certificate_status))
        .route("/certificates/:id/download", get(settings::download_certificate))
        .route("/certificates/:id",   patch(settings::update_certificate).delete(settings::delete_certificate))
        .route("/certificates/test",  post(settings::test_certificate_setup))
        .route("/auth/change-password", post(auth::change_password))
        .route_layer(middleware::from_fn_with_state(Arc::clone(&state), require_auth));

    let public_api = Router::new()
        .route("/setup/status", get(auth::setup_status))
        .route("/setup",        post(auth::do_setup))
        .route("/auth/login",   post(auth::login))
        .route("/auth/refresh", post(auth::refresh))
        .route("/auth/logout",  post(auth::logout));

    let api_routes = Router::new()
        .merge(public_api)
        .merge(protected);

    let api = Router::new()
        .nest("/api", api_routes)
        .layer(Extension(Arc::clone(&ssl_worker)))
        .with_state(Arc::clone(&state));

    // Static files + SPA fallback
    let static_dir = config.static_dir.clone();
    let spa_service = tower::service_fn(move |req: axum::http::Request<axum::body::Body>| {
        let dir = static_dir.clone();
        async move {
            let path = req.uri().path().trim_start_matches('/');
            let file_path = dir.join(if path.is_empty() { "index.html" } else { path });

            let result = if file_path.exists() && file_path.is_file() {
                match tokio::fs::read(&file_path).await {
                    Ok(data) => {
                        let ct = match file_path.extension().and_then(|e| e.to_str()) {
                            Some("html") => "text/html",
                            Some("css")  => "text/css",
                            Some("js")   => "application/javascript",
                            Some("json") => "application/json",
                            Some("png")  => "image/png",
                            Some("svg")  => "image/svg+xml",
                            Some("ico")  => "image/x-icon",
                            _ => "application/octet-stream",
                        };
                        let cache = if file_path.file_name().and_then(|n| n.to_str()) == Some("index.html") {
                            "no-cache"
                        } else {
                            "public, max-age=31536000, immutable"
                        };
                        let mut resp = ([(axum::http::header::CONTENT_TYPE, ct)], data).into_response();
                        resp.headers_mut().insert(
                            axum::http::header::CACHE_CONTROL,
                            axum::http::HeaderValue::from_static(cache),
                        );
                        resp
                    }
                    Err(_) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "read error").into_response(),
                }
            } else {
                match tokio::fs::read_to_string(dir.join("index.html")).await {
                    Ok(html) => (axum::http::StatusCode::OK, Html(html)).into_response(),
                    Err(_) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "index.html not found").into_response(),
                }
            };
            Ok::<_, std::convert::Infallible>(result)
        }
    });

    let app = api
        .fallback_service(spa_service)
        .layer(cors)
        .layer(security_headers);

    let addr = format!("{}:{}", bind_addr, config.web_port);
    tracing::info!("Web interface listening on http://{}:{}", bind_addr, config.web_port);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(shutdown_signal)
        .await?;
    Ok(())
}

fn detect_lan_ip() -> Option<String> {
    if let Ok(output) = std::process::Command::new("ip")
        .args(["-4", "-o", "addr", "show", "scope", "global"])
        .output()
    {
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            if line.contains("inet ") && !line.contains("docker") && !line.contains("lo") {
                if let Some(ip) = line.split_whitespace()
                    .find(|w| w.contains('.') && w.chars().all(|c| c.is_ascii_digit() || c == '.'))
                {
                    let ip = ip.trim();
                    if !ip.starts_with("127.") {
                        return Some(ip.to_string());
                    }
                }
            }
        }
    }
    None
}
