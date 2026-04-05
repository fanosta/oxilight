use std::{error::Error, rc::Rc, str::Utf8Error, sync::atomic::AtomicBool};

use rumqttc::{AsyncClient, ClientError, ConnectionError, Event, EventLoop, Incoming, MqttOptions, Publish, QoS, StateError, SubscribeFilter, Transport};
use tokio::signal::unix::{signal, SignalKind};
use tokio_rustls::rustls::ClientConfig;
use thiserror::Error;

mod config;
mod light_manager;
mod ha_discovery;
mod message_types;
mod systemd;

use message_types::LightMessage;
use ha_discovery::{publish_discovery_msg, publish_online_msg};
use light_manager::LightManager;

use crate::{config::{Config, ConfigError}, message_types::{HueButtonMessage, HueDimmerMessage, HueTapDialMessage}};


struct Oxilight {
    config: Rc<Config>,
    light_manager: LightManager,
    client: AsyncClient,
    event_loop: EventLoop,
}

#[derive(Error, Debug)]
pub enum HandleMessageError {
    #[error("UTF-8 parsing failed")]
    UnicodeParseError(#[from] Utf8Error),

    #[error("Parsing JSON failed")]
    JsonParseError(#[from] serde_json::Error),

    #[error("Sending failed")]
    SendError(#[from] ClientError),
} 

#[derive(Error, Debug)]
pub enum RunError {
    #[error("Connection Error")]
    ConnectionError(#[from] ConnectionError),

    #[error("Client Error")]
    ClientError(#[from] ClientError),

    #[error("Signal Handler Error")]
    SignalHandlerError(#[from] std::io::Error),

    #[error("Systemd Notify Error")]
    SystemdError(#[from] systemd::NotifyError),
}

impl Oxilight {
    pub fn new(config: Config) -> Result<Oxilight, Box<dyn Error>> {
        let config = Rc::new(config);
        let mut mqttoptions = MqttOptions::new(&config.mqtt.client_id, &config.mqtt.host, config.mqtt.port);

        mqttoptions.set_keep_alive(std::time::Duration::from_secs(5));
        mqttoptions.set_credentials(&config.mqtt.username, &config.mqtt.password);
        mqttoptions.set_last_will(ha_discovery::get_last_will(&config));

        let client_config = ClientConfig::builder();
        if config.mqtt.use_tls {
            // Use rustls-native-certs to load root certificates from the operating system.
            let mut root_cert_store = tokio_rustls::rustls::RootCertStore::empty();
            root_cert_store.add_parsable_certificates(
                rustls_native_certs::load_native_certs().expect("could not load platform certs"),
            );


            let client_config = client_config
                .with_root_certificates(root_cert_store)
                .with_no_client_auth();
            mqttoptions.set_transport(Transport::tls_with_config(client_config.into()));
        }

        let (client, event_loop) = AsyncClient::new(mqttoptions, 10);
        Ok(Oxilight{
                config: config.clone(),
                light_manager: LightManager::new(client.clone(), config),
                client,
                event_loop,
        })
   }

    async fn handle_message(&mut self, p: Publish) -> Result<(), HandleMessageError> {
        let topic = p.topic;
        let payload = str::from_utf8(&p.payload)?;
        // println!("{}: {}", topic, payload);

        if topic == self.config.mqtt.initial_scene_idx_topix {
            self.light_manager.set_initial_scene_idx(serde_json::from_str(payload)?).await;
        }

        if self.config.all_light_topics.contains(&topic) {
            let light_message: LightMessage = serde_json::from_str(payload)?;
            self.light_manager.handle_light_message(&topic, light_message);
        }

        if let Some(config) = self.config.buttons.get(&topic) {
            let button_message: HueButtonMessage = serde_json::from_str(payload)?;
            self.light_manager.handle_hue_button_message(&config, button_message).await?;
        }

        if let Some(config) = self.config.dimmers.get(&topic) {
            let dimmer_message: HueDimmerMessage = serde_json::from_str(payload)?;
            self.light_manager.handle_hue_dimmer_message(&config, dimmer_message).await?;
        }

        if let Some(config) = self.config.tap_dials.get(&topic) {
            let tap_dial_message: HueTapDialMessage = serde_json::from_str(payload)?;
            self.light_manager.handle_hue_tap_dial_message(&config, tap_dial_message).await?;
        }

        Ok(())
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        publish_discovery_msg(&self.client, &self.config).await?;
        let stop_requested: AtomicBool = AtomicBool::new(false);

        let subscribe_topics: Vec<SubscribeFilter> = self.config.all_light_topics.iter()
            .chain(self.config.buttons.keys())
            .chain(self.config.dimmers.keys())
            .chain(self.config.tap_dials.keys())
            .chain(std::iter::once(&self.config.mqtt.initial_scene_idx_topix))
            .map(|topic| SubscribeFilter::new(topic.clone(), QoS::AtLeastOnce))
            .collect();
        let subscribe_config = self.config.clone();
        let subscribe_client = self.client.clone();
        let subscribe_lwt = ha_discovery::get_last_will(&self.config);

        let subscribe = async {
            subscribe_client.subscribe_many(subscribe_topics).await?;
            publish_online_msg(&subscribe_client, subscribe_config).await?;
            systemd::notify_ready()?;
            println!("Listening...");

            let mut stop_signal = signal(SignalKind::terminate())?;

            stop_signal.recv().await;
            stop_requested.store(true, std::sync::atomic::Ordering::SeqCst);
            println!("Shutting down...");
            subscribe_client.publish(subscribe_lwt.topic, subscribe_lwt.qos, subscribe_lwt.retain, subscribe_lwt.message).await?;
            subscribe_client.disconnect().await?;

            Ok::<(), RunError>(())
        };

        let main_loop = async {
            loop {
                let result = self.event_loop.poll().await;
                if stop_requested.load(std::sync::atomic::Ordering::SeqCst) && let Err(ConnectionError::MqttState(StateError::ConnectionAborted)) = result {
                    return Ok(());
                }

                match result? {
                    Event::Incoming(Incoming::Publish(p))  => {
                        match self.handle_message(p).await {
                            Ok(()) => {},
                            Err(e) => {
                                eprintln!("WARN: {e:?}");
                            }
                        }
                    }
                    Event::Incoming(_i)  => {
                        // println!("Incoming = {i:?}");
                    }
                    Event::Outgoing(_o)  => {
                        // println!("Outgoing = {o:?}");
                    }
                    // Err(e) => {
                    //     // println!("Error = {e:?}");
                    //
                    // }
                }
            }
            #[allow(unreachable_code)]
            Ok::<(), RunError>(())
        };

        // subscribe and main_loop need to run concurrently
        // otherwise a buffer fills up and subscribe blocks
        tokio::try_join!(main_loop, subscribe)?;

        Ok(())
    }
}


#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let config = match config::load_confg() {
        Ok(config) => config,
        Err(ConfigError::ReadError(e)) => {
            eprintln!("could not read config file: {e}");
            std::process::exit(1);
        }
        Err(ConfigError::ParseError(e)) => {
            eprintln!("could not parse config file: {e}");
            std::process::exit(1);
        }
    };
    let mut oxilight = Oxilight::new(config)?;
    oxilight.run().await?;

    Ok(())
}
