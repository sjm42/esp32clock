// apiserver.rs

use std::any::Any;

use anyhow::Context;
use askama::Template;
use axum::{
    Json,
    body::Body,
    extract::{
        DefaultBodyLimit, Form, State,
        rejection::{FormRejection, JsonRejection},
    },
    http::{Response, StatusCode, header},
    response::{Html, IntoResponse},
    routing::*,
};
pub use axum_macros::debug_handler;
use embedded_svc::http::client::Client as HttpClient;
use esp_idf_svc::{http::client::EspHttpConnection, io, ota::EspOta};
use serde_json::json;

pub use crate::*;

const HTTP_BODY_LIMIT: usize = 8 * 1024;
const REBOOT_DELAY: Duration = Duration::from_secs(2);

type ApiResult<T> = Result<T, ApiError>;

#[derive(Debug, thiserror::Error)]
enum ApiError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    BadGateway(String),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self::BadRequest(message.into())
    }

    fn bad_gateway(message: impl Into<String>) -> Self {
        Self::BadGateway(message.into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response<Body> {
        error!("HTTP handler failed: {self:#}");
        let (status, message) = match self {
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
            Self::BadGateway(message) => (StatusCode::BAD_GATEWAY, message),
            Self::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string()),
        };
        json_error(status, message)
    }
}

fn schedule_reboot(state: Arc<Pin<Box<MyState>>>) {
    tokio::spawn(async move {
        sleep(REBOOT_DELAY).await;
        *state.reset.write().await = true;
    });
}

macro_rules! static_handler {
    ($fn_name:ident, $path:literal, $content_type:literal, $bytes:expr) => {
        pub async fn $fn_name(State(state): State<Arc<Pin<Box<MyState>>>>) -> Response<Body> {
            let cnt = state.api_cnt.fetch_add(1, Ordering::Relaxed);
            info!("#{cnt} {}", $path);

            (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, $content_type),
                    (header::CONTENT_ENCODING, "gzip"),
                ],
                $bytes.to_vec(),
            )
                .into_response()
        }
    };
}

static_handler!(
    get_favicon,
    "/favicon.ico",
    "image/vnd.microsoft.icon",
    include_bytes!(concat!(env!("OUT_DIR"), "/favicon.ico.gz"))
);
static_handler!(
    get_formjs,
    "/form.js",
    "text/javascript",
    include_bytes!(concat!(env!("OUT_DIR"), "/form.js.gz"))
);
static_handler!(
    get_indexcss,
    "/index.css",
    "text/css; charset=utf-8",
    include_bytes!(concat!(env!("OUT_DIR"), "/index.css.gz"))
);

pub async fn run_api_server(state: Arc<Pin<Box<MyState>>>) -> anyhow::Result<()> {
    loop {
        if *state.wifi_up.read().await {
            break;
        }
        sleep(Duration::from_secs(1)).await;
    }

    let listen = format!("0.0.0.0:{}", state.config.port);
    let addr = listen.parse::<SocketAddr>()?;

    let app = Router::new()
        .route("/", get(get_index))
        .route("/favicon.ico", get(get_favicon))
        .route("/form.js", get(get_formjs))
        .route("/index.css", get(get_indexcss))
        .route("/msg", post(send_msg).options(options))
        .route("/uptime", get(get_uptime))
        .route("/tz", get(list_timezones))
        .route("/config", get(get_config).post(set_config).options(options))
        .route("/reset_config", get(reset_config))
        .route("/fw", post(update_fw).options(options))
        .layer(DefaultBodyLimit::max(HTTP_BODY_LIMIT))
        .with_state(state);
    // .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("API server listening to {listen}");
    Ok(axum::serve(listener, app.into_make_service()).await?)
}

