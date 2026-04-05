use std::rc::Rc;

use rumqttc::{AsyncClient, ClientError, LastWill, QoS};
use serde_json::json;

use crate::config::Config;


pub async fn publish_discovery_msg(client: &AsyncClient, config: &Config) -> Result<(), ClientError> {
    let msg = json!({
        "name": None::<String>,
        "device_class": "connectivity",
        "state_topic": config.is_online_topic(),
        "unique_id": "org.nageler.oxilight",
        "device": {
            "identifiers": vec!["org-nageler-oxilight-online"],
            "name": "Oxilight",
        }
    });

    let topic = config.discovery_topic();

    client.publish(topic, rumqttc::QoS::AtLeastOnce, true, msg.to_string()).await?;
    Ok(())
}

pub fn get_last_will(config: &Config) -> LastWill {
    LastWill{
        topic: config.is_online_topic(),
        message: "OFF".into(),
        qos: QoS::AtLeastOnce,
        retain: true,
    }
}

pub async fn publish_online_msg(client: &AsyncClient, config: Rc<Config>) -> Result<(), ClientError> {
    let topic = config.is_online_topic();
    client.publish(topic, rumqttc::QoS::AtLeastOnce, true, "ON").await?;

    Ok(())
}
