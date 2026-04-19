use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use rumqttc::{AsyncClient, ClientError, QoS};
use serde_json::json;

use crate::message_types::{ButtonEvent, HueButtonMessage, HueDimmerButton, HueDimmerMessage, HueTapDialButton, HueTapDialMessage, HueTapDialRotationDirection, LightMessage, LightStateEnum};
use crate::config::{ButtonConfig, Config, DimmerConfig, SceneId, TapDialConfig, TargetConfig};


pub struct LightManager {
    client: AsyncClient,
    config: Rc<Config>,
    light_states: HashMap<String, LightState>,
    initial_scene_idx: usize,
}

// internal structs
#[derive(Debug, Copy, Clone, PartialEq)]
struct SceneActivation{
    id: SceneId,
    timestamp: Instant,
}

#[derive(Debug, PartialEq)]
struct LightState {
    zigbee_topic: String,
    is_on: bool,

    last_scene: Option<SceneActivation>,
}

impl LightManager {
    pub fn new(client: AsyncClient, config: Rc<Config>) -> LightManager {
        LightManager {
            client,
            config,
            light_states: HashMap::new(),
            initial_scene_idx: 1,
        }
    }

    fn update_light_state(&mut self, topic: &str, is_on: bool) {
        self.light_states.entry(topic.to_string())
            .and_modify(|state| {
                state.is_on = is_on;
            })
            .or_insert_with(|| LightState {
                zigbee_topic: topic.to_string(),
                is_on,
                last_scene: None,
            });
    }

    pub fn handle_light_message(&mut self, topic: &str, msg: LightMessage) {
        // println!("{topic}: {msg:?}");
        self.update_light_state(topic, msg.state == LightStateEnum::On);
    }

    fn topic_for_target(&self, target: &TargetConfig) -> String {
        format!("{}/{}", self.config.mqtt.zigbee2mqtt_topic_prefix, target.lights)
    }

    async fn publish_for_light_name(&self, light_name: &str, payload: &str) -> Result<(), ClientError>{
        let topic = format!("{}/{}", self.config.mqtt.zigbee2mqtt_topic_prefix, light_name);
        self.client.publish(topic + "/set", QoS::AtLeastOnce, false, payload).await?;
        Ok(())
    }

    async fn publish_for_target(&self, target: &TargetConfig, payload: &str) -> Result<(), ClientError>{
        Ok(self.publish_for_light_name(&target.lights, payload).await?)
    }


    async fn activate_scene(&mut self, target: &TargetConfig, scene_id: SceneId) -> Result<(), ClientError> {
        // println!("{}: activate {scene_id:?}", target.lights);
        self.publish_for_target(target, &json!({"scene_recall": scene_id}).to_string()).await?;
        let topic = self.topic_for_target(target);
        let activation = SceneActivation{id: scene_id, timestamp: Instant::now()};
        self.light_states.entry(topic.clone())
            .and_modify(|state| {
                state.is_on = true;
                state.last_scene = Some(activation)
            })
            .or_insert_with(|| LightState {
                zigbee_topic: topic,
                is_on: true,
                last_scene: Some(activation),
            });
        Ok(())
    }

    fn default_scene(&self, target: &TargetConfig) -> SceneId {
        target.scenes.get(self.initial_scene_idx)
            .cloned()
            .unwrap_or_else(|| target.scenes[0])
    }

    async fn cycle_scene(&mut self, target: &TargetConfig) -> Result<(), ClientError> {
        let topic = format!("{}/{}", self.config.mqtt.zigbee2mqtt_topic_prefix, target.lights);
        let is_on = self.light_states.get(&topic).map_or(false, |s| {s.is_on});
        let last_scene = self.light_states.get(&topic).map(|s| s.last_scene).flatten();

        if !is_on {
            self.activate_scene(target, self.default_scene(target)).await?;
            return Ok(());
        }

        if let Some(last_scene) = last_scene {
            if Instant::now() - last_scene.timestamp < Duration::from_millis(1500) {
                let last_idx = target.scenes.iter().position(|scene_id| *scene_id == last_scene.id);
                let next_idx = last_idx.map(|idx| (idx + 1) % target.scenes.len());
                let scene = next_idx
                    .map(|idx| target.scenes.get(idx).cloned())
                    .flatten()
                    .unwrap_or_else(|| self.default_scene(target));

                self.activate_scene(target, scene).await?;
                return Ok(());
            }
        }

        self.turn_off(target).await?;
        Ok(())
    }

