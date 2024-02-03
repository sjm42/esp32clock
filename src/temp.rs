// temp.rs

use anyhow::bail;
use core::f32;
use embedded_svc::mqtt::client::EventPayload;
use esp_idf_svc::mqtt;
use log::*;
use serde::Deserialize;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

use crate::*;

#[derive(Debug, Deserialize)]
pub struct Temperature {
    temperature: f32,
}

pub async fn run_temp(state: Arc<Pin<Box<MyState>>>) -> anyhow::Result<()> {
    if !state.config.read().await.enable_temp {
        info!("Temp is disabled.");
        // we cannot return, otherwise tokio::select in main() will exit
        loop {
            sleep(Duration::from_secs(3600)).await;
        }
    }

    loop {
        sleep(Duration::from_secs(10)).await;
        {
            let url = &state.config.read().await.mqtt_url;
            let myid = state.myid.read().await.clone();
            info!("MQTT conn: {url} [{myid}]");

            let (client, conn) = match mqtt::client::EspAsyncMqttClient::new(
                url,
                &mqtt::client::MqttClientConfiguration {
                    client_id: Some(&myid),
                    keep_alive_interval: Some(Duration::from_secs(25)),
                    ..Default::default()
                },
            ) {
                Ok(c) => c,
                Err(e) => {
                    error!("MQTT conn failed: {e:?}");
                    continue;
                }
            };

            let _ = tokio::try_join!(
                Box::pin(event_loop(state.clone(), conn)),
                Box::pin(subscribe(state.clone(), client))
            );

            error!("MQTT error, retrying...");
        }
    }
}

async fn event_loop(
    state: Arc<Pin<Box<MyState>>>,
    mut conn: mqtt::client::EspAsyncMqttConnection,
) -> anyhow::Result<()> {
    while let Ok(notification) = Box::pin(conn.next()).await {
        debug!("MQTT recvd: {:?}", notification.payload());

        if let EventPayload::Received {
            id: _,
            topic: _,
            data,
            details: _,
        } = notification.payload()
        {
            match serde_json::from_slice::<Temperature>(data) {
                Err(e) => {
                    error!("JSON error: {e}");
                }
                Ok(t) => {
                    info!("Got temp: {t:?}");
                    *state.temp.write().await = t.temperature;
                }
            }
        }
    }
    bail!("MQTT closed.")
}

async fn subscribe(
    state: Arc<Pin<Box<MyState>>>,
    mut client: mqtt::client::EspAsyncMqttClient,
) -> anyhow::Result<()> {
    info!("MQTT subscribing...");
    if let Err(e) = client
        .subscribe(
            &state.config.read().await.mqtt_topic,
            mqtt::client::QoS::AtLeastOnce,
        )
        .await
    {
        error!("MQTT subscribe error: {e}");
        bail!(e);
    } else {
        info!("MQTT subscribed.");
    }

    loop {
        sleep(Duration::from_secs(60)).await;
    }
}

// EOF
