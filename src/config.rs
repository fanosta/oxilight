use std::{collections::{HashMap, HashSet}, fs, io};
use thiserror::Error;

use serde::{Deserialize, Serialize};


#[derive(Copy, Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct SceneId(u8);

#[derive(Debug, PartialEq, Deserialize)]
pub struct TargetConfig {
    pub lights: String,
    pub scenes: Vec<SceneId>,
    // pub scenes: (SceneId, SceneId, SceneId),
}

#[derive(Debug, PartialEq, Deserialize)]
pub struct ButtonConfig {
    pub name: String,
    pub target: TargetConfig,
}
#[derive(Debug, PartialEq, Deserialize)]
pub struct DimmerConfig {
    pub name: String,
    pub main_target: TargetConfig,
    pub secondary_target: Option<TargetConfig>,
}
#[derive(Debug, PartialEq, Deserialize)]
pub struct TapDialConfig {
    pub name: String,
    pub dimmer_lights: String,
    pub target_1: TargetConfig,
    pub target_2: TargetConfig,
    pub target_3: TargetConfig,
    pub target_4: TargetConfig,
}

fn default_init_scene_topic() -> String {
    "oxilight/initial-scene".to_string()
}

#[derive(Debug, PartialEq, Deserialize)]
pub struct MqttConfig {
  pub username: String,
  pub password: String,
  pub host: String,
  pub port: u16,
  pub use_tls: bool,

  pub client_id: String,
  pub zigbee2mqtt_topic_prefix: String,
  pub home_assistant_discovery_prefix: String,
  pub home_assistant_state_topic: String,

  #[serde(default = "default_init_scene_topic")]
  pub initial_scene_idx_topix: String,
}

#[derive(Debug, PartialEq, Deserialize)]
struct FileConfig {
    pub mqtt: MqttConfig,
    pub buttons: Vec<ButtonConfig>,
    pub dimmers: Vec<DimmerConfig>,
    pub tap_dials: Vec<TapDialConfig>,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(from = "FileConfig")]
pub struct Config {
    pub mqtt: MqttConfig,
    pub all_light_topics: HashSet<String>,
    pub buttons: HashMap<String, ButtonConfig>,
    pub dimmers: HashMap<String, DimmerConfig>,
    pub tap_dials: HashMap<String, TapDialConfig>,
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("could not read config file")]
    ReadError(#[from] io::Error),

    #[error("could not parse config file")]
    ParseError(#[from] yaml_serde::Error),
}


pub fn load_confg() -> Result<Config, ConfigError> {
    let contents = fs::read("/etc/oxilight.yml")?;
    let config: Config = yaml_serde::from_slice(&contents)?;
    Ok(config)
}

impl From<FileConfig> for Config {
    fn from(file_config: FileConfig) -> Config {
        let mut lights = HashSet::new();

        for button in file_config.buttons.iter() {
            lights.insert(button.target.lights.clone());
        }

        for dimmer in file_config.dimmers.iter() {
            lights.insert(dimmer.main_target.lights.clone());
            if let Some(secondary_target) = &dimmer.secondary_target {
                lights.insert(secondary_target.lights.clone());
            }
        }

        for tap_dial in file_config.tap_dials.iter() {
            lights.insert(tap_dial.target_1.lights.clone());
            lights.insert(tap_dial.target_2.lights.clone());
            lights.insert(tap_dial.target_3.lights.clone());
            lights.insert(tap_dial.target_4.lights.clone());
        }

        let prefix = file_config.mqtt.zigbee2mqtt_topic_prefix.clone();
        let lights = lights.into_iter().map(|l| {format!("{prefix}/{l}")}).collect();
        let buttons = file_config.buttons.into_iter().map(|l| {(format!("{prefix}/{}", l.name), l)}).collect();
        let dimmers = file_config.dimmers.into_iter().map(|l| {(format!("{prefix}/{}", l.name), l)}).collect();
        let tap_dials = file_config.tap_dials.into_iter().map(|l| {(format!("{prefix}/{}", l.name), l)}).collect();

        Config{
            mqtt: file_config.mqtt,
            all_light_topics: lights,
            buttons: buttons,
            dimmers: dimmers,
            tap_dials: tap_dials,
        }
    }
}

impl Config {
    const SENSOR_NAME: &str = "oxilight";

    pub fn is_online_topic(&self) -> String {
        return self.mqtt.home_assistant_state_topic.clone();
        // let prefix = &self.mqtt.home_assistant_discovery_prefix;
        // let topic = format!("{prefix}/binary_sensor/{}/state", Config::SENSOR_NAME);
        // topic
    }
    
    pub fn discovery_topic(&self) -> String {
        let prefix = &self.mqtt.home_assistant_discovery_prefix;
        let topic = format!("{prefix}/binary_sensor/{}/config", Config::SENSOR_NAME);
        topic
    }
}
