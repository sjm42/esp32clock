// apiserver.rs

use std::any::Any;

use askama::Template;
use axum::{
    Json,
    body::Body,
    extract::{Form, State},
    http::{Response, StatusCode, header},
    response::{Html, IntoResponse},
    routing::*,
};
pub use axum_macros::debug_handler;
use embedded_svc::http::client::Client as HttpClient;
use esp_idf_svc::{http::client::EspHttpConnection, io, ota::EspOta};
use serde_json::json;

pub use crate::*;

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

pub async fn get_favicon(State(state): State<Arc<Pin<Box<MyState>>>>) -> Response<Body> {
    let cnt = state.api_cnt.fetch_add(1, Ordering::Relaxed);
    info!("#{cnt} get_favicon()");

    let favicon = include_bytes!("favicon.ico");
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "image/vnd.microsoft.icon")],
        favicon.to_vec(),
    )
        .into_response()
}

pub async fn get_formjs(State(state): State<Arc<Pin<Box<MyState>>>>) -> Response<Body> {
    let cnt = state.api_cnt.fetch_add(1, Ordering::Relaxed);
    info!("#{cnt} get_formjs()");

    let formjs = include_bytes!("form.js");
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/javascript")],
        formjs.to_vec(),
    )
        .into_response()
}

pub async fn get_indexcss(State(state): State<Arc<Pin<Box<MyState>>>>) -> Response<Body> {
    let cnt = state.api_cnt.fetch_add(1, Ordering::Relaxed);
    info!("#{cnt} get_indexcss()");

    let indexcss = include_bytes!("index.css");
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        indexcss.to_vec(),
    )
        .into_response()
}

pub async fn send_msg(State(state): State<Arc<Pin<Box<MyState>>>>, Json(message): Json<MyMessage>) -> Response<Body> {
    let cnt = state.api_cnt.fetch_add(1, Ordering::Relaxed);
    info!("#{cnt} send_msg()");

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
    Json(mut config): Json<MyConfig>,
) -> Response<Body> {
    let cnt = state.api_cnt.fetch_add(1, Ordering::Relaxed);
    info!("#{cnt} set_conf()");

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
            info!("Config saved to nvs. Resetting soon...");
            *state.reset.write().await = true;
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
    Form(fw_update): Form<UpdateFirmware>,
) -> Response<Body> {
    let cnt = state.api_cnt.fetch_add(1, Ordering::Relaxed);
    info!("#{cnt} update_fw()");

    info!("Firmware update: \n{fw_update:#?}");
    let url = fw_update.url.trim().to_owned();
    if url.is_empty() {
        return json_error(StatusCode::BAD_REQUEST, "firmware url cannot be empty");
    }

    let mut ota = match EspOta::new() {
        Ok(ota) => ota,
        Err(e) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, format!("OTA init failed: {e:?}")),
    };

    let conn = match EspHttpConnection::new(&Default::default()) {
        Ok(conn) => conn,
        Err(e) => {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("HTTP client init failed: {e:?}"),
            );
        }
    };
    let mut client = HttpClient::wrap(conn);

    let req = match client.get(&url) {
        Ok(req) => req,
        Err(e) => return json_error(StatusCode::BAD_REQUEST, format!("invalid firmware url: {e:?}")),
    };

    let resp = match req.submit() {
        Ok(resp) => resp,
        Err(e) => return json_error(StatusCode::BAD_GATEWAY, format!("firmware download failed: {e:?}")),
    };

    let status = StatusCode::from_u16(resp.status()).unwrap_or(StatusCode::BAD_GATEWAY);
    if status != StatusCode::OK {
        return json_error(
            StatusCode::BAD_GATEWAY,
            format!("firmware download failed with status {}", resp.status()),
        );
    }

    let update_src = Box::new(resp);
    let mut update = match ota.initiate_update() {
        Ok(update) => update,
        Err(e) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, format!("OTA start failed: {e:?}")),
    };
    let mut buffer = [0_u8; 8192];

    if let Err(e) = io::utils::copy(update_src, &mut update, &mut buffer) {
        return json_error(StatusCode::BAD_GATEWAY, format!("OTA write failed: {e:?}"));
    }
    if let Err(e) = update.complete() {
        return json_error(StatusCode::INTERNAL_SERVER_ERROR, format!("OTA finalize failed: {e:?}"));
    }

    info!("Update done. Restarting...");
    task::spawn(async {
        sleep(Duration::from_millis(500)).await;
        esp_idf_svc::hal::reset::restart();
    });
    json_ok("firmware updated, restarting")
}

fn json_ok(message: &str) -> Response<Body> {
    (StatusCode::OK, Json(json!({ "status": "ok", "message": message }))).into_response()
}

fn json_error(status: StatusCode, message: impl Into<String>) -> Response<Body> {
    (status, Json(json!({ "status": "error", "message": message.into() }))).into_response()
}

// EOF
