// apiserver.rs

use askama::Template;
use axum::{
    body::Body,
    extract::State,
    http::{header, Response, StatusCode},
    response::{Html, IntoResponse},
    routing::*,
    Json,
};
pub use axum_macros::debug_handler;

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
        .route("/msg", post(send_msg).options(options))
        .route("/tz", get(list_timezones))
        .route("/conf", get(get_conf).post(set_conf).options(options))
        .route("/reset_conf", get(reset_conf))
        .with_state(state);
    // .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("API server listening to {listen}");
    Ok(axum::serve(listener, app.into_make_service()).await?)
}

pub async fn options(State(state): State<Arc<Pin<Box<MyState>>>>) -> Response<Body> {
    let cnt = {
        let mut c = state.api_cnt.write().await;
        *c += 1;
        *c
    };
    info!("#{cnt} options()");

    (
        StatusCode::OK,
        [
            (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*"),
            (header::ACCESS_CONTROL_ALLOW_METHODS, "get,post"),
            (header::ACCESS_CONTROL_ALLOW_HEADERS, "content-type"),
        ],
    )
        .into_response()
}

pub async fn get_index(State(state): State<Arc<Pin<Box<MyState>>>>) -> Response<Body> {
    let cnt = {
        let mut c = state.api_cnt.write().await;
        *c += 1;
        *c
    };
    info!("#{cnt} get_index()");

    let index = match state.config.clone().render() {
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
    let cnt = {
        let mut c = state.api_cnt.write().await;
        *c += 1;
        *c
    };
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
    let cnt = {
        let mut c = state.api_cnt.write().await;
        *c += 1;
        *c
    };
    info!("#{cnt} get_formjs()");

    let formjs = include_bytes!("form.js");
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/javascript")],
        formjs.to_vec(),
    )
        .into_response()
}

pub async fn send_msg(
    State(state): State<Arc<Pin<Box<MyState>>>>,
    Json(message): Json<MyMessage>,
) -> Response<Body> {
    let cnt = {
        let mut c = state.api_cnt.write().await;
        *c += 1;
        *c
    };
    info!("#{cnt} send_msg()");

    let msg = message.msg;
    info!("Got msg: {msg}");
    *state.msg.write().await = Some(msg);
    (StatusCode::OK, "OK\n".to_string()).into_response()
}

pub async fn list_timezones(State(state): State<Arc<Pin<Box<MyState>>>>) -> Response<Body> {
    let cnt = {
        let mut c = state.api_cnt.write().await;
        *c += 1;
        *c
    };
    info!("#{cnt} send_msg()");

    // yes, it's almost 10 KiB so alloc it already
    let mut tz_s = String::with_capacity(10240);
    for tz in TZ_VARIANTS {
        tz_s.push_str(&format!("{tz}\n"));
    }
    (StatusCode::OK, tz_s).into_response()
}

pub async fn get_conf(State(state): State<Arc<Pin<Box<MyState>>>>) -> Response<Body> {
    let cnt = {
        let mut c = state.api_cnt.write().await;
        *c += 1;
        *c
    };
    info!("#{cnt} get_conf()");

    (StatusCode::OK, Json(state.config.clone())).into_response()
}

pub async fn set_conf(
    State(state): State<Arc<Pin<Box<MyState>>>>,
    Json(mut config): Json<MyConfig>,
) -> Response<Body> {
    let cnt = {
        let mut c = state.api_cnt.write().await;
        *c += 1;
        *c
    };
    info!("#{cnt} set_conf()");

    if config.v4mask > 30 {
        let emsg = "IPv4 mask error: bits must be between 0..30\n";
        error!("{emsg}");
        return (StatusCode::INTERNAL_SERVER_ERROR, emsg.to_string()).into_response();
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
        return (StatusCode::BAD_REQUEST, emsg).into_response();
    }

    // MAX7219 has 4 bits for intensity, i.e. values 0-15
    config.led_intensity_night = config.led_intensity_night.min(15);
    config.led_intensity_day = config.led_intensity_day.min(15);

    info!("Saving new config to nvs...");
    Box::pin(save_conf(state, config)).await
}

pub async fn reset_conf(State(state): State<Arc<Pin<Box<MyState>>>>) -> Response<Body> {
    let cnt = {
        let mut c = state.api_cnt.write().await;
        *c += 1;
        *c
    };
    info!("#{cnt} reset_conf()");

    info!("Saving  default config to nvs...");
    Box::pin(save_conf(state, MyConfig::default())).await
}

async fn save_conf(state: Arc<Pin<Box<MyState>>>, config: MyConfig) -> Response<Body> {
    let mut nvs = state.nvs.write().await;
    match config.to_nvs(&mut nvs) {
        Ok(_) => {
            info!("Config saved to nvs. Resetting soon...");
            *state.reset.write().await = true;
            (StatusCode::OK, "OK\n".to_string()).into_response()
        }
        Err(e) => {
            let emsg = format!("Nvs write error: {e:?}\n");
            error!("{emsg}");
            (StatusCode::INTERNAL_SERVER_ERROR, emsg).into_response()
        }
    }
}
// EOF
