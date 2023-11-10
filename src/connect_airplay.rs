use std::{process::{Child, Command, Stdio}, io::Write, time::Duration};
use anyhow::Context;

pub struct AirplayConnector {
	ip: String,
	pub state_topic: String,
	client_id: usize,
	connected_airplay: Option<AirplayConnected>,
}
impl AirplayConnector {
	pub fn new(ip: String, client_id: usize, state_topic: String) -> Self {
		Self {
			ip,
			client_id,
			state_topic,
			connected_airplay: None,
		}
	}

	pub fn connect(&mut self) -> anyhow::Result<()> {
		if self.connected_airplay.is_none() {
			self.connected_airplay = Some(AirplayConnected::new(&self.ip, self.client_id)?);
		}
		return Ok(());
	}

	pub fn disconnect(&mut self) -> anyhow::Result<()> {
		self.connected_airplay.take();
		return Ok(());
	}
}
struct AirplayConnected {
	pipewire: Child,
	snapclient: Child,
}
impl AirplayConnected {
	pub fn new(ip: &str, client_id: usize) -> anyhow::Result<Self> {
		let pipewire = connect_airplay_pipewire(ip)?;
		let snapclient = connect_airplay_snapclient(ip, client_id)?;
		return Ok(Self {
			pipewire,
			snapclient,
		});
	}
}
impl Drop for AirplayConnected {
	fn drop(&mut self) {
		if let Err(err) = self.snapclient.kill() {
			eprintln!("{:?}", err);
		}
		if let Err(err) = self.snapclient.wait() {
			eprintln!("{:?}", err);
		}
		if let Err(err) = self.pipewire.kill() {
			eprintln!("{:?}", err);
		}
		if let Err(err) = self.pipewire.wait() {
			eprintln!("{:?}", err);
		}
	}
}

// This function is not safe. Shell injection could be used here, but the config
// should only be editable by a legitimate user, so I'm letting it slide for now
fn connect_airplay_pipewire(ip: &str) -> anyhow::Result<Child> {
	// Start pw-cli
	let mut command_ext = Command::new("pw-cli")
		.stdin(Stdio::piped())
		.stdout(Stdio::null())
		.spawn()
		.context("Failed to spawn pw-cli")?;

	// Write load-module command to pw-cli
	command_ext
		.stdin
		.as_mut()
		.context("Missing pw-cli stdin")?
		.write_fmt(format_args!("load-module libpipewire-module-raop-discover stream.rules = [ {{ matches = [ {{ raop.ip = \"{ip}\" }} ] actions = {{ create-stream = {{ stream.props = {{ }} }} }} }} ]\n"))
		.context("Couldn't write command to pw-cli")?;

	std::thread::sleep(Duration::from_millis(5000));

	return Ok(command_ext);
}

fn connect_airplay_snapclient(ip: &str, client_id: usize) -> anyhow::Result<Child> {
	// snapclient -i 100 -s 10.0.0.234 --player pulse --mixer hardware
	let command_ext = Command::new("snapclient")
		.args(["-i", &client_id.to_string()])
		.args(["-s", ip])
		.args(["--player", "pulse"])
		.args(["--mixer", "hardware"])
		.spawn()
		.context("Failed to spawn snapclient")?;

	return Ok(command_ext);
}
