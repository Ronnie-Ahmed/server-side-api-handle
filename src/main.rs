use axum::{
    extract::{ConnectInfo, Json},
    http::StatusCode,
    routing::post,
    Extension, Router,
};
use chrono::{Duration, Utc};
use dashmap::DashMap;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{env, net::SocketAddr, sync::Arc};
use thiserror::Error;
use tower_http::trace::TraceLayer;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Debug, Deserialize, Serialize)]
struct GeoRequest {
    #[serde(rename = "considerIp")]
    consider_ip: bool,

    #[serde(rename = "wifiAccessPoints")]
    wifi_access_points: Vec<WifiAccessPoint>,
}

#[derive(Debug, Deserialize, Serialize)]
struct WifiAccessPoint {
    #[serde(rename = "macAddress")]
    mac_address: String,

    #[serde(rename = "signalStrength")]
    signal_strength: i32,
}

#[derive(Debug, Deserialize)]
struct GoogleGeoResponse {
    location: GoogleLocation,
    accuracy: f64,
}

#[derive(Debug, Deserialize)]
struct GoogleLocation {
    lat: f64,
    lng: f64,
}

#[derive(Debug, Serialize, Clone)]
struct LocationResponse {
    lat: f64,
    lon: f64,
}

#[derive(Debug, Clone)]
struct CacheEntry {
    response: LocationResponse,
    timestamp: chrono::DateTime<Utc>,
}

type RateLimitStore = Arc<DashMap<String, Vec<chrono::DateTime<Utc>>>>;
type CacheStore = Arc<DashMap<String, CacheEntry>>;

#[derive(Clone)]
struct AppConfig {
    cache_ttl: Duration,
    max_requests_per_day: usize,
    google_api_key: String,
}

#[derive(Error, Debug)]
enum GeoError {
    #[error("Rate limit exceeded")]
    RateLimited,
    #[error("Google API error: {0}")]
    GoogleApi(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

impl Into<(StatusCode, String)> for GeoError {
    fn into(self) -> (StatusCode, String) {
        match self {
            GeoError::RateLimited => (StatusCode::TOO_MANY_REQUESTS, self.to_string()),
            GeoError::GoogleApi(e) => (StatusCode::BAD_GATEWAY, e),
            GeoError::Internal(e) => (StatusCode::INTERNAL_SERVER_ERROR, e),
        }
    }
}

async fn handle_geo(
    Extension(config): Extension<AppConfig>,
    Extension(rate_limit_store): Extension<RateLimitStore>,
    Extension(cache_store): Extension<CacheStore>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(payload): Json<GeoRequest>,
) -> Result<Json<LocationResponse>, (StatusCode, String)> {
    let ip = addr.ip().to_string();
    let now = Utc::now();

    //  cache check
    if let Some(entry) = cache_store.get(&ip) {
        if entry.timestamp + config.cache_ttl > now {
            info!(%ip, "cache hit");
            return Ok(Json(entry.response.clone()));
        }
    }

    // rate limiting check
    let mut entry = rate_limit_store.entry(ip.clone()).or_default();
    entry.retain(|t| *t + Duration::days(1) > now);

    if entry.len() >= config.max_requests_per_day {
        error!(%ip, "rate limit exceeded");
        return Err(GeoError::RateLimited.into());
    }

    entry.push(now);

    info!(%ip, ?payload, "calling Google API");

    let url = format!(
        "https://www.googleapis.com/geolocation/v1/geolocate?key={}",
        config.google_api_key
    );

    let client = Client::new();
    let resp = client.post(&url).json(&payload).send().await.map_err(|e| {
        error!(%ip, error = ?e, "request failed");
        GeoError::Internal(e.to_string()).into()
    })?;

    if resp.status().is_success() {
        let geo: GoogleGeoResponse = resp.json().await.map_err(|e| {
            error!(%ip, error = ?e, "json decode failed");
            GeoError::Internal(e.to_string()).into()
        })?;

        info!(%ip, lat = geo.location.lat, lon = geo.location.lng, accuracy = geo.accuracy, "success");

        let response = LocationResponse {
            lat: geo.location.lat,
            lon: geo.location.lng,
        };

        //  update cache
        cache_store.insert(
            ip.clone(),
            CacheEntry {
                response: response.clone(),
                timestamp: now,
            },
        );

        Ok(Json(response))
    } else {
        let status = resp.status();
        error!(%ip, ?status, "Google API error");
        Err(GeoError::GoogleApi(format!("{}", status)).into())
    }
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cache_ttl_hours: i64 = env::var("CACHE_TTL_HOURS")
        .unwrap_or_else(|_| "12".to_string())
        .parse()
        .unwrap_or(12);

    let max_requests_per_day: usize = env::var("MAX_REQUESTS_PER_DAY")
        .unwrap_or_else(|_| "2".to_string())
        .parse()
        .unwrap_or(2);

    let google_api_key =
        env::var("GOOGLE_API_KEY").expect("GOOGLE_API_KEY must be set in .env");

    let config = AppConfig {
        cache_ttl: Duration::hours(cache_ttl_hours),
        max_requests_per_day,
        google_api_key,
    };

    let rate_limit_store: RateLimitStore = Arc::new(DashMap::new());
    let cache_store: CacheStore = Arc::new(DashMap::new());

    let app = Router::new()
        .route("/geo", post(handle_geo))
        .layer(Extension(config))
        .layer(Extension(rate_limit_store))
        .layer(Extension(cache_store))
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .unwrap();

    info!("🚀 Server running at http://{}", listener.local_addr().unwrap());

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}
