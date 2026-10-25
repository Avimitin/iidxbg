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
    full: Config,
    saved_at: Option<SystemTime>,
    error: Option<String>,
    scope: Scope,
    scoped: Config,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Scope {
    Legacy,
    PreLogin,
    Assets,
    User(String),
}
const USER_KEYS: &[&str] = &["surfaces", "sources", "selection", "mmd", "results_video"];

impl ConfigStore {
    pub fn load(path: impl Into<PathBuf>) -> Result<Self, String> {
        let path = path
            .into()
            .canonicalize()
            .map_err(|e| format!("无法打开配置：{e}"))?;
        let text = fs::read_to_string(&path)
            .map_err(|error| format!("无法读取 {}：{error}", path.display()))?;
        let mut doc = text
            .parse::<DocumentMut>()
            .map_err(|error| format!("TOML 语法错误：{error}"))?;
        let mut config = Config::parse(&text).map_err(|error| format!("配置无效：{error}"))?;
        promote_artwork_document(&mut doc);
        config.config_path = path.clone();
        Ok(Self {
            path,
            doc,
            scoped: config.clone(),
            full: config,
            scope: Scope::Legacy,
            saved_at: None,
            error: None,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn config(&self) -> &Config {
        &self.scoped
    }

    pub fn document_config(&self) -> &Config {
        &self.full
    }
    pub fn scope(&self) -> &Scope {
        &self.scope
    }
    pub fn set_scope(&mut self, scope: Scope) {
        self.scope = scope;
        self.refresh_scope();
    }
    fn refresh_scope(&mut self) {
        self.full.config_path = self.path.clone();
        if let Scope::User(id) = &self.scope
            && !self.full.users.contains_key(id)
        {
            self.scope = Scope::PreLogin;
        }
        self.scoped = match &self.scope {
            Scope::Legacy => self.full.clone(),
            Scope::PreLogin => self.full.for_user(None),
            Scope::Assets => self.full.for_assets(),
            Scope::User(id) => self.full.for_user(Some(id)),
        };
    }
    pub fn accepts_surface(&self, surface: Surface) -> bool {
        match self.scope {
            Scope::Legacy => true,
            Scope::PreLogin | Scope::Assets => iidxbg::users::is_pre_login(surface),
            Scope::User(_) => !iidxbg::users::is_pre_login(surface),
        }
    }

    fn scoped_document(&self) -> DocumentMut {
        if self.scope == Scope::Legacy {
            return self.doc.clone();
        }
        let mut doc = self.doc.clone();
        doc.remove("users");
        doc.remove("pre_login");
        for key in USER_KEYS {
            if self.scope != Scope::Assets {
                doc.remove(key);
            }
        }
        set_bool(&mut doc, &["surfaces", "results"], false);
        match &self.scope {
            Scope::User(id) => {
                if let Some(settings) = self
                    .doc
                    .get("users")
                    .and_then(|u| u.get(id))
                    .and_then(|u| u.get("settings"))
                {
                    for key in USER_KEYS {
                        if let Some(value) = settings.get(key) {
                            doc[key] = value.clone();
                        }
                    }
                }
            }
            Scope::PreLogin | Scope::Assets => {
                for surface in [Surface::Title, Surface::Login] {
                    set_bool(
                        &mut doc,
                        &["surfaces", surface.name()],
                        self.scoped.surface_enabled(surface),
                    );
                    let source = if self.doc.contains_key("pre_login") {
                        self.doc
                            .get("pre_login")
                            .and_then(|p| p.get(surface.name()))
                            .and_then(|p| p.get("source"))
                    } else {
                        self.doc.get("sources").and_then(|p| p.get(surface.name()))
                    }
                    .cloned()
                    .unwrap_or(Item::Table(Table::new()));
                    table_mut(&mut doc, &["sources"]).insert(surface.name(), source);
                }
            }
            Scope::Legacy => {}
        }
        if doc.get("surfaces").and_then(|s| s.get("results")).is_none() {
            set_bool(&mut doc, &["surfaces", "results"], false);
        }
        doc
    }

    #[cfg(test)]
    pub fn saved_at(&self) -> Option<SystemTime> {
        self.saved_at
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Apply a format-preserving edit and write it if the whole document is
    /// valid. Returns `true` only when the file on disk is up to date.
    pub fn edit(&mut self, edit: impl FnOnce(&mut DocumentMut)) -> bool {
        if self.scope == Scope::Legacy {
            edit(&mut self.doc);
            return self.commit();
        }
        let mut draft = self.scoped_document();
        edit(&mut draft);
        let scope = self.scope.clone();
        self.edit_root(|doc| {
            if draft.get("enabled").and_then(Item::as_bool) == Some(true) {
                set_bool(doc, &["enabled"], true);
            }
            match scope {
                Scope::User(id) => {
                    let table = table_mut(doc, &["users", &id, "settings"]);
                    for key in USER_KEYS {
                        if let Some(value) = draft.get(key) {
                            table.insert(key, value.clone());
                        } else {
                            table.remove(key);
                        }
                    }
                }
                Scope::PreLogin | Scope::Assets => {
                    for surface in [Surface::Title, Surface::Login] {
                        let name = surface.name();
                        set_bool(
                            doc,
                            &["pre_login", name, "enabled"],
                            draft["surfaces"][name].as_bool().unwrap_or(false),
                        );
                        if let Some(source) = draft.get("sources").and_then(|s| s.get(name)) {
                            table_mut(doc, &["pre_login", name]).insert("source", source.clone());
                        }
                    }
                }
                Scope::Legacy => unreachable!(),
            }
        })
    }

    pub fn edit_root(&mut self, edit: impl FnOnce(&mut DocumentMut)) -> bool {
        let before = self.doc.clone();
        edit(&mut self.doc);
        if self.commit() {
            true
        } else {
            self.doc = before;
            false
        }
    }
    pub fn create_user(&mut self, name: &str) -> Option<String> {
        let id = format!(
            "user-{:x}",
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .ok()?
                .as_nanos()
        );
        self.edit_root(|doc| {
            set_string(doc, &["users", &id, "name"], name.trim());
            for surface in Surface::ALL {
                set_bool(
                    doc,
                    &["users", &id, "settings", "surfaces", surface.name()],
                    false,
                );
            }
        })
        .then_some(id)
    }
    pub fn bind_card(&mut self, id: &str, card: &str) -> bool {
        let card = match iidxbg::users::normalize_card(card) {
            Ok(c) => c,
            Err(e) => {
                self.error = Some(e.to_string());
                return false;
            }
        };
        let Some(user) = self.full.users.get(id) else {
            return false;
        };
        let mut cards = user.cards.clone();
        cards.push(card);
        self.edit_root(|doc| set_strings(doc, &["users", id, "cards"], &cards))
    }

    pub(crate) fn artwork_document(&self, index: usize) -> DocumentMut {
        let mut doc = self.scoped_document();
        let art = &self.scoped.artwork[index];
        let source = doc["artwork"]
            .as_array_of_tables()
            .and_then(|a| a.get(index))
            .and_then(|t| t.get("source"))
            .cloned()
            .unwrap_or(Item::Table(Table::new()));
        table_mut(&mut doc, &["sources"]).insert("transition", source);
        set_value(&mut doc, &["surfaces", "transition"], art.enabled.into());
        doc
    }
    pub(crate) fn edit_artwork(
        &mut self,
        index: usize,
        edit: impl FnOnce(&mut DocumentMut),
    ) -> bool {
        let mut draft = self.artwork_document(index);
        edit(&mut draft);
        let source = draft["sources"]["transition"].clone();
        let enabled = draft["surfaces"]["transition"].clone();
        self.edit_root(|doc| {
            if let Some(table) = doc["artwork"]
                .as_array_of_tables_mut()
                .and_then(|a| a.get_mut(index))
            {
                table.insert("source", source);
                table.insert("enabled", enabled);
            }
        })
    }
    pub(crate) fn add_artwork(&mut self, archive: &str, texture: &str) -> Option<usize> {
        if let Some(index) = self
            .scoped
            .artwork
            .iter()
            .position(|a| a.archive == archive && a.texture == texture)
        {
            return Some(index);
        }
        let index = self.scoped.artwork.len();
        self.edit_root(|doc| {
            if !doc.contains_key("artwork") {
                doc["artwork"] = Item::ArrayOfTables(toml_edit::ArrayOfTables::new());
            }
            let mut table = Table::new();
            table.insert("archive", toml_edit::value(archive));
            table.insert("texture", toml_edit::value(texture));
            table.insert("enabled", toml_edit::value(false));
            doc["artwork"].as_array_of_tables_mut().unwrap().push(table);
        })
        .then_some(index)
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
                self.full = parsed;
                self.refresh_scope();
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
        let mut doc = match text.parse::<DocumentMut>() {
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
        promote_artwork_document(&mut doc);
        self.doc = doc;
        self.full = config;
        self.refresh_scope();
        self.error = None;
        true
    }
}

/// Mirror Config's promotion while retaining the original TOML tables/comments.
/// The normalized document is written only when the user makes an edit.
fn promote_artwork_document(doc: &mut DocumentMut) {
    fn tables(item: Item) -> toml_edit::ArrayOfTables {
        if item.as_array().is_some_and(|array| array.is_empty()) {
            return toml_edit::ArrayOfTables::new();
        }
        item.into_array_of_tables().expect("validated artwork")
    }
    // Accept both [[artwork]] and an inline array of tables.
    if let Some(artwork) = doc.remove("artwork") {
        doc["artwork"] = Item::ArrayOfTables(tables(artwork));
    }
    let mut ids: Vec<_> = doc
        .get("users")
        .and_then(Item::as_table_like)
        .map(|users| users.iter().map(|(id, _)| id.to_owned()).collect())
        .unwrap_or_default();
    ids.sort();
    for id in ids {
        let legacy = doc
            .get_mut("users")
            .and_then(|users| users.get_mut(&id))
            .and_then(|user| user.get_mut("settings"))
            .and_then(Item::as_table_like_mut)
            .and_then(|settings| settings.remove("artwork"));
        let Some(legacy) = legacy else { continue };
        let entries = tables(legacy);
        for entry in entries {
            let exists = doc
                .get("artwork")
                .and_then(Item::as_array_of_tables)
                .is_some_and(|entries| {
                    entries.iter().any(|existing| {
                        ["archive", "texture"].iter().all(|key| {
                            existing.get(key).and_then(Item::as_str)
                                == entry.get(key).and_then(Item::as_str)
                        })
                    })
                });
            if !exists {
                if !doc.contains_key("artwork") {
                    doc["artwork"] = Item::ArrayOfTables(Default::default());
                }
                doc["artwork"]
                    .as_array_of_tables_mut()
                    .expect("artwork tables")
                    .push(entry);
            }
        }
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

pub fn set_character_source(doc: &mut DocumentMut, source: iidxbg::config::CharacterSource) {
    set_string(
        doc,
        &["selection", "frame_chara_source"],
        character_source_key(source),
    );
    if source == iidxbg::config::CharacterSource::Mmd {
        set_bool(doc, &["mmd", "enabled"], true);
    }
}

fn character_source_key(source: iidxbg::config::CharacterSource) -> &'static str {
    use iidxbg::config::CharacterSource;
    match source {
        CharacterSource::Auto => "auto",
        CharacterSource::Stock => "stock",
        CharacterSource::Image => "image",
        CharacterSource::Mmd => "mmd",
    }
}

pub fn set_qpro(
    doc: &mut DocumentMut,
    enabled: bool,
    current_character: iidxbg::config::CharacterSource,
) {
    // Resolve legacy auto before enabling MMD: Qpro must not change the main character.
    set_string(
        doc,
        &["selection", "frame_chara_source"],
        character_source_key(current_character),
    );
    set_bool(doc, &["mmd", "replace_qpro"], enabled);
    if enabled {
        set_bool(doc, &["mmd", "enabled"], true);
    }
}

pub fn set_string(doc: &mut DocumentMut, path: &[&str], value: &str) {
    set_value(doc, path, string_value(value));
}

pub fn set_strings(doc: &mut DocumentMut, path: &[&str], values: &[String]) {
    let array = values
        .iter()
        .map(|value| value.as_str())
        .collect::<toml_edit::Array>();
    set_value(doc, path, Value::Array(array));
}

pub fn set_surface_enabled(doc: &mut DocumentMut, surface: Surface, enabled: bool) {
    if enabled && doc.get("enabled").and_then(Item::as_bool) == Some(false) {
        for other in Surface::ALL {
            set_bool(doc, &["surfaces", other.name()], false);
        }
        set_bool(doc, &["enabled"], true);
    }
    set_bool(doc, &["surfaces", surface.name()], enabled);
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
pub fn set_f64(doc: &mut DocumentMut, path: &[&str], value: f64) {
    set_value(doc, path, Value::from(value));
}

#[cfg(test)]
pub fn set_u32(doc: &mut DocumentMut, path: &[&str], value: u32) {
    set_value(doc, path, Value::from(i64::from(value)));
}

pub fn source_key(source: Source) -> &'static str {
    match source {
        Source::Bilibili => "bilibili",
        Source::File => "file",
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
        Surface::Results => config.surfaces.results,
        Surface::SongSelect => config.surfaces.song_select,
        Surface::SuddenPlus => config.surfaces.sudden_plus,
        Surface::Lift => config.surfaces.lift,
        Surface::TargetGraph => config.surfaces.target_graph,
        Surface::PremiumArea => config.surfaces.premium_area,
        Surface::Concentration => config.surfaces.concentration,
        Surface::NoteSkin => config.surfaces.note_skin,
        Surface::GameplayShell => config.surfaces.gameplay_shell,
        Surface::Decision => config.surfaces.decision,
        Surface::Transition => config.surfaces.transition,
        Surface::Title => config.surfaces.title,
        Surface::Login => config.surfaces.login,
        Surface::ModeSelect => config.surfaces.mode_select,
        Surface::GameplayBottom => config.surfaces.gameplay_bottom,
        Surface::Logout => config.surfaces.logout,
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

    #[test]
    fn ui_and_runtime_keep_the_same_user_account_across_edits_and_reload() {
        let (_dir, path) = fixture_path();
        let mut store = ConfigStore::load(&path).unwrap();
        let a = store.create_user("Alice").unwrap();
        let b = store.create_user("Bob").unwrap();
        store.set_scope(Scope::User(a.clone()));
        let key = store.config().bilibili.account.clone();
        assert!(key.is_some());
        assert_eq!(
            Config::load(&path)
                .unwrap()
                .for_user(Some(&a))
                .bilibili
                .account,
            key
        );
        assert!(store.edit(|doc| set_surface_enabled(doc, Surface::Results, true)));
        assert!(store.reload());
        assert_eq!(
            crate::preview::specs(store.config())[Surface::Results.index()]
                .fingerprint
                .bilibili
                .account,
            key
        );
        store.set_scope(Scope::User(b));
        assert_ne!(store.config().bilibili.account, key);
        store.set_scope(Scope::PreLogin);
        assert!(store.config().bilibili.account.is_none());
        store.set_scope(Scope::User(a));
        assert_eq!(store.config().bilibili.account, key);
        assert!(!fs::read_to_string(path).unwrap().contains("config_path"));
    }

    #[test]
    fn user_settings_and_cards_are_isolated_but_artwork_is_shared() {
        let (_dir, path) = fixture_path();
        let mut store = ConfigStore::load(&path).unwrap();
        let legacy = store.document_config().clone();
        let a = store.create_user("Alice").unwrap();
        let b = store.create_user("Bob").unwrap();
        assert!(store.bind_card(&a, "e004010000000001"));
        assert!(store.bind_card(&a, "E004010000000002"));
        let before_duplicate = fs::read_to_string(&path).unwrap();
        assert!(!store.bind_card(&b, "E004010000000001"));
        assert_eq!(fs::read_to_string(&path).unwrap(), before_duplicate);
        store.set_scope(Scope::User(a.clone()));
        assert!(store.edit(|doc| {
            set_surface_enabled(doc, Surface::SongSelect, true);
            set_string(
                doc,
                &["sources", "song_select", "local_directory"],
                "alice-art",
            );
            set_bool(doc, &["selection", "frame_chara"], false);
        }));
        let index = store.add_artwork("example.ifs", "tile").unwrap();
        assert!(store.edit_artwork(index, |doc| set_surface_enabled(
            doc,
            Surface::Transition,
            true
        )));
        assert!(store.config().artwork[0].enabled);
        store.set_scope(Scope::User(b.clone()));
        assert!(!store.config().surface_enabled(Surface::SongSelect));
        assert!(!store.config().surface_enabled(Surface::Results));
        assert!(!store.config().hide_frame_chara());
        assert!(store.config().artwork[0].enabled);
        assert!(
            store
                .document_config()
                .users
                .values()
                .all(|u| u.settings.artwork.is_empty())
        );
        assert_eq!(store.document_config().sources, legacy.sources);
        assert_eq!(store.document_config().surfaces, legacy.surfaces);
        assert_eq!(store.document_config().users[&a].cards.len(), 2);
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("#"));
        let fresh = ConfigStore::load(path).unwrap();
        assert_eq!(
            fresh.document_config().users[&a]
                .settings
                .sources
                .song_select
                .local_directory
                .as_deref(),
            Some(Path::new("alice-art"))
        );
    }

    #[test]
    fn pre_login_edits_do_not_touch_users_and_new_users_start_disabled() {
        let (_dir, path) = fixture_path();
        let mut store = ConfigStore::load(&path).unwrap();
        assert!(store.edit(|doc| {
            set_surface_enabled(doc, Surface::SongSelect, true);
            set_bool(doc, &["selection", "frame_chara"], false);
        }));
        let empty = store.create_user("Empty").unwrap();
        let second = store.create_user("Second").unwrap();
        assert!(
            !store.document_config().users[&empty]
                .settings
                .surfaces
                .song_select
        );
        assert!(
            !store.document_config().users[&second]
                .settings
                .surfaces
                .song_select
        );
        let before = store.document_config().users.clone();
        store.set_scope(Scope::PreLogin);
        assert!(store.edit(|doc| {
            set_surface_enabled(doc, Surface::Title, true);
            set_string(doc, &["sources", "title", "local_directory"], "cabinet");
        }));
        assert_eq!(store.document_config().users, before);
        assert!(
            store
                .document_config()
                .pre_login
                .as_ref()
                .unwrap()
                .title
                .enabled
        );
        assert!(store.config().surface_enabled(Surface::Title));
        assert!(!store.config().hide_frame_chara());
        assert!(!store.config().surface_enabled(Surface::SongSelect));
        assert!(store.reload());
        assert!(store.config().surface_enabled(Surface::Title));
    }

    #[test]
    fn new_users_do_not_inherit_legacy_character_or_model() {
        let (_dir, path) = fixture_path();
        let mut store = ConfigStore::load(&path).unwrap();
        assert!(store.edit(|doc| {
            set_bool(doc, &["enabled"], false);
            set_bool(doc, &["selection", "frame_chara"], false);
            set_bool(doc, &["mmd", "enabled"], true);
        }));
        let id = store.create_user("New user").unwrap();
        store.set_scope(Scope::User(id));
        assert!(store.edit(|doc| set_surface_enabled(doc, Surface::Lift, true)));
        assert!(store.config().surface_enabled(Surface::Lift));
        assert!(!store.config().surface_enabled(Surface::Results));
        assert!(!store.config().hide_frame_chara());
        assert!(!store.config().uses_mmd());
    }

    #[test]
    fn partial_user_surface_edit_does_not_enable_results_and_invalid_edit_rolls_back() {
        let (_dir, path) = fixture_path();
        let mut store = ConfigStore::load(&path).unwrap();
        assert!(store.edit_root(|doc| {
            set_string(doc, &["users", "a", "name"], "A");
            set_bool(
                doc,
                &["users", "a", "settings", "surfaces", "song_select"],
                true,
            );
        }));
        store.set_scope(Scope::User("a".into()));
        assert!(store.edit(|doc| set_string(
            doc,
            &["sources", "song_select", "local_directory"],
            "a"
        )));
        assert!(!store.config().surface_enabled(Surface::Results));
        let before = fs::read_to_string(&path).unwrap();
        assert!(!store.edit(|doc| set_string(doc, &["sources", "song_select", "bind"], "title")));
        assert_eq!(fs::read_to_string(&path).unwrap(), before);
        assert!(store.edit(|doc| set_surface_enabled(doc, Surface::Lift, true)));
    }

    #[test]
    fn qpro_switch_preserves_legacy_stock_image_and_explicit_main_source() {
        use iidxbg::config::CharacterSource;
        for image in ["", "saved-character.png"] {
            let (dir, path) = fixture_path();
            let mut store = ConfigStore::load(&path).unwrap();
            assert!(store.edit(|doc| set_string(doc, &["selection", "frame_chara_image"], image)));
            let main = store.config().character_source();
            assert!(store.edit(|doc| set_qpro(doc, true, main)));
            assert_eq!(store.config().character_source(), main);
            assert!(!store.config().uses_mmd_character());
            assert!(store.config().uses_mmd());
            for source in [
                CharacterSource::Mmd,
                CharacterSource::Stock,
                CharacterSource::Image,
            ] {
                assert!(store.edit(|doc| set_character_source(doc, source)));
                assert!(store.config().mmd.replace_qpro);
                assert_eq!(
                    store.config().selection.frame_chara_image,
                    PathBuf::from(image)
                );
            }
            assert!(store.edit(|doc| set_qpro(doc, false, CharacterSource::Image)));
            assert_eq!(store.config().character_source(), CharacterSource::Image);
            assert!(!store.config().uses_mmd());
            fs::remove_file(path).unwrap();
            fs::remove_dir(dir).unwrap();
        }
    }
    use std::time::{SystemTime, UNIX_EPOCH};

    const FIXTURE: &str = include_str!("testdata/minimal.toml");

    #[test]
    fn binding_cycles_never_reach_disk_and_results_switch_is_independent() {
        let (dir, path) = fixture_path();
        let mut store = ConfigStore::load(&path).unwrap();
        assert!(store.edit(|doc| {
            set_surface_enabled(doc, Surface::Lift, true);
            set_surface_enabled(doc, Surface::Results, false);
            set_string(doc, &["sources", "sudden_plus", "bind"], "lift");
            set_string(doc, &["sources", "lift", "source"], "file");
            set_string(
                doc,
                &["sources", "lift", "media_path"],
                r"D:\Videos\test.mp4",
            );
        }));
        assert!(store.config().surface_enabled(Surface::Lift));
        assert!(!store.config().surface_enabled(Surface::Results));
        assert_eq!(
            store.config().binding_root(Surface::SuddenPlus).unwrap(),
            Surface::Lift
        );
        let saved = fs::read_to_string(&path).unwrap();
        assert!(!store.edit(|doc| set_string(doc, &["sources", "lift", "bind"], "sudden_plus")));
        assert_eq!(fs::read_to_string(&path).unwrap(), saved);
        assert!(store.reload());
        assert!(store.edit(|doc| {
            set_bool(doc, &["enabled"], false);
            set_surface_enabled(doc, Surface::SongSelect, true);
        }));
        assert!(store.config().surface_enabled(Surface::SongSelect));
        assert!(!store.config().surface_enabled(Surface::Lift));
        fs::remove_file(path).unwrap();
        fs::remove_dir(dir).unwrap();
    }

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
    fn mmd_condition_paths_survive_reload_and_invalid_rate_keeps_disk() {
        let (dir, path) = fixture_path();
        let mut store = ConfigStore::load(&path).unwrap();
        let paths = [
            ("motion_path", r"D:\动作\dance.vmd"),
            ("scroll_up_motion_path", r"D:\动作\up.vmd"),
            ("scroll_down_motion_path", r"D:\动作\down.vmd"),
            ("happy_motion_path", r"D:\动作\happy.vmd"),
            ("other_result_motion_path", r"D:\动作\other.vmd"),
        ];
        assert!(store.edit(|doc| {
            set_bool(doc, &["mmd", "enabled"], true);
            set_string(doc, &["mmd", "model_path"], r"D:\模型\model.pmx");
            set_bool(doc, &["mmd", "bpm_sync"], true);
            set_f64(doc, &["mmd", "motion_bpm"], 148.5);
            set_f64(doc, &["mmd", "display_scale"], 42.);
            set_f64(doc, &["mmd", "position_x"], 20.);
            set_f64(doc, &["mmd", "position_y"], 32.);
            set_bool(doc, &["mmd", "replace_qpro"], true);
            set_bool(doc, &["mmd", "keep_hud_clear"], false);
            set_f64(doc, &["mmd", "qpro_height"], 360.);
            for (key, value) in paths {
                set_string(doc, &["mmd", key], value);
            }
        }));
        assert!(store.reload());
        assert!(store.config().mmd.bpm_sync);
        assert_eq!(store.config().mmd.motion_bpm, 148.5);
        assert_eq!(store.config().mmd.display_scale, 42.);
        assert_eq!(store.config().mmd.position_x, 20.);
        assert_eq!(store.config().mmd.position_y, 32.);
        assert!(store.config().mmd.replace_qpro);
        assert!(!store.config().mmd.keep_hud_clear);
        assert_eq!(store.config().mmd.qpro_height, 360.);
        for (actual, (_, expected)) in store.config().mmd.motion_paths().iter().zip(paths) {
            assert_eq!(*actual, Path::new(expected));
        }
        let before = fs::read(&path).unwrap();
        assert!(!store.edit(|doc| set_u64(doc, &["mmd", "fps"], 120)));
        assert_eq!(fs::read(&path).unwrap(), before);
        assert!(!store.edit(|doc| {
            set_u64(doc, &["mmd", "fps"], 30);
            set_f64(doc, &["mmd", "motion_bpm"], 0.);
        }));
        assert_eq!(fs::read(&path).unwrap(), before);
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
        assert!(store.edit(|doc| {
            set_string(doc, &["results_video"], r"D:\Videos\结算.mp4");
        }));
        assert_eq!(
            store.config().results_video,
            Path::new(r"D:\Videos\结算.mp4")
        );
        assert!(store.reload());
        assert_eq!(
            store.config().results_video,
            Path::new(r"D:\Videos\结算.mp4")
        );
        assert!(store.edit(|doc| set_string(doc, &["results_video"], "")));
        assert!(store.config().results_video.as_os_str().is_empty());

        fs::remove_file(path).unwrap();
        fs::remove_dir(dir).unwrap();
    }

    #[test]
    fn bilibili_modes_preserve_each_selection_and_surface_bindings() {
        use iidxbg::bilibili::Mode;
        let (dir, path) = fixture_path();
        let mut store = ConfigStore::load(&path).unwrap();
        assert!(store.edit(|doc| {
            set_string(doc, &["sources", "lift", "source"], "bilibili");
            set_string(
                doc,
                &["sources", "lift", "bilibili", "video"],
                "BV1Pqi4BzEkP",
            );
            set_string(
                doc,
                &["sources", "lift", "bilibili", "playlist"],
                "https://space.bilibili.com/123/lists/456?type=season",
            );
            set_string(doc, &["sources", "lift", "bilibili", "favourites"], "789");
            set_string(doc, &["sources", "lift", "bilibili", "room"], "24065");
            set_string(doc, &["sources", "sudden_plus", "bind"], "lift");
        }));
        for mode in [
            Mode::Video,
            Mode::Recommendations,
            Mode::Playlist,
            Mode::Favourites,
            Mode::Live,
        ] {
            assert!(store.edit(|doc| set_string(
                doc,
                &["sources", "lift", "bilibili", "mode"],
                mode.key()
            )));
            assert!(store.reload());
            let selected = store
                .config()
                .for_surface(Surface::SuddenPlus)
                .bilibili
                .selection;
            assert_eq!(selected.mode, mode);
            assert_eq!(selected.video, "BV1Pqi4BzEkP");
            assert_eq!(selected.favourites, "789");
            assert_eq!(selected.room, "24065");
        }
        fs::remove_file(path).unwrap();
        fs::remove_dir(dir).unwrap();
    }
}
#[cfg(test)]
mod artwork_tests {
    use super::*;
    #[test]
    fn assets_save_globally_without_users_and_survive_scope_switches() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, include_str!("../../tests/fixtures/default.toml")).unwrap();
        let mut store = ConfigStore::load(&path).unwrap();
        store.set_scope(Scope::Assets);
        assert!(store.config().bilibili.account.is_none());
        assert!(store.edit(|doc| {
            set_surface_enabled(doc, Surface::Title, true);
            set_string(doc, &["sources", "title", "media_path"], "title.png");
        }));
        let index = store.add_artwork("test.ifs", "tile").unwrap();
        assert!(store.edit_artwork(index, |doc| {
            set_surface_enabled(doc, Surface::Transition, true);
            set_string(doc, &["sources", "transition", "media_path"], "shared.png");
        }));
        assert!(store.reload());
        assert_eq!(store.scope(), &Scope::Assets);
        let art = store.config().artwork.clone();
        for name in ["Alice", "Bob"] {
            let id = store.create_user(name).unwrap();
            store.set_scope(Scope::User(id));
            assert_eq!(store.config().artwork, art);
            assert!(
                store
                    .document_config()
                    .users
                    .values()
                    .all(|u| u.settings.artwork.is_empty())
            );
        }
        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded.artwork, loaded.for_user(None).artwork);
        assert!(loaded.pre_login.unwrap().title.enabled);
    }

    #[test]
    fn legacy_artwork_document_migrates_only_on_save_and_preserves_comments() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let text = format!(
            "artwork = []\n{}\n[users.a]\nname='A' # keep name\n[[users.a.settings.artwork]]\narchive='test.ifs' # keep archive\ntexture='tile'\n[users.a.settings.artwork.source]\nsource='file'\nmedia_path='shared.png'\n[users.b]\nname='B'\n[users.b.settings]\nartwork=[{{archive='test.ifs',texture='tile',source={{source='file',media_path='shared.png'}}}}]\n",
            include_str!("../../tests/fixtures/default.toml")
        );
        fs::write(&path, &text).unwrap();
        let mut store = ConfigStore::load(&path).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), text);
        store.set_scope(Scope::Assets);
        assert_eq!(store.config().artwork.len(), 1);
        assert!(store.edit_artwork(0, |doc| set_surface_enabled(
            doc,
            Surface::Transition,
            false
        )));
        let saved = fs::read_to_string(&path).unwrap();
        assert!(saved.contains("# keep name"));
        assert!(saved.contains("# keep archive"));
        assert!(!saved.contains("settings.artwork"));
        assert!(store.reload());
        assert!(!store.config().artwork[0].enabled);
        assert_eq!(store.config().artwork.len(), 1);
    }

    #[test]
    fn named_editor_does_not_change_transition_or_discard_comments() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, include_str!("../../tests/fixtures/default.toml")).unwrap();
        let mut store = ConfigStore::load(&path).unwrap();
        let original = store.config().sources.transition.clone();
        let index = store.add_artwork("gmframe34.ifs", "lane_base").unwrap();
        assert!(store.edit_artwork(index, |doc| {
            set_value(doc, &["sources", "transition", "source"], "file".into());
            set_value(
                doc,
                &["sources", "transition", "media_path"],
                "chosen.mp4".into(),
            );
            set_value(doc, &["surfaces", "transition"], true.into());
        }));
        assert_eq!(store.config().sources.transition, original);
        assert!(store.config().artwork[index].enabled);
        assert_eq!(
            store.config().artwork[index].source.media_path,
            Some(PathBuf::from("chosen.mp4"))
        );
        assert_eq!(store.add_artwork("gmframe34.ifs", "lane_base"), Some(index));
        assert_eq!(store.config().artwork.len(), 1);
    }
}
