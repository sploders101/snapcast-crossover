use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
	pub discovery_prefix: Option<String>,
	pub node_id: String,
	pub mqtt_host: String,
	pub mqtt_port: Option<u16>,
	pub mqtt_user: String,
	pub mqtt_pass: String,
	pub devices: Vec<AudioDevice>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum AudioDevice {
	Airplay {
		name: String,
		instance_id: usize,
		ip_addr: String,
	},
}