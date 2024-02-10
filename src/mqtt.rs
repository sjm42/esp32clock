// mqtt.rs

use anyhow::bail;
use chrono::*;
use embedded_svc::mqtt::client::EventPayload;
use esp_idf_svc::mqtt;
use log::*;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

use crate::*;

pub async fn run_mqtt(state: Arc<Pin<Box<MyState>>>) -> anyhow::Result<()> {
    if !state.config.read().await.enable_mqtt {
        info!("Temp is disabled.");
        // we cannot return, otherwise tokio::select in main() will exit
        loop {
            sleep(Duration::from_secs(3600)).await;
        }
    }

    loop {
        // we wait for WiFi and NTP to settle
        if *state.wifi_up.read().await && Utc::now().year() > 2020 {
            break;
        }
        sleep(Duration::from_secs(1)).await;
    }

    loop {
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
                Box::pin(subscribe(state.clone(), client)),
                Box::pin(event_loop(state.clone(), conn)),
            );

            error!("MQTT error, retrying...");
        }
    }
}

async fn subscribe(
    state: Arc<Pin<Box<MyState>>>,
    mut client: mqtt::client::EspAsyncMqttClient,
) -> anyhow::Result<()> {
    info!("MQTT subscribing...");
    for t in [
        "esp32clock-all",
        &state.myid.read().await,
        &state.config.read().await.temp_topic,
    ] {
        info!("Subscribe: {t}");
        if let Err(e) = client.subscribe(t, mqtt::client::QoS::AtLeastOnce).await {
            error!("MQTT subscribe error: {e}");
            bail!(e);
        }
    }
    info!("Subscribed.");

    // we stay here looping slowly or the program will crash miserably, idk why.
    loop {
        sleep(Duration::from_secs(60)).await;
    }
}

async fn event_loop(
    state: Arc<Pin<Box<MyState>>>,
    mut conn: mqtt::client::EspAsyncMqttConnection,
) -> anyhow::Result<()> {
    let temp_topic = &state.config.read().await.temp_topic;

    while let Ok(notification) = Box::pin(conn.next()).await {
        debug!("MQTT recvd: {:?}", notification.payload());

        if let EventPayload::Received {
            id: _,
            topic: Some(topic),
            data,
            details: _,
        } = notification.payload()
        {
            info!("Rcvd topic: {topic}");

            if topic == temp_topic {
                match serde_json::from_slice::<Temperature>(data) {
                    Err(e) => {
                        error!("Temp JSON error: {e}");
                    }
                    Ok(t) => {
                        info!("Got temp: {t:?}");
                        *state.temp.write().await = t.temperature;
                        *state.temp_t.write().await = Utc::now().timestamp();
                    }
                }
                continue;
            }

            // all other topics are considered as incoming message

            match serde_json::from_slice::<MyMessage>(data) {
                Err(e) => {
                    error!("Msg JSON error: {e}");
                }
                Ok(m) => {
                    info!("Got msg: {m:?}");
                    *state.msg.write().await = Some(m.msg);
                }
            }
        }
    }
    bail!("MQTT closed.")
}

// EOF
