use std::{fs, path::Path};

use serde::{Deserialize, Serialize};

use crate::{
    error::{Error, Result},
    events::EventKind,
    logging,
    paths::AppPaths,
};

const CONFIG_SCHEMA_VERSION: u32 = 1;

fn default_true() -> bool {
    true
}

fn default_pack() -> String {
    "starter".to_owned()
}

fn default_volume() -> u8 {
    80
}

fn default_threshold() -> u32 {
    50
}

fn default_queue_age() -> u64 {
    5_000
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Config {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_pack")]
    pub active_pack: String,
    #[serde(default = "default_volume")]
    pub volume: u8,
    #[serde(default = "default_threshold")]
    pub massive_push_threshold: u32,
    #[serde(default = "default_queue_age")]
    pub queue_max_age_ms: u64,
    #[serde(default)]
    pub events: EventSettings,
}

fn default_schema_version() -> u32 {
    CONFIG_SCHEMA_VERSION
}

impl Default for Config {
    fn default() -> Self {
        Self {
            schema_version: CONFIG_SCHEMA_VERSION,
            enabled: true,
            active_pack: default_pack(),
            volume: default_volume(),
            massive_push_threshold: default_threshold(),
            queue_max_age_ms: default_queue_age(),
            events: EventSettings::default(),
        }
    }
}

impl Config {
    pub fn parse(text: &str) -> Result<Self> {
        let config: Self =
            toml::from_str(text).map_err(|error| Error::Config(error.to_string()))?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema_version > CONFIG_SCHEMA_VERSION {
            return Err(Error::Config(format!(
                "configuration schema {} is newer than this GitSama build",
                self.schema_version
            )));
        }
        if self.active_pack.trim().is_empty() {
            return Err(Error::Config("active_pack cannot be empty".to_owned()));
        }
        if self.volume > 100 {
            return Err(Error::Config("volume must be between 0 and 100".to_owned()));
        }
        if self.massive_push_threshold == 0 {
            return Err(Error::Config(
                "massive_push_threshold must be at least 1".to_owned(),
            ));
        }
        if self.queue_max_age_ms == 0 {
            return Err(Error::Config(
                "queue_max_age_ms must be greater than 0".to_owned(),
            ));
        }
        Ok(())
    }

    pub fn load_or_default(paths: &AppPaths) -> Self {
        match fs::read_to_string(&paths.config) {
            Ok(text) => match Self::parse(&text) {
                Ok(config) => config,
                Err(error) => {
                    logging::write(paths, &format!("configuration ignored: {error}"));
                    Self::default()
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(error) => {
                logging::write(paths, &format!("configuration unreadable: {error}"));
                Self::default()
            }
        }
    }

    pub fn save(&self, paths: &AppPaths) -> Result<()> {
        self.validate()?;
        paths.ensure_layout()?;
        let text = toml::to_string_pretty(self)
            .map_err(|error| Error::Config(format!("could not serialize configuration: {error}")))?;
        fs::write(&paths.config, text).map_err(|source| Error::WriteFile {
            path: paths.config.clone(),
            source,
        })
    }

    pub fn is_event_enabled(&self, event: EventKind) -> bool {
        self.events.is_enabled(event)
    }

    pub fn set_event_enabled(&mut self, event: EventKind, enabled: bool) {
        self.events.set_enabled(event, enabled);
    }

    pub fn path_exists(&self, path: &Path) -> bool {
        path.exists()
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct EventSettings {
    #[serde(default = "default_true")]
    pub commit: bool,
    #[serde(default = "default_true")]
    pub push: bool,
    #[serde(default = "default_true")]
    pub massive_push: bool,
    #[serde(default = "default_true")]
    pub merge: bool,
    #[serde(default = "default_true")]
    pub branch_switch: bool,
    #[serde(default = "default_true")]
    pub branch_create: bool,
    #[serde(default = "default_true")]
    pub branch_delete: bool,
    #[serde(default = "default_true")]
    pub rebase: bool,
}

impl Default for EventSettings {
    fn default() -> Self {
        Self {
            commit: true,
            push: true,
            massive_push: true,
            merge: true,
            branch_switch: true,
            branch_create: true,
            branch_delete: true,
            rebase: true,
        }
    }
}

impl EventSettings {
    pub fn is_enabled(&self, event: EventKind) -> bool {
        match event {
            EventKind::Commit => self.commit,
            EventKind::Push => self.push,
            EventKind::MassivePush => self.massive_push,
            EventKind::Merge => self.merge,
            EventKind::BranchSwitch => self.branch_switch,
            EventKind::BranchCreate => self.branch_create,
            EventKind::BranchDelete => self.branch_delete,
            EventKind::Rebase => self.rebase,
        }
    }

    pub fn set_enabled(&mut self, event: EventKind, enabled: bool) {
        match event {
            EventKind::Commit => self.commit = enabled,
            EventKind::Push => self.push = enabled,
            EventKind::MassivePush => self.massive_push = enabled,
            EventKind::Merge => self.merge = enabled,
            EventKind::BranchSwitch => self.branch_switch = enabled,
            EventKind::BranchCreate => self.branch_create = enabled,
            EventKind::BranchDelete => self.branch_delete = enabled,
            EventKind::Rebase => self.rebase = enabled,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Config, CONFIG_SCHEMA_VERSION};
    use crate::events::EventKind;

    #[test]
    fn parses_defaults_and_values() {
        let config = Config::parse(
            r#"
            enabled = false
            active_pack = "custom"
            volume = 35
            massive_push_threshold = 7
            [events]
            commit = false
            "#,
        )
        .expect("valid config");

        assert!(!config.enabled);
        assert_eq!(config.active_pack, "custom");
        assert_eq!(config.volume, 35);
        assert_eq!(config.massive_push_threshold, 7);
        assert!(!config.is_event_enabled(EventKind::Commit));
        assert!(config.is_event_enabled(EventKind::Push));
    }

    #[test]
    fn rejects_invalid_values_and_future_schema() {
        assert!(Config::parse("volume = 101").is_err());
        assert!(Config::parse("massive_push_threshold = 0").is_err());
        assert!(Config::parse(&format!("schema_version = {}", CONFIG_SCHEMA_VERSION + 1)).is_err());
    }

    #[test]
    fn event_settings_can_be_changed() {
        let mut config = Config::default();
        config.set_event_enabled(EventKind::Rebase, false);
        assert!(!config.is_event_enabled(EventKind::Rebase));
    }
}

