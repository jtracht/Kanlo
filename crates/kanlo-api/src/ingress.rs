use super::*;
use axum::{
    extract::{ConnectInfo, Request},
    http::Method,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    net::SocketAddr,
    time::{Duration, Instant},
};

#[derive(Clone)]
pub enum Policy {
    Local {
        hosts: BTreeSet<String>,
        require_private_peer: bool,
    },
    External {
        origin: String,
        authority: String,
    },
}
impl Policy {
    pub fn local() -> Self {
        Self::Local {
            hosts: BTreeSet::new(),
            require_private_peer: false,
        }
    }
    pub fn lan(hosts: BTreeSet<String>) -> Self {
        Self::Local {
            hosts,
            require_private_peer: true,
        }
    }
    pub fn external(origin: &str) -> Result<Self, Error> {
        let uri: axum::http::Uri = origin
            .parse()
            .map_err(|_| Error::Validation("Invalid public origin".into()))?;
        if uri.scheme_str() != Some("https")
            || uri.authority().is_none()
            || uri.path() != "/"
            || uri.query().is_some()
            || origin.ends_with('/')
        {
            return Err(Error::Validation(
                "Public origin must be https://host[:port] without a trailing slash".into(),
            ));
        }
        let authority = uri.authority().ok_or(Error::Storage)?.as_str().to_string();
        if authority.contains('@') {
            return Err(Error::Validation(
                "Origin must not contain credentials".into(),
            ));
        }
        Ok(Self::External {
            origin: origin.into(),
            authority,
        })
    }
}
#[derive(Clone)]
pub struct GuardState {
    db: Database,
    policy: Policy,
    limits: Arc<Mutex<BTreeMap<String, (Instant, u32)>>>,
}
impl GuardState {
    pub fn new(db: Database, policy: Policy) -> Self {
        Self {
            db,
            policy,
            limits: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }
}
#[derive(Clone)]
pub struct Context {
    pub external: bool,
}
pub fn session_token(request: &axum::http::HeaderMap) -> Option<String> {
    request
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|part| part.trim().strip_prefix("__Host-kanlo=").map(String::from))
}
fn local_host(host: &str) -> bool {
    host == "localhost"
        || host == "127.0.0.1"
        || host == "[::1]"
        || ["localhost:", "127.0.0.1:", "[::1]:"]
            .iter()
            .any(|prefix| host.strip_prefix(prefix).is_some_and(super::valid_port))
}
fn private_peer(peer: &SocketAddr) -> bool {
    match peer.ip() {
        std::net::IpAddr::V4(ip) => ip.is_private() || ip.is_loopback() || ip.is_link_local(),
        std::net::IpAddr::V6(ip) => {
            ip.is_loopback() || ip.is_unique_local() || ip.is_unicast_link_local()
        }
    }
}
fn denial(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(json!({"error":{"code":code,"message":message}})),
    )
        .into_response()
}

pub async fn guard(State(state): State<GuardState>, mut request: Request, next: Next) -> Response {
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or_default();
    let write = !matches!(*request.method(), Method::GET | Method::HEAD);
    let path = request.uri().path();
    let login = path == "/api/v1/login";
    let asset = matches!(
        path,
        "/" | "/app.js" | "/style.css" | "/sw.js" | "/manifest.webmanifest" | "/icon.svg"
    );
    let external = matches!(state.policy, Policy::External { .. });
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|p| p.0);
    let limit_key = format!(
        "{}:{}",
        peer.map_or("local".into(), |p| p.ip().to_string()),
        if login { "login" } else { "requests" }
    );
    let limited = {
        let mut limits = match state.limits.lock() {
            Ok(l) => l,
            Err(_) => return ApiError(Error::Storage).into_response(),
        };
        let now = Instant::now();
        limits.retain(|_, (start, _)| now.duration_since(*start) < Duration::from_secs(60));
        if limits.len() > 8192 {
            true
        } else {
            let counter = limits.entry(limit_key).or_insert((now, 0));
            counter.1 += 1;
            counter.1 > if login { 10 } else { 1200 }
        }
    };
    if limited {
        return denial(
            StatusCode::TOO_MANY_REQUESTS,
            "RATE_LIMIT",
            "Please retry later",
        );
    }
    let expected = match &state.policy {
        Policy::Local {
            hosts,
            require_private_peer,
        } => {
            if !(local_host(host) || hosts.contains(host)) {
                return denial(StatusCode::FORBIDDEN, "HOST", "Invalid host");
            }
            if *require_private_peer && !peer.is_some_and(|p| private_peer(&p)) {
                return denial(
                    StatusCode::FORBIDDEN,
                    "INGRESS",
                    "LAN listener requires a private or loopback peer",
                );
            }
            format!("http://{host}")
        }
        Policy::External { origin, authority } => {
            if host != authority {
                return denial(
                    StatusCode::FORBIDDEN,
                    "HOST",
                    "Invalid protected ingress host",
                );
            }
            origin.clone()
        }
    };
    let bearer = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(String::from);
    if let Some(token) = bearer {
        if let Err(error) = database(state.db.clone(), move |db| db.authorize_key(&token)).await {
            return error.into_response();
        }
    } else {
        if write
            && request
                .headers()
                .get(header::ORIGIN)
                .and_then(|h| h.to_str().ok())
                != Some(expected.as_str())
        {
            return denial(
                StatusCode::FORBIDDEN,
                "CSRF",
                "Same-origin Origin header required",
            );
        }
        if external && !asset && !login {
            let Some(token) = session_token(request.headers()) else {
                return ApiError(Error::Unauthorized).into_response();
            };
            let csrf = request
                .headers()
                .get("x-kanlo-csrf")
                .and_then(|h| h.to_str().ok())
                .map(String::from);
            if let Err(error) = database(state.db.clone(), move |db| {
                db.authorize_session(&token, csrf.as_deref(), write)
            })
            .await
            {
                return error.into_response();
            }
        }
    }
    request.extensions_mut().insert(Context { external });
    let mut response = next.run(request).await;
    for (name, value) in [
        (
            "content-security-policy",
            "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' https: http: data:; connect-src 'self'; worker-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'",
        ),
        ("x-content-type-options", "nosniff"),
        ("referrer-policy", "no-referrer"),
        ("cache-control", "no-store"),
    ] {
        response
            .headers_mut()
            .insert(name, HeaderValue::from_static(value));
    }
    response
}
