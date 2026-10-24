//! IIDX 34 / 010-20260917.
//! Native ABI/lifetime evidence: docs/iidx34-010-0917-analysis.md.
//! All addresses are RVAs; file hash and live guards remain mandatory.

use crate::profile::{Layout, LibraryProfile, Profile, Table, native_entries, offsets};

pub(super) fn profile() -> Profile {
    let (rvas, guards) = native_entries(&[
        (
            "artwork_login_setup",
            0x8a06a0,
            "4055415441574883ec404c8be24c8bf9",
        ),
        (
            "artwork_logout_setup",
            0x8a1fd0,
            "4055415641574883ec70488b05d73e7b",
        ),
        (
            "artwork_file_load",
            0x6abd90,
            "405356574883ec6048c7442428feffff",
        ),
        (
            "artwork_direct_release",
            0x344c30,
            "40534883ec208bd9e863eeffff488bc8",
        ),
        (
            "artwork_direct_bind",
            0x338570,
            "4c8bdc49895b1049896b184989732041",
        ),
        (
            "artwork_archive_open",
            0x33ac50,
            "48895c241048896c2418488974242057",
        ),
        (
            "artwork_archive_load",
            0x33af10,
            "48895c241048896c2418488974242057",
        ),
        (
            "artwork_archive_release",
            0x33b500,
            "48895c2408574883ec208b810c050000",
        ),
        (
            "artwork_frame",
            0x328b40,
            "488bc4565741564883ec7048c74098fe",
        ),
        (
            "native_texture",
            0x343e10,
            "48895c2408574883ec408bf9e87ffcff",
        ),
        (
            "effective_texture",
            0x343f30,
            "40574883ec3048c7442420feffffff48",
        ),
        (
            "replace_texture",
            0x344950,
            "405355565741564883ec4048c7442430",
        ),
        (
            "release_texture",
            0x344100,
            "48895c2408574883ec408bf9e8fffcff",
        ),
        (
            "artwork_layer_create",
            0x32da80,
            "48895c241048896c2418488974242057",
        ),
        (
            "artwork_dispatch",
            0x34fed0,
            "48895c240848896c2410488974241857",
        ),
        ("artwork_draw", 0x338900, "488bc441554883ec50f30f100db73cb1"),
        (
            "note_batch_flush",
            0x342730,
            "48895c241855415641574883ec504889",
        ),
        // Function name, RVA, first 16 bytes of the verified native entry.
        (
            "select_frame_build",
            0x5cf1f0,
            "488bc4554154415541564157488da818",
        ),
        (
            "background_update",
            0x906eb0,
            "488bc4554154415541564157488da8c8",
        ),
        (
            "result_cleanup",
            0x9155c0,
            "4055535657415441564157488d6c24d9",
        ),
        ("find_clip", 0x81ea30, "4885c974094885d20f8562e7b0ff33c0"),
        ("set_archive", 0x329f30, "48895c240848896c2410488974241848"),
        ("set_texture", 0x329e50, "48895c24084889742410574883ec308b"),
        ("set_visible", 0x32a1c0, "40534883ec208b990002000085db784b"),
        ("set_origin", 0x32a460, "488bc4534883ec60488968f0418bd848"),
        ("texture_wrap", 0xa4f8e0, "48894c24085355565741564883ec7048"),
        (
            "texture_destroy",
            0xa4fa70,
            "40534883ec5048c7442420feffffff48",
        ),
        ("get_device", 0x922a90, "488b05c908bc0a488b80e0000000c3cc"),
        (
            "result_delete",
            0x912be0,
            "48895c2408574883ec208bda488bf9e8",
        ),
        (
            "set_origin_pixels",
            0x32a3e0,
            "40534883ec408b99000200000f297424",
        ),
        (
            "select_update",
            0x8d8290,
            "48895c240848896c2410488974241848",
        ),
        (
            "select_cleanup",
            0x8d7fa0,
            "488bc4565741564883ec6048c740b8fe",
        ),
        (
            "select_delete",
            0x8d7380,
            "40534883ec2048896c2430488bd94889",
        ),
        ("movie_draw", 0x9362b0, "4883ec4880791000743080792400742a"),
        (
            "artwork_movie_texture",
            0xa4fd50,
            "40534883ec3048c7442420feffffff48",
        ),
        ("cover_build", 0x8f6390, "4885d20f84df01000048895c24184889"),
        ("cover_visual", 0x8f5120, "4053574883ec580f29742440488bf90f"),
        ("cover_delete", 0x8f4e20, "48895c2408574883ec20488bd933ff48"),
        ("graph_art", 0xa1fd30, "4c8bdc574883ec70488b057961630048"),
        ("graph_reset", 0xa20d50, "40574883ec3048c7442420feffffff48"),
        (
            "sprite_release",
            0x3363e0,
            "40574883ec508b5108488bf985d20f84",
        ),
        (
            "sprite_manager",
            0x334890,
            "488d0599ebad0bc3cccccccccccccccc",
        ),
        (
            "sprite_create",
            0x334de0,
            "48896c24104889742418574883ec2041",
        ),
        (
            "sprite_origin",
            0x3366d0,
            "40534883ec20660f6ec2488bd98b8910",
        ),
        ("sprite_scale", 0x336c10, "48895c2410574883ec408b9910020000"),
        ("sprite_clip", 0x336920, "4883ec38488b01448b520c448b4a0844"),
        (
            "sprite_position",
            0x336580,
            "4883ec288b4908660f6ec2488d542430",
        ),
        ("sprite_alpha", 0x336b20, "8b49080f28c3f30f105c24280f28e2f3"),
        ("graph_marker", 0x81ea30, "4885c974094885d20f8562e7b0ff33c0"),
        ("cover_init", 0x8f5b20, "48895c241048896c2418488974242057"),
        (
            "graph_parent_reset",
            0xa284e0,
            "488bc45541564157488da878fbffff48",
        ),
        (
            "graph_storage_release",
            0xa1f2a0,
            "40574883ec20488b01488bf9488b0848",
        ),
        (
            "layer_dimensions",
            0x32d350,
            "4885d2743b48895c2408574883ec6049",
        ),
        (
            "premium_area_ctor",
            0x6447a0,
            "4c894c242041564883ec4048c7442430",
        ),
        (
            "concentration_apply",
            0x6375c0,
            "40565741564883ec4048c7442420feff",
        ),
        (
            "concentration_show",
            0x6377f0,
            "488bc45541544156488bec4881ec8000",
        ),
        (
            "concentration_layer",
            0x638420,
            "4883ec284c8bd248b825232284e49cf2",
        ),
        (
            "box_fill_update",
            0x339db0,
            "448b49084585c974218b4118488d5128",
        ),
        (
            "box_outline_update",
            0x3397f0,
            "448b49084585c974218b4118488d5128",
        ),
        ("line_update", 0x33bb10, "448b49084585c974218b4118488d5128"),
    ]);
    Profile {
        artwork_library: Some({
            let (rvas, guards) = native_entries(&[
                ("shape_id", 0x119b0, "40534883ec20488bda8bd1488d0d3e3a"),
                ("shape_info", 0x10550, "40534883ec20488bd9f7c20000007875"),
                (
                    "artwork_shape_draw",
                    0x1a420,
                    "405556488dac2458f5ffff4881eca80b",
                ),
            ]);
            LibraryProfile {
                name: "afp-utils.dll".into(),
                sha256: "b74a538ee13603d8d9d695a6c14b296ac532685528923b30e125d0197d706322".into(),
                image_size: 0x42000,
                rvas,
                guards,
            }
        }),
        label: "IIDX 34 / 010-0917".into(),
        sha256: "c98e183ec81ecb9e1cb13c237f9c9cb0b3f65b6fdaff6fd5e6a526008c546534".into(),
        image_size: 0xc5ff000,
        rvas,
        guards,
        layout: Layout {
            scene_vtable: 0xd72950,
            scene_background: 264,
            background_archive: 272,
            background_movie: 296,
        },
        tables: vec![
            Table {
                rva: 0xd70598,
                slot: 2,
                target: 0x906eb0,
            },
            Table {
                rva: 0xd72950,
                slot: 17,
                target: 0x9155c0,
            },
            Table {
                rva: 0xd72950,
                slot: 7,
                target: 0x912be0,
            },
        ],
        surface_data: offsets(&[
            ("artwork_texture_info", 0xc66da0),
            ("artwork_native_id", 0xc66d78),
            ("note_batches", 0xc38f720),
            ("afp_group_get", 0xc66bf8),
            ("afp_group_set", 0xc66bf0),
            ("select_vtable", 0xd52508),
            ("selection_fade", 0x0),
            ("select_update_extended", 0x1),
            ("select_background", 0xf8),
            ("select_background_priority", 0xb0),
            ("select_frame_chara", 0x118),
            ("layer_vtable", 0xd968b0),
            ("afp_property_get", 0xc66c28),
            ("afp_property_set", 0xc66c10),
            ("select_frame_priority", 0xbc),
            ("premium_area_base", 0x10),
            ("premium_area_custom", 0x18),
            ("concentration_priority", 0x6),
            ("clip_pool_count", 0xc500a80),
        ]),
    }
}
