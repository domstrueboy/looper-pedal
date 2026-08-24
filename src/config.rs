use std::path::PathBuf;

/// Persisted device/sample-rate/input-channel choice, stored next to the
/// executable as a small `key=value` text file - no need for a
/// serialization crate for three fields.
pub struct AppConfig {
    pub device_name: String,
    pub sample_rate: u32,
    /// 0-indexed input channel to capture/record/loop.
    pub input_channel: u16,
}

impl AppConfig {
    fn path() -> PathBuf {
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|dir| dir.join("looper-pedal.cfg")))
            .unwrap_or_else(|| PathBuf::from("looper-pedal.cfg"))
    }

    /// Returns `None` if there's no saved config yet, or if it's missing
    /// fields / malformed - either way, the caller should fall back to
    /// showing the settings picker.
    pub fn load() -> Option<Self> {
        let text = std::fs::read_to_string(Self::path()).ok()?;

        let mut device_name = None;
        let mut sample_rate = None;
        let mut input_channel = None;
        for line in text.lines() {
            let (key, value) = line.split_once('=')?;
            match key {
                "device_name" => device_name = Some(value.to_string()),
                "sample_rate" => sample_rate = value.parse::<u32>().ok(),
                "input_channel" => input_channel = value.parse::<u16>().ok(),
                _ => {}
            }
        }

        Some(Self {
            device_name: device_name?,
            sample_rate: sample_rate?,
            input_channel: input_channel?,
        })
    }

    pub fn save(&self) -> std::io::Result<()> {
        let text = format!(
            "device_name={}\nsample_rate={}\ninput_channel={}\n",
            self.device_name, self.sample_rate, self.input_channel
        );
        std::fs::write(Self::path(), text)
    }
}
