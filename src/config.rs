use std::{env, fs, path::PathBuf};

use serde::Deserialize;

const DEFAULT_PATH_TEMPLATE: &str = "{repo}-{branch_slug}";

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub worktree: WorktreeConfig,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct WorktreeConfig {
    pub path_template: String,
    pub directory: Option<String>,
}

impl Default for WorktreeConfig {
    fn default() -> Self {
        Self {
            path_template: DEFAULT_PATH_TEMPLATE.to_owned(),
            directory: None,
        }
    }
}

impl Config {
    pub fn load() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let Some(config_path) = config_path() else {
            return Ok(Self::default());
        };

        match fs::read_to_string(&config_path) {
            Ok(contents) => toml::from_str(&contents).map_err(|error| {
                format!("failed to parse {}: {error}", config_path.display()).into()
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(format!("failed to read {}: {error}", config_path.display()).into()),
        }
    }
}

fn config_path() -> Option<PathBuf> {
    config_path_from_dirs(
        env::var_os("XDG_CONFIG_HOME").as_deref(),
        env::var_os("HOME").as_deref(),
    )
}

fn config_path_from_dirs(
    xdg_config_home: Option<&std::ffi::OsStr>,
    home: Option<&std::ffi::OsStr>,
) -> Option<PathBuf> {
    xdg_config_home
        .map(PathBuf::from)
        .or_else(|| home.map(PathBuf::from).map(|path| path.join(".config")))
        .map(|directory| directory.join("yggdrasill").join("config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_current_worktree_naming_rule() {
        let config = Config::default();
        assert_eq!(config.worktree.path_template, "{repo}-{branch_slug}");
    }

    #[test]
    fn defaults_to_placing_worktrees_next_to_the_repository() {
        let config = Config::default();
        assert_eq!(config.worktree.directory, None);
    }

    #[test]
    fn parses_worktree_path_template() {
        let config: Config = toml::from_str(
            r#"
            [worktree]
            path_template = "../{repo}/{branch}/{branch_slug}"
            "#,
        )
        .unwrap();

        assert_eq!(
            config.worktree.path_template,
            "../{repo}/{branch}/{branch_slug}"
        );
    }

    #[test]
    fn parses_worktree_directory() {
        let config: Config = toml::from_str(
            r#"
            [worktree]
            directory = "~/worktrees"
            "#,
        )
        .unwrap();

        assert_eq!(config.worktree.directory.as_deref(), Some("~/worktrees"));
    }

    #[test]
    fn example_configuration_is_valid_when_all_settings_are_enabled() {
        let example = include_str!("../config.example.toml");
        let enabled = example
            .replace(
                "# path_template = \"{repo}-{branch_slug}\"",
                "path_template = \"{repo}-{branch_slug}\"",
            )
            .replace(
                "# directory = \"~/worktrees\"",
                "directory = \"~/worktrees\"",
            );
        let config: Config = toml::from_str(&enabled).unwrap();

        assert_eq!(config.worktree.path_template, "{repo}-{branch_slug}");
        assert_eq!(config.worktree.directory.as_deref(), Some("~/worktrees"));
    }

    #[test]
    fn uses_xdg_config_home_when_set() {
        let path = config_path_from_dirs(
            Some(std::ffi::OsStr::new("/tmp/config")),
            Some(std::ffi::OsStr::new("/home/user")),
        );

        assert_eq!(
            path,
            Some(PathBuf::from("/tmp/config/yggdrasill/config.toml"))
        );
    }

    #[test]
    fn falls_back_to_home_config_directory() {
        let path = config_path_from_dirs(None, Some(std::ffi::OsStr::new("/home/user")));

        assert_eq!(
            path,
            Some(PathBuf::from("/home/user/.config/yggdrasill/config.toml"))
        );
    }
}
