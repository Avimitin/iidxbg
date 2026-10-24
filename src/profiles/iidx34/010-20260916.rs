//! IIDX 34 / 010-20260916; legacy display label: 20260917.
//! Native ABI/lifetime evidence: docs/iidx34-analysis.md.
//! All addresses are RVAs; file hash and live guards remain mandatory.

use crate::profile::{Layout, Profile, Table, native_entries, offsets};

pub(super) fn profile() -> Profile {
    let (rvas, guards) = native_entries(&[
        // Function name, RVA, first 16 bytes of the verified native entry.
        (
            "background_update",
            0x906e70,
            "488bc4554154415541564157488da8c8",
        ),
        (
            "result_cleanup",
            0x915580,
            "4055535657415441564157488d6c24d9",
        ),
        ("find_clip", 0x81e9f0, "4885c974094885d20f85a2e7b0ff33c0"),
        ("set_archive", 0x329f30, "48895c240848896c2410488974241848"),
        ("set_texture", 0x329e50, "48895c24084889742410574883ec308b"),
        ("set_visible", 0x32a1c0, "40534883ec208b990002000085db784b"),
        ("set_origin", 0x32a460, "488bc4534883ec60488968f0418bd848"),
        ("texture_wrap", 0xa4f8a0, "48894c24085355565741564883ec7048"),
        (
            "texture_destroy",
            0xa4fa30,
            "40534883ec5048c7442420feffffff48",
        ),
        ("get_device", 0x922a50, "488b050909bc0a488b80e0000000c3cc"),
        (
            "result_delete",
            0x912ba0,
            "48895c2408574883ec208bda488bf9e8",
        ),
        (
            "set_origin_pixels",
            0x32a3e0,
            "40534883ec408b99000200000f297424",
        ),
        (
            "select_update",
            0x8d8250,
            "48895c240848896c2410488974241848",
        ),
        (
            "select_cleanup",
            0x8d7f60,
            "488bc4565741564883ec6048c740b8fe",
        ),
        (
            "select_delete",
            0x8d7340,
            "40534883ec2048896c2430488bd94889",
        ),
        ("movie_draw", 0x936270, "4883ec4880791000743080792400742a"),
        ("cover_build", 0x8f6350, "4885d20f84df01000048895c24184889"),
        ("cover_visual", 0x8f50e0, "4053574883ec580f29742440488bf90f"),
        ("cover_delete", 0x8f4de0, "48895c2408574883ec20488bd933ff48"),
        ("graph_art", 0xa1fcf0, "4c8bdc574883ec70488b05b961630048"),
        ("graph_reset", 0xa20d10, "40574883ec3048c7442420feffffff48"),
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
        ("graph_marker", 0x81e9f0, "4885c974094885d20f85a2e7b0ff33c0"),
        ("cover_init", 0x8f5ae0, "48895c241048896c2418488974242057"),
        (
            "graph_parent_reset",
            0xa284a0,
            "488bc45541564157488da878fbffff48",
        ),
        (
            "graph_storage_release",
            0xa1f260,
            "40574883ec20488b01488bf9488b0848",
        ),
        (
            "layer_dimensions",
            0x32d350,
            "4885d2743b48895c2408574883ec6049",
        ),
    ]);
    Profile {
        artwork_library: None,
        label: "IIDX 34 / 20260917".into(),
        sha256: "3755587f44785effe5008f74cfe7dccbdf423ad09feead48e00a90f78a42fe83".into(),
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
                target: 0x906e70,
            },
            Table {
                rva: 0xd72950,
                slot: 17,
                target: 0x915580,
            },
            Table {
                rva: 0xd72950,
                slot: 7,
                target: 0x912ba0,
            },
        ],
        surface_data: offsets(&[
            ("afp_group_get", 0xc66bf8),
            ("afp_group_set", 0xc66bf0),
            ("select_vtable", 0xd52508),
            ("selection_fade", 0x0),
            ("select_update_extended", 0x0),
            ("select_frame_chara", 0x118),
            ("layer_vtable", 0xd968b0),
            ("afp_property_get", 0xc66c28),
            ("afp_property_set", 0xc66c10),
            ("select_frame_priority", 0xbc),
        ]),
    }
}