    async fn turn_off(&mut self, target: &TargetConfig) -> Result<(), ClientError> {
        self.publish_for_target(target, &json!({"state": "OFF"}).to_string()).await?;
        self.update_light_state(&self.topic_for_target(&target), false);
        Ok(())
    }

    pub async fn set_initial_scene_idx(&mut self, idx: usize) {
        println!("setting inital scene index to {idx}");
        self.initial_scene_idx = idx;
    }

    pub async fn handle_hue_button_message(&mut self, config: &ButtonConfig, msg: HueButtonMessage) -> Result<(), ClientError> {
        println!("{}: {msg:?}", config.name);
        Ok(())
    }

    pub async fn handle_hue_dimmer_message(&mut self, config: &DimmerConfig, msg: HueDimmerMessage) -> Result<(), ClientError> {
        println!("{}: {msg:?}", config.name);

        match (msg.action.button, msg.action.event) {
            (HueDimmerButton::On, ButtonEvent::Press) => {
                self.cycle_scene(&config.main_target).await?;
            },
            (HueDimmerButton::Off, ButtonEvent::Press) => {
                if let Some(target) = config.secondary_target.as_ref() {
                    self.cycle_scene(target).await?;
                } else {
                    self.turn_off(&config.main_target).await?;
                }
            },
            (HueDimmerButton::Up, ButtonEvent::Press) => {
                let target_light = config.dimmer_lights.as_ref().unwrap_or(&config.main_target.lights);
                self.publish_for_light_name(target_light, &json!({"brightness_move": 150}).to_string()).await?;
            },
            (HueDimmerButton::Down, ButtonEvent::Press) => {
                let target_light = config.dimmer_lights.as_ref().unwrap_or(&config.main_target.lights);
                self.publish_for_light_name(target_light, &json!({"brightness_move": -150}).to_string()).await?;
            },
            (HueDimmerButton::Up|HueDimmerButton::Down, ButtonEvent::Release|ButtonEvent::PressRelease|ButtonEvent::HoldRelease) => {
                let target_light = config.dimmer_lights.as_ref().unwrap_or(&config.main_target.lights);
                self.publish_for_light_name(target_light, &json!({"brightness_move": 0}).to_string()).await?;
            },
            _ => {},
        }
        Ok(())
    }

    pub async fn handle_hue_tap_dial_message(&mut self, config: &TapDialConfig, msg: HueTapDialMessage) -> Result<(), ClientError>{
        match msg {
            HueTapDialMessage::ButtonMessage{action} => {
                println!("{}: {action:?}", config.name);
                let target = match action.button{
                    HueTapDialButton::Button1 => &config.target_1,
                    HueTapDialButton::Button2 => &config.target_2,
                    HueTapDialButton::Button3 => &config.target_3,
                    HueTapDialButton::Button4 => &config.target_4,
                };

                if action.event == ButtonEvent::Press {
                    self.cycle_scene(&target).await?;
                }
                Ok(())
            }
            HueTapDialMessage::RotateMessage{action_direction, action_type: _, action_time} => {
                // println!("{}: {:?}", config.name, rotate_msg);
                let step = match action_direction {
                    HueTapDialRotationDirection::Left => -(action_time as i32),
                    HueTapDialRotationDirection::Right => action_time as i32,
                };
                let transition_time = 0.4;
                self.publish_for_light_name(&config.dimmer_lights, &json!({"brightness_step": step, "transition": transition_time}).to_string()).await?;
                Ok(())
            }
        }
    }
}
