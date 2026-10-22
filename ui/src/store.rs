//! Format-preserving edits to `iidxbg.toml`.
//!
//! The plugin owns the file, so this module never regenerates it from a Rust
//! struct. It mutates only the keys the UI changes, keeps every comment and
//! unknown-to-the-editor whitespace run, validates the complete document with
//! the plugin's own `Config::parse`, and writes it through a temporary file so
//! a scene request never observes a half-written configuration.

use iidxbg::config::{Config, Fit, MoebooruDomain, Source, Surface};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    str::FromStr,
    time::SystemTime,
};
use toml_edit::{DocumentMut, Item, Table, Value};

pub struct ConfigStore {
    path: PathBuf,
    doc: DocumentMut,
    config: Config,
    saved_at: Option<SystemTime>,
    error: Option<String>,
}

impl ConfigStore {
    pub fn load(path: impl Into<PathBuf>) -> Result<Self, String> {
        let path = path.into();
        let text = fs::read_to_string(&path)
            .map_err(|error| format!("无法读取 {}：{error}", path.display()))?;
        let doc = text
            .parse::<DocumentMut>()
            .map_err(|error| format!("TOML 语法错误：{error}"))?;
        let config = Config::parse(&text).map_err(|error| format!("配置无效：{error}"))?;
        Ok(Self {
            path,
            doc,
            config,
            saved_at: None,
            error: None,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn dir(&self) -> &Path {
        self.path.parent().unwrap_or_else(|| Path::new("."))
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn saved_at(&self) -> Option<SystemTime> {
        self.saved_at
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn has_draft_error(&self) -> bool {
        self.error.is_some()
    }

    /// Apply a format-preserving edit and write it if the whole document is
    /// valid. Returns `true` only when the file on disk is up to date.
    pub fn edit(&mut self, edit: impl FnOnce(&mut DocumentMut)) -> bool {
        edit(&mut self.doc);
        self.commit()
    }

    fn commit(&mut self) -> bool {
        let text = self.doc.to_string();
        let parsed = match Config::parse(&text) {
            Ok(config) => config,
            Err(error) => {
                self.error = Some(format!("未保存：{error}"));
                return false;
            }
        };
        match write_atomic(&self.path, &text) {
            Ok(()) => {
                self.config = parsed;
                self.saved_at = Some(SystemTime::now());
                self.error = None;
                true
            }
            Err(error) => {
                self.error = Some(format!("写入 {} 失败：{error}", self.path.display()));
                false
            }
        }
    }

    /// Re-read the file, discarding an invalid draft. Keeps the current state
    /// and reports an error when the file on disk is not valid.
    pub fn reload(&mut self) -> bool {
        let text = match fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(error) => {
                self.error = Some(format!("无法读取 {}：{error}", self.path.display()));
                return false;
            }
        };
        let doc = match text.parse::<DocumentMut>() {
            Ok(doc) => doc,
            Err(error) => {
                self.error = Some(format!("TOML 语法错误：{error}"));
                return false;
            }
        };
        let config = match Config::parse(&text) {
            Ok(config) => config,
            Err(error) => {
                self.error = Some(format!("配置无效：{error}"));
                return false;
            }
        };
        self.doc = doc;
        self.config = config;
        self.error = None;
        true
    }
}

pub fn set_value(doc: &mut DocumentMut, path: &[&str], value: Value) {
    let Some((key, parents)) = path.split_last() else {
        return;
    };
    let table = table_mut(doc, parents);
    if let Some(existing) = table.get_mut(key).and_then(Item::as_value_mut) {
        // Keep an inline comment and the original indentation attached to the
        // value the user edited.
        let decor = existing.decor().clone();
        *existing = value;
        *existing.decor_mut() = decor;
    } else {
        table.insert(key, Item::Value(value));
    }
}

pub fn remove_value(doc: &mut DocumentMut, path: &[&str]) -> bool {
    let Some((key, parents)) = path.split_last() else {
        return false;
    };
    table_mut_existing(doc, parents)
        .and_then(|table| table.remove(key))
        .is_some()
}

fn table_mut_existing<'a>(doc: &'a mut DocumentMut, path: &[&str]) -> Option<&'a mut Table> {
    let mut table = doc.as_table_mut();
    for key in path {
        table = table.get_mut(key)?.as_table_mut()?;
    }
    Some(table)
}

fn table_mut<'a>(doc: &'a mut DocumentMut, path: &[&str]) -> &'a mut Table {
    let mut table = doc.as_table_mut();
    for key in path {
        if !table.get(key).is_some_and(Item::is_table) {
            table.insert(key, Item::Table(Table::new()));
        }
        table = table
            .get_mut(key)
            .and_then(Item::as_table_mut)
            .expect("table was just inserted");
    }
    table
}

