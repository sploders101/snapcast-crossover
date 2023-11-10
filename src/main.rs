use std::{fs::File, collections::HashMap};

use clap::Parser;
use config_types::AudioDevice;
use rumqttc::{MqttOptions, Client};
use serde_json::json;

use crate::connect_airplay::AirplayConnector;

mod config_types;
mod connect_airplay;

#[derive(Parser)]
/// Allows dynamically joining AirPlay speakers to a snapcast server
struct Args {
	#[arg(short)]
	/// The path to the config file
	config_file: String,
}

fn get_state_topic(node_id: &str, instance_id: usize) -> String {
	return format!("{}/{}/state", node_id, instance_id);
}
fn get_command_topic(node_id: &str, instance_id: usize) -> String {
	return format!("{}/{}/command", node_id, instance_id);
}
fn get_availability_topic(node_id: &str) -> String {
	return format!("{}/availability", node_id);
}

fn main() {
	let args = Args::parse();
	let config_file = File::open(args.config_file).expect("Couldn't open config file");
	let config: config_types::Config = serde_yaml::from_reader(config_file).expect("Couldn't read config file");

	eprintln!("Connecting to {}:{} as {}", &config.mqtt_host, config.mqtt_port.unwrap_or(1883), &config.node_id);
	let mut mqtt_options = MqttOptions::new(&config.node_id, &config.mqtt_host, config.mqtt_port.unwrap_or(1883));
	mqtt_options.set_last_will(rumqttc::LastWill::new(get_availability_topic(&config.node_id), "OFF", rumqttc::QoS::AtLeastOnce, true));
	mqtt_options.set_credentials(config.mqtt_user.clone(), config.mqtt_pass.clone());
	let (mut mqtt_client, mut notifications) = Client::new(mqtt_options, 10);

	let mut instances = HashMap::<String, AirplayConnector>::new();

	for device in config.devices.iter() {
		let device_id = get_device_id(device);
		let device_name = get_device_name(&device);

		let state_topic = get_state_topic(&config.node_id, device_id);
		let command_topic = get_command_topic(&config.node_id, device_id);
		let availability_topic = get_availability_topic(&config.node_id);

		// Tell Home Assistant how to talk to this device
		mqtt_client.publish(
			format!(
				"{}/switch/{}/{}/config",
				config.discovery_prefix.as_ref().map(|dp| dp.as_str()).unwrap_or("homeassistant"),
				config.node_id,
				get_device_id(device),
			),
			rumqttc::QoS::AtLeastOnce,
			true,
			serde_json::to_vec(&json!({
				"icon": "mdi:cast-audio",
				"name": device_name,
				"state_topic": state_topic,
				"command_topic": command_topic,
				"availability_topic": availability_topic,
				"payload_available": "ON",
				"payload_not_available": "OFF",
				"unique_id": device_id,
			})).unwrap(),
		).expect("Failed to push discovery message");

		// Subscribe to relevant topics and reset states
		match device {
			AudioDevice::Airplay { name, instance_id, ip_addr } => {
				mqtt_client.subscribe(
					&command_topic,
					rumqttc::QoS::AtLeastOnce,
				).expect("Could not subscribe to topic");
				mqtt_client.publish(&state_topic, rumqttc::QoS::AtLeastOnce, true, "OFF")
					.expect("Could not reset state");

				instances.insert(command_topic, AirplayConnector::new(ip_addr.clone(), *instance_id, state_topic));
			}
		}

		// Create topic-device mapping
	}
	mqtt_client.publish(get_availability_topic(&config.node_id), rumqttc::QoS::AtLeastOnce, true, "ON")
		.expect("Could not signal availability");

	println!("Parking main thread");

	while let Ok(notification) = notifications.recv() {
		match notification {
			Ok(rumqttc::Event::Incoming(rumqttc::Packet::Publish(msg))) => {
				if let Some(connector) = instances.get_mut(&msg.topic) {
					let payload = String::from_utf8_lossy(&msg.payload);
					match payload.as_ref() {
						"ON" => {
							if let Err(err) = connector.connect() {
								eprintln!("{:?}", err);
								let _ = mqtt_client.publish(&connector.state_topic, rumqttc::QoS::AtLeastOnce, true, "OFF");
							} else {
								let _ = mqtt_client.publish(&connector.state_topic, rumqttc::QoS::AtLeastOnce, true, "ON");
							}
						}
						"OFF" => {
							if let Err(err) = connector.disconnect() {
								eprintln!("{:?}", err);
								let _ = mqtt_client.publish(&connector.state_topic, rumqttc::QoS::AtLeastOnce, true, "ON");
							} else {
								let _ = mqtt_client.publish(&connector.state_topic, rumqttc::QoS::AtLeastOnce, true, "OFF");
							}
						}
						_ => {}
					}
				}
			}
			_ => {}
		}
	}
}

fn get_device_id(device: &AudioDevice) -> usize {
	match device {
		AudioDevice::Airplay { instance_id, .. } => *instance_id,
	}
}
fn get_device_name<'a>(device: &'a &AudioDevice) -> &'a str {
	match device {
		AudioDevice::Airplay { name, .. } => name,
	}
}