pub async fn options(State(state): State<Arc<Pin<Box<MyState>>>>) -> Response<Body> {
    let cnt = state.api_cnt.fetch_add(1, Ordering::Relaxed);
    info!("#{cnt} options()");

    (
        StatusCode::OK,
        [
            (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*"),
            (header::ACCESS_CONTROL_ALLOW_METHODS, "get,post"),
            (header::ACCESS_CONTROL_ALLOW_HEADERS, "content-type"),
        ],
        Json(json!({ "status": "ok" })),
    )
        .into_response()
}

pub async fn get_index(State(state): State<Arc<Pin<Box<MyState>>>>) -> Response<Body> {
    let cnt = state.api_cnt.fetch_add(1, Ordering::Relaxed);
    info!("#{cnt} get_index()");

    let value_tuple: (&str, &dyn Any) = ("ota_slot", &state.ota_slot.clone());
    let index = match state.config.clone().render_with_values(&value_tuple) {
        Err(e) => {
            let err_msg = format!("Index template error: {e:?}\n");
            error!("{err_msg}");
            return (StatusCode::INTERNAL_SERVER_ERROR, err_msg).into_response();
        }
        Ok(s) => s,
    };
    (StatusCode::OK, Html(index)).into_response()
}

pub async fn send_msg(
    State(state): State<Arc<Pin<Box<MyState>>>>,
    message: Result<Json<MyMessage>, JsonRejection>,
) -> Response<Body> {
    let cnt = state.api_cnt.fetch_add(1, Ordering::Relaxed);
    info!("#{cnt} send_msg()");
    let Json(message) = match message {
        Ok(message) => message,
        Err(error) => return json_error(error.status(), format!("Invalid message JSON: {error}")),
    };

    let msg = message.msg;
    info!("Got msg: {msg}");
    *state.msg.write().await = Some(msg);
    json_ok("message accepted")
}

pub async fn get_uptime(State(state): State<Arc<Pin<Box<MyState>>>>) -> Response<Body> {
    let cnt = state.api_cnt.fetch_add(1, Ordering::Relaxed);
    info!("#{cnt} get_uptime()");

    let uptime = state.boot_instant.elapsed().as_secs();
    (StatusCode::OK, Json(json!({ "uptime": uptime }))).into_response()
}

pub async fn list_timezones(State(state): State<Arc<Pin<Box<MyState>>>>) -> Response<Body> {
    let cnt = state.api_cnt.fetch_add(1, Ordering::Relaxed);
    info!("#{cnt} list_timezones()");

    let timezones: Vec<String> = TZ_VARIANTS.iter().map(|tz| tz.to_string()).collect();
    (StatusCode::OK, Json(json!({ "timezones": timezones }))).into_response()
}

pub async fn get_config(State(state): State<Arc<Pin<Box<MyState>>>>) -> Response<Body> {
    let cnt = state.api_cnt.fetch_add(1, Ordering::Relaxed);
    info!("#{cnt} get_conf()");
    (StatusCode::OK, Json(state.config.clone())).into_response()
}

pub async fn set_config(
    State(state): State<Arc<Pin<Box<MyState>>>>,
    config: Result<Json<MyConfig>, JsonRejection>,
) -> Response<Body> {
    let cnt = state.api_cnt.fetch_add(1, Ordering::Relaxed);
    info!("#{cnt} set_conf()");
    let Json(mut config) = match config {
        Ok(config) => config,
        Err(error) => return json_error(error.status(), format!("Invalid config JSON: {error}")),
    };

    if config.v4mask > 30 {
        let emsg = "IPv4 mask error: bits must be between 0..30\n";
        error!("{emsg}");
        return json_error(StatusCode::BAD_REQUEST, emsg);
    }

    if !config.wifi_wpa2ent {
        // No username without WPA2 Enterprise auth
        config.wifi_username.clear();
    }

    if config.v4dhcp {
        // clear out these if we are using DHCP
        config.v4addr = net::Ipv4Addr::new(0, 0, 0, 0);
        config.v4mask = 0;
        config.v4gw = net::Ipv4Addr::new(0, 0, 0, 0);
        config.dns1 = net::Ipv4Addr::new(0, 0, 0, 0);
        config.dns2 = net::Ipv4Addr::new(0, 0, 0, 0);
    }

    let tz_s = &config.tz;
    if let Err(e) = tz_s.parse::<Tz>() {
        let emsg = format!("Cannot parse timezone {tz_s:?}: {e:?}\n");
        error!("{emsg}");
        return json_error(StatusCode::BAD_REQUEST, emsg);
    }

    if !config.lat.is_finite() || !(-90.0..=90.0).contains(&config.lat) {
        let emsg = format!("Latitude out of range: {} (expected -90..90)", config.lat);
        error!("{emsg}");
        return json_error(StatusCode::BAD_REQUEST, emsg);
    }
    if !config.lon.is_finite() || !(-180.0..=180.0).contains(&config.lon) {
        let emsg = format!("Longitude out of range: {} (expected -180..180)", config.lon);
        error!("{emsg}");
        return json_error(StatusCode::BAD_REQUEST, emsg);
    }

    // MAX7219 has 4 bits for intensity, i.e. values 0-15
    config.led_intensity_night = config.led_intensity_night.min(15);
    config.led_intensity_day = config.led_intensity_day.min(15);

    info!("Saving new config to nvs...");
    Box::pin(save_config(state, config)).await
}

pub async fn reset_config(State(state): State<Arc<Pin<Box<MyState>>>>) -> Response<Body> {
    let cnt = state.api_cnt.fetch_add(1, Ordering::Relaxed);
    info!("#{cnt} reset_conf()");

    info!("Saving  default config to nvs...");
    Box::pin(save_config(state, MyConfig::default())).await
}

async fn save_config(state: Arc<Pin<Box<MyState>>>, config: MyConfig) -> Response<Body> {
    let mut nvs = state.nvs.write().await;
    match config.to_nvs(&mut nvs) {
        Ok(_) => {
            drop(nvs);
            info!("Config saved to nvs. Resetting soon...");
            schedule_reboot(state);
            json_ok("config saved, restarting")
        }
        Err(e) => {
            let emsg = format!("Nvs write error: {e:?}\n");
            error!("{emsg}");
            json_error(StatusCode::INTERNAL_SERVER_ERROR, emsg)
        }
    }
}

async fn update_fw(
    State(state): State<Arc<Pin<Box<MyState>>>>,
    fw_update: Result<Form<UpdateFirmware>, FormRejection>,
) -> ApiResult<Response<Body>> {
    let cnt = state.api_cnt.fetch_add(1, Ordering::Relaxed);
    info!("#{cnt} update_fw()");

    let Form(fw_update) = match fw_update {
        Ok(form) => form,
        Err(error) => return Ok(json_error(error.status(), format!("Invalid firmware request: {error}"))),
    };
    info!("Firmware update: \n{fw_update:#?}");
    let url = fw_update.url.trim();
    if !url.starts_with("http://") {
        return Err(ApiError::bad_request("Firmware URL must start with http://"));
    }

    let mut ota = EspOta::new().context("could not acquire OTA service")?;
    let connection =
        EspHttpConnection::new(&Default::default()).context("could not create firmware HTTP connection")?;
    let mut client = HttpClient::wrap(connection);
    let req = client
        .get(url)
        .map_err(|error| ApiError::bad_request(format!("Invalid firmware URL: {error}")))?;
    let resp = req
        .submit()
        .map_err(|error| ApiError::bad_gateway(format!("Firmware download failed: {error}")))?;
    if resp.status() != 200 {
        return Err(ApiError::bad_gateway(format!(
            "Firmware download failed: HTTP {}",
            resp.status()
        )));
    }

    let mut update = ota.initiate_update().context("could not initiate OTA update")?;
    let mut buffer = Vec::new();
    buffer
        .try_reserve_exact(8192)
        .context("could not allocate OTA transfer buffer")?;
    buffer.resize(8192, 0_u8);

    io::utils::copy(resp, &mut update, &mut buffer).map_err(|error| match error {
        io::utils::CopyError::Read(error) => {
            ApiError::bad_gateway(format!("Firmware download failed while reading: {error}"))
        }
        io::utils::CopyError::Write(error) => {
            ApiError::Internal(anyhow::anyhow!("firmware flash write failed: {error}"))
        }
    })?;
    update.complete().context("firmware validation or activation failed")?;

    info!("Update done. Restarting soon...");
    schedule_reboot(state);
    Ok(json_ok("firmware updated, restarting"))
}

fn json_ok(message: &str) -> Response<Body> {
    (StatusCode::OK, Json(json!({ "status": "ok", "message": message }))).into_response()
}

fn json_error(status: StatusCode, message: impl Into<String>) -> Response<Body> {
    (status, Json(json!({ "status": "error", "message": message.into() }))).into_response()
}

// EOF