pub fn set_bool(doc: &mut DocumentMut, path: &[&str], value: bool) {
    set_value(doc, path, Value::from(value));
}

pub fn set_string(doc: &mut DocumentMut, path: &[&str], value: &str) {
    set_value(doc, path, string_value(value));
}

/// Keep Windows paths readable by writing literal single-quoted strings when
/// the value contains backslashes. Otherwise use the standard quoted form.
fn string_value(value: &str) -> Value {
    if value.contains('\\')
        && !value.contains('\'')
        && !value.contains(['\n', '\r'])
        && let Ok(parsed) = Value::from_str(&format!("'{value}'"))
    {
        return parsed;
    }
    Value::from(value.to_string())
}

pub fn set_u64(doc: &mut DocumentMut, path: &[&str], value: u64) {
    set_value(doc, path, Value::from(value as i64));
}

pub fn set_usize(doc: &mut DocumentMut, path: &[&str], value: usize) {
    set_value(doc, path, Value::from(value as i64));
}

pub fn set_u32(doc: &mut DocumentMut, path: &[&str], value: u32) {
    set_value(doc, path, Value::from(i64::from(value)));
}

pub fn source_key(source: Source) -> &'static str {
    match source {
        Source::Local => "local",
        Source::Remote => "remote",
        Source::Mixed => "mixed",
    }
}

pub fn fit_key(fit: iidxbg::config::Fit) -> &'static str {
    match fit {
        Fit::Cover => "cover",
        Fit::Contain => "contain",
    }
}

pub fn domain_key(domain: MoebooruDomain) -> &'static str {
    match domain {
        MoebooruDomain::Net => "konachan.net",
        MoebooruDomain::Com => "konachan.com",
        MoebooruDomain::Yandere => "yande.re",
    }
}

pub fn surface_switch_value(config: &Config, surface: Surface) -> bool {
    match surface {
        Surface::Results => config.enabled,
        Surface::SongSelect => config.surfaces.song_select,
        Surface::SuddenPlus => config.surfaces.sudden_plus,
        Surface::Lift => config.surfaces.lift,
        Surface::TargetGraph => config.surfaces.target_graph,
        Surface::PremiumArea => config.surfaces.premium_area,
        Surface::Concentration => config.surfaces.concentration,
    }
}

pub fn effective_enabled(config: &Config, surface: Surface) -> bool {
    config.enabled && surface_switch_value(config, surface)
}

fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(text.as_bytes())?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|error| error.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    const FIXTURE: &str = include_str!("testdata/minimal.toml");

    fn fixture_path() -> (PathBuf, PathBuf) {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "iidxbg-config-store-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("iidxbg.toml");
        fs::write(&path, FIXTURE).unwrap();
        (dir, path)
    }

    #[test]
    fn edits_keep_comments_and_write_immediately() {
        let (dir, path) = fixture_path();
        let mut store = ConfigStore::load(&path).unwrap();
        assert!(store.edit(|doc| {
            set_bool(doc, &["enabled"], false);
            set_string(doc, &["moebooru", "domain"], "yande.re");
            set_u32(doc, &["image", "width"], 1280);
        }));

        let saved = fs::read_to_string(&path).unwrap();
        assert!(saved.contains("# iidxbg test fixture."));
        assert!(saved.contains("# API endpoint."));
        assert!(saved.contains("enabled = false"));
        assert!(saved.contains("domain = \"yande.re\""));
        assert!(saved.contains("width = 1280"));
        let parsed = Config::parse(&saved).unwrap();
        assert!(!parsed.enabled);
        assert_eq!(parsed.moebooru.domain, MoebooruDomain::Yandere);
        assert_eq!(parsed.image.width, 1280);
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        assert!(store.saved_at().is_some());

        fs::remove_file(path).unwrap();
        fs::remove_dir(dir).unwrap();
    }

    #[test]
    fn invalid_edit_keeps_the_last_valid_file() {
        let (dir, path) = fixture_path();
        let before = fs::read_to_string(&path).unwrap();
        let mut store = ConfigStore::load(&path).unwrap();
        assert!(!store.edit(|doc| {
            set_u32(doc, &["image", "width"], 1);
        }));
        assert!(store.error().is_some_and(|error| error.contains("未保存")));
        assert_eq!(fs::read_to_string(&path).unwrap(), before);
        assert_eq!(store.config().image.width, 1920);

        // The draft still holds the bad value; a later valid edit replaces it.
        assert!(store.edit(|doc| {
            set_u32(doc, &["image", "width"], 1920);
        }));
        assert!(store.error().is_none());

        fs::remove_file(path).unwrap();
        fs::remove_dir(dir).unwrap();
    }

    #[test]
    fn removing_overrides_falls_back_to_global_settings() {
        let (dir, path) = fixture_path();
        let mut store = ConfigStore::load(&path).unwrap();
        assert!(!store.config().sources.results.is_empty());
        assert!(store.edit(|doc| {
            remove_value(doc, &["sources", "results", "source"]);
            remove_value(doc, &["sources", "results", "local_directory"]);
            remove_value(doc, &["sources", "results", "fit"]);
            remove_value(doc, &["sources", "results", "moebooru", "save_directory"]);
        }));
        let config = store.config();
        assert!(config.sources.results.is_empty());
        let results = config.for_surface(Surface::Results);
        assert_eq!(results.local_directory, Path::new("iidxbg"));
        assert_eq!(results.image.fit, Fit::Contain);
        assert!(config.concentration_shares_premium());

        fs::remove_file(path).unwrap();
        fs::remove_dir(dir).unwrap();
    }

    #[test]
    fn concentration_override_can_be_created_and_removed() {
        let (dir, path) = fixture_path();
        let mut store = ConfigStore::load(&path).unwrap();
        assert!(store.edit(|doc| {
            set_string(doc, &["sources", "concentration", "source"], "remote");
            set_string(
                doc,
                &["sources", "concentration", "local_directory"],
                "iidxbg/concentration",
            );
        }));
        assert!(!store.config().concentration_shares_premium());
        assert!(store.edit(|doc| {
            remove_value(doc, &["sources", "concentration", "source"]);
            remove_value(doc, &["sources", "concentration", "local_directory"]);
        }));
        assert!(store.config().concentration_shares_premium());

        fs::remove_file(path).unwrap();
        fs::remove_dir(dir).unwrap();
    }

    #[test]
    fn reload_replaces_an_invalid_draft() {
        let (dir, path) = fixture_path();
        let mut store = ConfigStore::load(&path).unwrap();
        assert!(!store.edit(|doc| {
            set_u32(doc, &["image", "height"], 0);
        }));
        assert!(store.reload());
        assert!(store.error().is_none());
        assert_eq!(store.config().image.height, 1080);

        fs::remove_file(path).unwrap();
        fs::remove_dir(dir).unwrap();
    }

    #[test]
    fn windows_paths_keep_literal_backslashes() {
        let (dir, path) = fixture_path();
        let mut store = ConfigStore::load(&path).unwrap();
        assert!(store.edit(|doc| {
            set_string(doc, &["local_directory"], r"D:\Pictures\IIDX");
        }));
        let saved = fs::read_to_string(&path).unwrap();
        assert!(saved.contains(r"local_directory = 'D:\Pictures\IIDX'"));
        assert_eq!(
            store.config().local_directory,
            Path::new(r"D:\Pictures\IIDX")
        );

        fs::remove_file(path).unwrap();
        fs::remove_dir(dir).unwrap();
    }
}
