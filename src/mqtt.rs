// mqtt.rs

use esp_idf_svc::mqtt::{
    self,
    client::{EventPayload, MessageId},
};
use esp_idf_sys::EspError;

use crate::*;

pub async fn run_mqtt(state: Arc<Pin<Box<MyState>>>) -> anyhow::Result<()> {
    if !state.config.mqtt_enable {
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
        sleep(Duration::from_secs(10)).await;
    }

    loop {
        {
            let url = &state.config.mqtt_url;
            let myid = state.myid.read().await.clone();
            info!("MQTT conn: {url} [{myid}]");

            let (client, conn) = match mqtt::client::EspAsyncMqttClient::new(
                url,
                &mqtt::client::MqttClientConfiguration {
                    client_id: Some(&myid),
                    keep_alive_interval: Some(Duration::from_secs(100)),
                    ..Default::default()
                },
            ) {
                Ok(c) => c,
                Err(e) => {
                    error!("MQTT conn failed: {e:?}");
                    continue;
                }
            };
            info!("MQTT connected.");
            sleep(Duration::from_secs(5)).await;

            let _ = tokio::try_join!(
                Box::pin(subscribe_publish(state.clone(), client)),
                Box::pin(event_loop(state.clone(), conn)),
            );

            error!("MQTT error, retrying after 30s...");
            sleep(Duration::from_secs(30)).await;
        }
    }
}

async fn subscribe_publish(
    state: Arc<Pin<Box<MyState>>>,
    mut client: mqtt::client::EspAsyncMqttClient,
) -> anyhow::Result<()> {
    info!("MQTT subscribing...");
    for t in [
        "esp32clock-all",
        &state.myid.read().await,
        &state.config.mqtt_topic,
    ] {
        sleep(Duration::from_secs(1)).await;
        info!("Subscribe: {t}");
        if let Err(e) = client.subscribe(t, mqtt::client::QoS::AtLeastOnce).await {
            error!("MQTT subscribe error: {e}");
            bail!(e);
        }
    }
    info!("MQTT all subscribed.");

    let sensor_topic = state.config.sensor_topic.clone();
    loop {
        sleep(Duration::from_secs(10)).await;

        if state.config.sensor_enable {
            {
                let mut fresh_data = state.meas_updated.write().await;
                if !*fresh_data {
                    continue;
                }
                *fresh_data = false;
            }
            let temp = *state.meas.read().await;
            if temp > -100.0 {
                let mqtt_data = format!("{{ \"temperature\": {temp} }}");
                Box::pin(mqtt_send(&mut client, &sensor_topic, &mqtt_data)).await?;
            }
        }
    }
}

async fn mqtt_send(
    client: &mut mqtt::client::EspAsyncMqttClient,
    topic: &str,
    data: &str,
) -> Result<MessageId, EspError> {
    info!("MQTT sending {topic} {data}");

    let result = client
        .publish(
            topic,
            mqtt::client::QoS::AtLeastOnce,
            false,
            data.as_bytes(),
        )
        .await;
    if let Err(e) = result {
        let msg = format!("MQTT send error: {e}");
        error!("{msg}");
    }
    result
}

async fn event_loop(
    state: Arc<Pin<Box<MyState>>>,
    mut conn: mqtt::client::EspAsyncMqttConnection,
) -> anyhow::Result<()> {
    let temp_topic = &state.config.mqtt_topic;

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
