// apiserver.rs

use askama::Template;
pub use axum_macros::debug_handler;

use axum::{
    body::Body,
    extract::State,
    http::{header, Response, StatusCode},
    response::{Html, IntoResponse},
    routing::*,
    Json,
};
use chrono_tz::{Tz, TZ_VARIANTS};
use std::{net, net::SocketAddr};
use tokio::time::{sleep, Duration};
// use tower_http::trace::TraceLayer;

pub use crate::*;

pub async fn run_api_server(state: Arc<Pin<Box<MyState>>>) -> anyhow::Result<()> {
    loop {
        if *state.wifi_up.read().await {
            break;
        }
        sleep(Duration::from_secs(1)).await;
    }

    let listen = format!("0.0.0.0:{}", state.config.read().await.port);
    let addr = listen.parse::<SocketAddr>()?;

    let app = Router::new()
        .route("/", get(get_index))
        .route("/favicon.ico", get(get_favicon))
        .route("/conf", get(get_conf).post(set_conf).options(options))
        .route("/tz", get(list_timezones))
        .route("/msg", post(send_msg))
        .route("/reset_conf", get(reset_conf))
        .with_state(state);
    // .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("API server listening to {listen}");
    Ok(axum::serve(listener, app.into_make_service()).await?)
}

pub async fn get_index(State(state): State<Arc<Pin<Box<MyState>>>>) -> Response<Body> {
    {
        let mut c = state.cnt.write().await;
        *c += 1;
        info!("#{c} get_index()");
    }

    let index = match state.config.read().await.clone().render() {
        Err(e) => {
            let emsg = format!("Index template error: {e:?}\n");
            error!("{emsg}");
            return (StatusCode::INTERNAL_SERVER_ERROR, emsg).into_response();
        }
        Ok(s) => s,
    };
    (StatusCode::OK, Html(index)).into_response()
}

pub async fn get_favicon(State(state): State<Arc<Pin<Box<MyState>>>>) -> Response<Body> {
    {
        let mut c = state.cnt.write().await;
        *c += 1;
        info!("#{c} get_favicon()");
    }
    let favicon = include_bytes!("favicon.ico");
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "image/vnd.microsoft.icon")],
        favicon.to_vec(),
    )
        .into_response()
}

pub async fn get_conf(State(state): State<Arc<Pin<Box<MyState>>>>) -> Response<Body> {
    {
        let mut c = state.cnt.write().await;
        *c += 1;
        info!("#{c} get_conf()");
    }
    (StatusCode::OK, Json(state.config.read().await.clone())).into_response()
}

pub async fn options(State(state): State<Arc<Pin<Box<MyState>>>>) -> Response<Body> {
    {
        let mut c = state.cnt.write().await;
        *c += 1;
        info!("#{c} options()");
    }
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

pub async fn set_conf(
    State(state): State<Arc<Pin<Box<MyState>>>>,
    Json(mut config): Json<MyConfig>,
) -> Response<Body> {
    {
        let mut c = state.cnt.write().await;
        *c += 1;
        info!("#{c} set_conf()");
    }

    if config.v4mask > 30 {
        let emsg = "IPv4 mask error: bits must be between 0..30\n";
        error!("{emsg}");
        return (StatusCode::INTERNAL_SERVER_ERROR, emsg.to_string()).into_response();
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

    info!("Saving new config to nvs...");
    Box::pin(save_conf(state, config)).await
}

pub async fn reset_conf(State(state): State<Arc<Pin<Box<MyState>>>>) -> Response<Body> {
    {
        let mut c = state.cnt.write().await;
        *c += 1;
        info!("#{c} reset_conf()");
    }
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

pub async fn send_msg(
    State(state): State<Arc<Pin<Box<MyState>>>>,
    Json(message): Json<MyMessage>,
) -> Response<Body> {
    {
        let mut c = state.cnt.write().await;
        *c += 1;
        info!("#{c} send_msg()");
    }

    let msg = message.msg;
    info!("Got msg: {msg}");
    *state.msg.write().await = Some(msg);
    (StatusCode::OK, "OK\n".to_string()).into_response()
}

pub async fn list_timezones(State(state): State<Arc<Pin<Box<MyState>>>>) -> Response<Body> {
    {
        let mut c = state.cnt.write().await;
        *c += 1;
        info!("#{c} send_msg()");
    }

    // yes, it's almost 10 KiB so alloc it already
    let mut tz_s = String::with_capacity(10240);
    for tz in TZ_VARIANTS {
        tz_s.push_str(&format!("{tz}\n"));
    }
    (StatusCode::OK, tz_s).into_response()
}

// EOF
