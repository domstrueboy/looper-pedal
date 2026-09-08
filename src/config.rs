use std::path::PathBuf;

/// Default wait before recording starts, for a fresh install or a config
/// written before the setting existed. Long enough to put a guitar back
/// on and be ready; drag the slider to zero to turn it off.
pub const DEFAULT_PREROLL_MS: u32 = 5000;

/// Persisted device/rate/input-channel/volume/pre-roll choice, stored
/// next to the executable as a small `key=value` file. Doubles as the
/// bundle of settings the looper is started with.
pub struct AppConfig {
    pub device_name: String,
    pub sample_rate: u32,
    /// 0-indexed input channel to capture/record/loop.
    pub input_channel: u16,
    /// Loop playback gain as a percentage of unity (100 = unchanged).
    /// Applied to the loop only, never the live passthrough.
    pub volume_pct: u32,
    /// How long to wait between pressing record and actually capturing,
    /// so there's time to get ready. 0 = start immediately.
    pub preroll_ms: u32,
}

impl AppConfig {
    fn path() -> PathBuf {
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|dir| dir.join("looper-pedal.cfg")))
            .unwrap_or_else(|| PathBuf::from("looper-pedal.cfg"))
    }

    /// `None` if there's no config yet or it's malformed - either way the
    /// caller falls back to the settings picker.
    pub fn load() -> Option<Self> {
        let text = std::fs::read_to_string(Self::path()).ok()?;

        let mut device_name = None;
        let mut sample_rate = None;
        let mut input_channel = None;
        let mut volume_pct = None;
        let mut preroll_ms = None;
        for line in text.lines() {
            let (key, value) = line.split_once('=')?;
            match key {
                "device_name" => device_name = Some(value.to_string()),
                "sample_rate" => sample_rate = value.parse::<u32>().ok(),
                "input_channel" => input_channel = value.parse::<u16>().ok(),
                "volume_pct" => volume_pct = value.parse::<u32>().ok(),
                "preroll_ms" => preroll_ms = value.parse::<u32>().ok(),
                _ => {}
            }
        }

        Some(Self {
            device_name: device_name?,
            sample_rate: sample_rate?,
            input_channel: input_channel?,
            volume_pct: volume_pct?,
            // Defaulted rather than required, so a config written before
            // pre-roll existed still loads instead of throwing the user
            // back to the settings screen.
            preroll_ms: preroll_ms.unwrap_or(DEFAULT_PREROLL_MS),
        })
    }

    pub fn save(&self) -> std::io::Result<()> {
        let text = format!(
            "device_name={}\nsample_rate={}\ninput_channel={}\nvolume_pct={}\npreroll_ms={}\n",
            self.device_name,
            self.sample_rate,
            self.input_channel,
            self.volume_pct,
            self.preroll_ms
        );
        std::fs::write(Self::path(), text)
    }
}
