use serde::{Deserialize, Serialize};
use std::{fs, io, path::Path};
#[derive(Default, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub theme: String,
    #[serde(default)]
    pub cell_aspect: Option<f64>,
}
impl Settings {
    pub fn load(path: &Path) -> Self {
        fs::read(path)
            .ok()
            .and_then(|s| serde_json::from_slice(&s).ok())
            .unwrap_or_default()
    }
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(p) = path.parent() {
            fs::create_dir_all(p)?;
        }
        let temp = path.with_extension("json.tmp");
        fs::write(&temp, serde_json::to_vec_pretty(self)?)?;
        fs::rename(temp, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_theme_and_aspect_survive_restart() {
        let dir = std::env::temp_dir().join(format!("htopmodern-settings-{}", std::process::id()));
        let path = dir.join("settings.json");
        let settings = Settings {
            theme: "ocean".into(),
            cell_aspect: Some(2.3),
        };
        settings.save(&path).unwrap();
        let restored = Settings::load(&path);
        assert_eq!(restored.theme, "ocean");
        assert_eq!(restored.cell_aspect, Some(2.3));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
