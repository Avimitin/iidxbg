//! First-page artwork mappings for the inspected IIDX 34 resources.
//! Native adapters remain exact-build guarded. See docs/iidx34-scene-presets.md.
use crate::config::{Artwork, Config, SourceOverride, Surface};

pub(crate) fn artwork(config: &Config) -> Vec<Artwork> {
    let mut entries = config.artwork.clone();
    let mut add = |surface, archive: &str, texture: &str, size, bottom_crop| {
        // Explicit catalog choices take precedence, including disabled choices.
        if entries
            .iter()
            .any(|a| a.scene_scope.is_none() && a.archive == archive && a.texture == texture)
        {
            return;
        }
        entries.push(Artwork {
            archive: archive.into(),
            texture: texture.into(),
            native_size: Some(size),
            enabled: match surface {
                Surface::Title => config.surfaces.title,
                Surface::Login => config.surfaces.login,
                Surface::ModeSelect => config.surfaces.mode_select,
                Surface::GameplayBottom => config.surfaces.gameplay_bottom,
                Surface::Logout => config.surfaces.logout,
                _ => false,
            },
            source: SourceOverride {
                bind: Some(surface),
                ..Default::default()
            },
            native_mask: true,
            scene_scope: Some(surface),
            bottom_crop,
        });
    };
    add(
        Surface::Title,
        "0/title.ifs",
        "34title_bg",
        [1920, 1080],
        false,
    );
    for (name, size, crop) in [
        ("unit_main_sp_1p", [622, 1080], true),
        ("unit_main_dp_1p", [620, 1080], true),
        ("unit_sp_01", [332, 166], false),
        ("unit_score", [186, 68], false),
        ("unit_spd", [160, 72], false),
        ("unit_groove_gauge", [520, 110], false),
        ("unit_dp_bottom", [1920, 186], false),
        ("unit_dp_bottom2", [504, 186], false),
        ("unit_dp_bottom3", [404, 178], false),
        ("unit_dp_bottom4", [426, 156], false),
    ] {
        add(Surface::GameplayBottom, "gmframe34.ifs", name, size, crop);
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn presets_are_opt_in_and_catalog_choices_take_precedence() {
        let mut config = Config::parse(include_str!("../tests/fixtures/default.toml")).unwrap();
        assert!(artwork(&config).iter().all(|a| !a.enabled));
        config.surfaces.title = true;
        let generated = artwork(&config);
        let mut explicit = generated
            .iter()
            .find(|a| a.scene_scope == Some(Surface::Title))
            .unwrap()
            .clone();
        explicit.scene_scope = None;
        explicit.enabled = false;
        config.artwork.push(explicit);
        let generated = artwork(&config);
        let title: Vec<_> = generated
            .iter()
            .filter(|a| a.archive == "0/title.ifs")
            .collect();
        assert_eq!(title.len(), 1);
        assert!(!title[0].enabled);
    }
}
