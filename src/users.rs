//! User identity is independent of card identity. No unbound-card inheritance.
use crate::{Error, Result, config::*};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Module {
    pub enabled: bool,
    pub source: SourceOverride,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct PreLogin {
    pub title: Module,
    pub login: Module,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct User {
    pub name: String,
    #[serde(default)]
    pub cards: Vec<String>,
    #[serde(default)]
    pub settings: UserSettings,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct UserSettings {
    #[serde(deserialize_with = "user_surfaces")]
    pub surfaces: Surfaces,
    pub sources: Sources,
    pub selection: SelectionOptions,
    pub mmd: crate::mmd::Settings,
    /// Accepted for migration from early multi-user builds; promoted to global artwork.
    pub artwork: Vec<Artwork>,
    pub results_video: PathBuf,
}

fn user_surfaces<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Surfaces, D::Error> {
    let mut fields = serde_json::Map::<String, serde_json::Value>::deserialize(deserializer)?;
    fields
        .entry("results")
        .or_insert(serde_json::Value::Bool(false));
    serde_json::from_value(serde_json::Value::Object(fields)).map_err(serde::de::Error::custom)
}

impl Default for UserSettings {
    fn default() -> Self {
        let surfaces = Surfaces {
            results: false,
            ..Default::default()
        };
        Self {
            surfaces,
            sources: Sources::default(),
            selection: SelectionOptions::default(),
            mmd: crate::mmd::Settings::default(),
            artwork: Vec::new(),
            results_video: PathBuf::new(),
        }
    }
}

pub fn is_pre_login(surface: Surface) -> bool {
    matches!(surface, Surface::Title | Surface::Login)
}

/// Canonical card UID as displayed by Spice; never a refid or DJ name.
pub fn normalize_card(value: &str) -> Result<String> {
    let value = value.trim();
    if value.len() != 16
        || !value.bytes().all(|b| b.is_ascii_hexdigit())
        || value.bytes().all(|b| b == b'0')
        || value.eq_ignore_ascii_case("FFFFFFFFFFFFFFFF")
    {
        return Err(Error::Config(
            "card ID must be 16 hexadecimal digits and nonzero",
        ));
    }
    Ok(value.to_ascii_uppercase())
}

impl UserSettings {
    /// Explicit migration only: old global switches never select an unknown user.
    pub fn from_legacy(config: &Config) -> Self {
        let mut settings = Self {
            surfaces: config.surfaces.clone(),
            sources: config.sources.clone(),
            selection: config.selection.clone(),
            mmd: config.mmd.clone(),
            artwork: Vec::new(),
            results_video: config.results_video.clone(),
        };
        if !config.enabled {
            settings = Self::default();
        }
        settings.surfaces.title = false;
        settings.surfaces.login = false;
        settings.sources.title = SourceOverride::default();
        settings.sources.login = SourceOverride::default();
        settings
    }

    pub fn apply(&self, base: &Config) -> Config {
        let mut value = base.clone();
        value.asset_defaults = Some(Box::new(base.for_assets()));
        value.bilibili.account = None;
        value.users.clear();
        value.pre_login = None;
        value.surfaces = self.surfaces.clone();
        value.sources = self.sources.clone();
        value.selection = self.selection.clone();
        value.mmd = self.mmd.clone();
        value.artwork = base.artwork.clone();
        value.results_video = self.results_video.clone();
        value
    }
}

impl Config {
    /// Direct resource replacements belong to the installation, including guests.
    pub fn for_assets(&self) -> Self {
        let mut config = self.asset_defaults.as_deref().unwrap_or(self).clone();
        config.asset_defaults = None;
        config.users.clear();
        config.bilibili.account = None;
        let pre = config.pre_login_settings();
        config.pre_login = None;
        config.surfaces.title = pre.title.enabled;
        config.surfaces.login = pre.login.enabled;
        config.sources.title = pre.title.source;
        config.sources.login = pre.login.source;
        config
    }

    pub(crate) fn promote_shared_artwork(&mut self) -> Result<()> {
        for user in self.users.values_mut() {
            for art in std::mem::take(&mut user.settings.artwork) {
                match self
                    .artwork
                    .iter()
                    .find(|a| a.archive == art.archive && a.texture == art.texture)
                {
                    Some(existing) if existing != &art => {
                        return Err(Error::Config(
                            "conflicting artwork replacements; keep one shared entry in top-level artwork",
                        ));
                    }
                    Some(_) => {}
                    None => self.artwork.push(art),
                }
            }
        }
        Ok(())
    }

    pub fn pre_login_settings(&self) -> PreLogin {
        self.pre_login.clone().unwrap_or_else(|| PreLogin {
            title: Module {
                enabled: self.surface_enabled(Surface::Title),
                source: self.sources.title.clone(),
            },
            login: Module {
                enabled: self.surface_enabled(Surface::Login),
                source: self.sources.login.clone(),
            },
        })
    }

    pub fn user_for_card(&self, card: &str) -> Option<&str> {
        let card = normalize_card(card).ok()?;
        self.users
            .iter()
            .find(|(_, u)| u.cards.iter().any(|c| c.trim().eq_ignore_ascii_case(&card)))
            .map(|(id, _)| id.as_str())
    }

    /// Shared-screen policy: every active participant must map to the same user.
    /// Guest, unbound and conflicting users all get stock session modules.
    pub fn user_for_session(
        &self,
        cards: &[Option<String>; 2],
        joined: &[bool; 2],
    ) -> Option<&str> {
        let mut selected = None;
        for side in 0..2 {
            if !joined[side] {
                continue;
            }
            let id = self.user_for_card(cards[side].as_deref()?)?;
            if selected.is_some_and(|previous| previous != id) {
                return None;
            }
            selected = Some(id);
        }
        selected
    }

    pub fn for_user(&self, user: Option<&str>) -> Self {
        let user = user.filter(|id| self.users.contains_key(*id));
        let mut config = user
            .and_then(|id| self.users.get(id))
            .map(|u| &u.settings)
            .cloned()
            .unwrap_or_default()
            .apply(self);
        if !self.config_path.as_os_str().is_empty() {
            config.bilibili.account =
                user.map(|id| crate::bilibili::auth::AccountKey::new(&self.config_path, id));
        }
        let pre = self.pre_login_settings();
        config.surfaces.title = pre.title.enabled;
        config.surfaces.login = pre.login.enabled;
        config.sources.title = pre.title.source;
        config.sources.login = pre.login.source;
        config
    }
}

pub(crate) fn validate(config: &Config) -> Result<()> {
    if config.users.len() > 128 {
        return Err(Error::Config("at most 128 users"));
    }
    let mut cards = HashSet::new();
    for (id, user) in &config.users {
        if id.is_empty()
            || id.len() > 64
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Err(Error::Config(
                "user ID must contain 1..64 letters, digits, underscores or hyphens",
            ));
        }
        if user.name.trim().is_empty()
            || user.name.len() > 128
            || user.name.chars().any(char::is_control)
        {
            return Err(Error::Config(
                "user name must contain 1..128 bytes without control characters",
            ));
        }
        if user.cards.len() > 64 {
            return Err(Error::Config("at most 64 cards per user"));
        }
        for card in &user.cards {
            if !cards.insert(normalize_card(card)?) {
                return Err(Error::Config("card ID is already bound to a user"));
            }
        }
        let s = &user.settings;
        if s.surfaces.title
            || s.surfaces.login
            || !s.sources.title.is_empty()
            || !s.sources.login.is_empty()
        {
            return Err(Error::Config(
                "title and login belong in pre_login, not user settings",
            ));
        }
        let effective = s.apply(config);
        for surface in Surface::ALL.into_iter().filter(|s| !is_pre_login(*s)) {
            if effective
                .source_override(surface)
                .bind
                .is_some_and(is_pre_login)
            {
                return Err(Error::Config(
                    "user modules cannot bind to pre-login modules",
                ));
            }
        }
        if s.artwork
            .iter()
            .any(|a| a.source.bind.is_some_and(is_pre_login))
        {
            return Err(Error::Config(
                "user artwork cannot bind to pre-login modules",
            ));
        }
        effective.validate()?;
    }
    if config.pre_login.is_some() {
        let pre = config.pre_login_settings();
        for module in [&pre.title, &pre.login] {
            if module.source.bind.is_some_and(|s| !is_pre_login(s)) {
                return Err(Error::Config(
                    "pre-login modules cannot bind to user modules",
                ));
            }
        }
        config.for_user(None).validate()?;
    }
    Ok(())
}

pub(crate) fn resolve_paths(config: &mut Config, root: &Path) {
    let users = std::mem::take(&mut config.users);
    for (id, mut user) in users {
        let mut resolved = user.settings.apply(config);
        resolved.resolve_paths(root);
        // Preserve character/model paths: their loaders resolve against root themselves.
        user.settings.sources = resolved.sources;
        user.settings.results_video = resolved.results_video;
        config.users.insert(id, user);
    }
    if let Some(pre) = &mut config.pre_login {
        for source in [&mut pre.title.source, &mut pre.login.source] {
            for path in [
                &mut source.media_path,
                &mut source.local_directory,
                &mut source.moebooru.save_directory,
            ]
            .into_iter()
            .flatten()
            {
                if !path.as_os_str().is_empty() && path.is_relative() {
                    *path = root.join(&*path);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config() -> Config {
        Config::parse(include_str!("../tests/fixtures/default.toml")).unwrap()
    }
    fn user(name: &str, cards: &[&str]) -> User {
        User {
            name: name.into(),
            cards: cards.iter().map(|c| c.to_string()).collect(),
            ..Default::default()
        }
    }
    #[test]
    fn direct_replacements_use_shared_sources_for_every_user_and_guest() {
        let mut c = Config::parse(&format!(
            "{}\n[[artwork]]\narchive='test.ifs'\ntexture='tile'\n[artwork.source]\nbind='results'\nsource='file'\nmedia_path='inactive.png'\n",
            include_str!("../tests/fixtures/default.toml")
        )).unwrap();
        c.enabled = true;
        c.artwork[0].enabled = true;
        c.surfaces.results = false; // A bound source's switch is independent.
        c.config_path = "cabinet.toml".into();
        c.sources.results.source = Some(Source::File);
        c.sources.results.media_path = Some("shared.png".into());
        for id in ["alice", "bob"] {
            let mut u = user(id, &[]);
            u.settings.surfaces.results = true;
            u.settings.sources.results.source = Some(Source::File);
            u.settings.sources.results.media_path = Some(format!("{id}.png").into());
            c.users.insert(id.into(), u);
        }
        let root = std::env::temp_dir().join("iidxbg-shared-artwork-test");
        c.resolve_paths(&root);
        for id in [Some("alice"), Some("bob"), None, Some("missing")] {
            let session = c.for_user(id);
            assert_eq!(session.artwork, c.artwork);
            let shared = session.for_artwork(&session.artwork[0]);
            assert_eq!(shared.media_path, root.join("shared.png"));
            assert!(shared.enabled);
            assert!(shared.bilibili.account.is_none());
            assert!(
                !serde_json::to_string(&session)
                    .unwrap()
                    .contains("asset_defaults")
            );
            if let Some(id) = id.filter(|id| *id != "missing") {
                // Generated module presets retain the selected user's source and account.
                let mut preset = session.artwork[0].clone();
                preset.scene_scope = Some(Surface::Results);
                let personal = session.for_artwork(&preset);
                assert_eq!(personal.media_path, root.join(format!("{id}.png")));
                assert_eq!(personal.bilibili.account, session.bilibili.account);
            }
        }
        c.pre_login = Some(PreLogin {
            title: Module {
                enabled: true,
                source: SourceOverride {
                    source: Some(Source::File),
                    media_path: Some("title.png".into()),
                    ..Default::default()
                },
            },
            ..Default::default()
        });
        c.artwork[0].source.bind = Some(Surface::Title);
        assert_eq!(
            c.for_user(None).for_artwork(&c.artwork[0]).media_path,
            PathBuf::from("title.png")
        );
    }

    #[test]
    fn legacy_user_artwork_is_promoted_deduplicated_and_conflicts_rejected() {
        let text = format!(
            "{}\n[users.a]\nname='A'\n[[users.a.settings.artwork]]\narchive='test.ifs'\ntexture='tile'\n[users.a.settings.artwork.source]\nsource='file'\nmedia_path='shared.png'\n[users.b]\nname='B'\n[[users.b.settings.artwork]]\narchive='test.ifs'\ntexture='tile'\n[users.b.settings.artwork.source]\nsource='file'\nmedia_path='shared.png'\n",
            include_str!("../tests/fixtures/default.toml")
        );
        let c = Config::parse(&text).unwrap();
        assert_eq!(c.artwork.len(), 1);
        assert!(c.users.values().all(|u| u.settings.artwork.is_empty()));
        assert_eq!(c.for_user(None).artwork, c.artwork);
        assert!(Config::parse(&text.replacen("shared.png", "different.png", 1)).is_err());
    }
    #[test]
    fn bilibili_identity_follows_user_but_never_pre_login_or_unknown_users() {
        let mut config = config();
        config.config_path = PathBuf::from("cabinet.toml");
        config.users.insert("alice".into(), user("Alice", &[]));
        config.users.insert("bob".into(), user("Bob", &[]));
        let alice = config.for_user(Some("alice"));
        let bob = config.for_user(Some("bob"));
        assert!(alice.bilibili.account.is_some());
        assert_ne!(alice.bilibili.account, bob.bilibili.account);
        for surface in Surface::ALL {
            let key = alice.for_surface(surface).bilibili.account;
            if is_pre_login(surface) {
                assert!(key.is_none());
            } else {
                assert_eq!(key, alice.bilibili.account);
            }
        }
        assert!(config.for_user(None).bilibili.account.is_none());
        assert!(config.for_user(Some("missing")).bilibili.account.is_none());
        config.users.get_mut("alice").unwrap().name = "Renamed".into();
        assert_eq!(
            config.for_user(Some("alice")).bilibili.account,
            alice.bilibili.account
        );
        config.config_path = PathBuf::from("other-cabinet.toml");
        assert_ne!(
            config.for_user(Some("alice")).bilibili.account,
            alice.bilibili.account
        );
        let saved = serde_json::to_string(&alice).unwrap();
        assert!(!saved.contains("config_path"));
        assert!(!saved.contains("\"account\""));
        assert!(
            serde_json::from_str::<Config>(&saved)
                .unwrap()
                .bilibili
                .account
                .is_none()
        );
    }
    #[test]
    fn multiple_cards_choose_one_user_and_unbound_never_inherits_legacy() {
        let mut config = config();
        config.selection.frame_chara = false;
        config.mmd.enabled = true;
        config.users.insert(
            "alice".into(),
            user("Alice", &["e004010000000001", "E004010000000002"]),
        );
        config
            .users
            .get_mut("alice")
            .unwrap()
            .settings
            .surfaces
            .song_select = true;
        for card in ["E004010000000001", " e004010000000002 "] {
            assert_eq!(config.user_for_card(card), Some("alice"));
        }
        assert!(
            config
                .for_user(Some("alice"))
                .surface_enabled(Surface::SongSelect)
        );
        let unbound = config.for_user(config.user_for_card("E004010000000003"));
        assert!(!unbound.hide_frame_chara());
        assert!(!unbound.uses_mmd());
        assert!(
            Surface::ALL
                .into_iter()
                .filter(|s| !is_pre_login(*s))
                .all(|s| !unbound.surface_enabled(s))
        );
    }
    #[test]
    fn two_players_must_agree_including_guests_and_unbound_cards() {
        let mut c = config();
        c.users.insert(
            "a".into(),
            user("A", &["E004010000000001", "E004010000000002"]),
        );
        c.users.insert("b".into(), user("B", &["E004010000000003"]));
        let cards = [
            Some("E004010000000001".into()),
            Some("E004010000000002".into()),
        ];
        assert_eq!(c.user_for_session(&cards, &[true, true]), Some("a"));
        assert_eq!(
            c.user_for_session(&[None, cards[1].clone()], &[false, true]),
            Some("a")
        );
        assert_eq!(
            c.user_for_session(&[cards[0].clone(), None], &[true, true]),
            None
        );
        assert_eq!(
            c.user_for_session(
                &[cards[0].clone(), Some("E004010000000003".into())],
                &[true, true]
            ),
            None
        );
        assert_eq!(c.user_for_session(&cards, &[false, false]), None);
    }
    #[test]
    fn reject_duplicates_invalid_ids_and_cross_scope_bindings() {
        let mut c = config();
        c.users.insert("a".into(), user("A", &["E004010000000001"]));
        c.users.insert("b".into(), user("B", &["e004010000000001"]));
        assert!(c.validate().is_err());
        c.users.remove("b");
        assert!(c.validate().is_ok());
        c.users.get_mut("a").unwrap().settings.sources.results.bind = Some(Surface::Title);
        assert!(c.validate().is_err());
        c.users.clear();
        c.pre_login = Some(PreLogin {
            title: Module {
                source: SourceOverride {
                    bind: Some(Surface::SongSelect),
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        });
        assert!(c.validate().is_err());
        for id in ["", "1234", "0000000000000000", "G004010000000001"] {
            assert!(normalize_card(id).is_err());
        }
    }
    #[test]
    fn partial_user_switches_default_off_and_paths_resolve() {
        let text = format!(
            "{}\n[users.alice]\nname='Alice'\ncards=['E004010000000001']\n[users.alice.settings.surfaces]\nsong_select=true\n[users.alice.settings.sources.song_select]\nsource='file'\nmedia_path='art/test.png'\n[pre_login.title]\nenabled=true\n[pre_login.title.source]\nsource='file'\nmedia_path='art/title.png'\n",
            include_str!("../tests/fixtures/default.toml")
        );
        let mut c = Config::parse(&text).unwrap();
        assert!(!c.users["alice"].settings.surfaces.results);
        let root = std::env::temp_dir().join("iidxbg-user-test");
        c.resolve_paths(&root);
        let a = c.for_user(Some("alice"));
        assert_eq!(
            a.for_surface(Surface::SongSelect).media_path,
            root.join("art/test.png")
        );
        assert_eq!(
            a.for_surface(Surface::Title).media_path,
            root.join("art/title.png")
        );
        assert!(a.surface_enabled(Surface::Title));
        assert!(!c.for_user(None).surface_enabled(Surface::SongSelect));
    }
}
