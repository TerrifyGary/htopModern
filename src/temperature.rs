use std::{
    io::Read,
    path::PathBuf,
    process::{Command, Stdio},
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant},
};
#[derive(Debug, Default)]
pub struct Reading {
    pub cpu: Option<f64>,
    pub gpu: Option<f64>,
    pub cpu_power: Option<f64>,
    pub gpu_power: Option<f64>,
}
fn valid(value: &serde_json::Value) -> Option<f64> {
    value
        .as_f64()
        .filter(|v| v.is_finite() && *v > 0. && *v < 150.)
}
fn parse(text: &str) -> Result<Reading, String> {
    let value: serde_json::Value = text
        .lines()
        .rev()
        .find_map(|l| serde_json::from_str(l).ok())
        .ok_or("Invalid sensor response")?;
    let reading = Reading {
        cpu: valid(&value["temp"]["cpu_temp_avg"]),
        gpu: valid(&value["temp"]["gpu_temp_avg"]),
        cpu_power: value["cpu_power"].as_f64(),
        gpu_power: value["gpu_power"].as_f64(),
    };
    if reading.cpu.is_none() && reading.gpu.is_none() {
        return Err("No supported temperature sensors".into());
    }
    Ok(reading)
}
fn executable() -> Option<PathBuf> {
    let mut paths = vec![
        PathBuf::from("/opt/homebrew/bin/macmon"),
        PathBuf::from("/usr/local/bin/macmon"),
    ];
    if let Some(home) = std::env::var_os("HOME") {
        paths.push(PathBuf::from(home).join(".cargo/bin/macmon"));
    }
    if let Some(path) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&path).map(|p| p.join("macmon")));
    }
    paths.into_iter().find(|p| p.is_file())
}
pub fn sample() -> Result<Reading, String> {
    let path = executable().ok_or("Install macmon for temperature (no sudo)")?;
    let mut child = Command::new(path)
        .args(["pipe", "-s", "1", "-i", "100"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let stdout = child.stdout.take().ok_or("Sensor output unavailable")?;
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        stdout.take(131072).read_to_string(&mut s).map(|_| s)
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match child.try_wait() {
            Ok(Some(s)) => {
                if !s.success() {
                    return Err("Sensor access unavailable".into());
                }
                break;
            }
            Ok(None) => {}
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e.to_string());
            }
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Temperature sensor timed out".into());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    parse(
        &reader
            .join()
            .map_err(|_| "Sensor reader failed")?
            .map_err(|e| e.to_string())?,
    )
}
pub struct Temperature {
    pub reading: Option<Reading>,
    pub status: String,
    pending: Option<Receiver<Result<Reading, String>>>,
    last: Instant,
}
impl Temperature {
    pub fn new() -> Self {
        Self {
            reading: None,
            status: "Sensor starting…".into(),
            pending: None,
            last: Instant::now() - Duration::from_secs(5),
        }
    }
    pub fn tick(&mut self) {
        let result = self.pending.as_ref().and_then(|rx| match rx.try_recv() {
            Ok(r) => Some(r),
            Err(mpsc::TryRecvError::Disconnected) => Some(Err("Sensor worker stopped".into())),
            Err(_) => None,
        });
        if let Some(result) = result {
            self.pending = None;
            match result {
                Ok(reading) => {
                    self.reading = Some(reading);
                    self.status = "macmon · no sudo".into();
                }
                Err(error) => {
                    self.reading = None;
                    self.status = error;
                }
            }
        }
        if self.pending.is_none() && self.last.elapsed() >= Duration::from_secs(5) {
            self.last = Instant::now();
            let (tx, rx) = mpsc::channel();
            self.pending = Some(rx);
            std::thread::spawn(move || {
                let _ = tx.send(sample());
            });
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_sensor_values_are_not_shown() {
        assert!(parse(r#"{"temp":{"cpu_temp_avg":0,"gpu_temp_avg":null}}"#).is_err());
        let r = parse(r#"{"temp":{"cpu_temp_avg":48.2,"gpu_temp_avg":45.1}}"#).unwrap();
        assert_eq!(r.cpu, Some(48.2));
    }
}
