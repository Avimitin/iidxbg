//! IIDX 33 / 010-20260819.
//! Native ABI/lifetime evidence: docs/iidx33-analysis.md.
//! All addresses are RVAs; file hash and live guards remain mandatory.

use crate::profile::{Layout, Profile, Table, native_entries, offsets};

pub(super) fn profile() -> Profile {
    let (rvas, guards) = native_entries(&[
        // Function name, RVA, first 16 bytes of the verified native entry.
        (
            "background_update",
            0x91c120,
            "488bc4554154415541564157488da8c8",
        ),
        (
            "result_cleanup",
            0x92d970,
            "4055535657415441564157488d6c24d9",
        ),
        ("find_clip", 0x8322c0, "4885c974094885d20f85d214b1ff33c0"),
        ("set_archive", 0x340520, "48895c240848896c2410488974241848"),
        ("set_texture", 0x340440, "48895c24084889742410574883ec308b"),
        ("set_visible", 0x3407b0, "40534883ec208b990002000085db784b"),
        ("set_origin", 0x340a50, "488bc4534883ec60488968f0418bd848"),
        ("texture_wrap", 0xa61ca0, "48894c24085355565741564883ec7048"),
        (
            "texture_destroy",
            0xa61e30,
            "40534883ec5048c7442420feffffff48",
        ),
        ("get_device", 0x93bc30, "488b051939270a488b80e0000000c3cc"),
        (
            "result_delete",
            0x92add0,
            "48895c2408574883ec208bda488bf9e8",
        ),
        (
            "set_origin_pixels",
            0x3409d0,
            "40534883ec408b99000200000f297424",
        ),
        (
            "select_update",
            0x8ec1f0,
            "48895c240848896c2410488974241848",
        ),
        (
            "select_cleanup",
            0x8ebeb0,
            "488bc4565741564883ec7048c740a8fe",
        ),
        (
            "select_delete",
            0x8eabb0,
            "48895c2408574883ec20488bd98bfa48",
        ),
        ("movie_draw", 0x94f7c0, "4883ec4880791000743080792400742a"),
        ("cover_build", 0x90b5e0, "4885d20f84df01000048895c24184889"),
        ("cover_visual", 0x90a430, "405341544883ec680f297424404c8be1"),
        ("cover_delete", 0x90a170, "48895c2408574883ec20488bd933ff48"),
        ("graph_art", 0xa3c850, "4c8bdc574883ec70488b05c982650048"),
        ("graph_reset", 0xa3d870, "40574883ec3048c7442420feffffff48"),
        (
            "sprite_release",
            0x34c160,
            "40574883ec508b5108488bf985d20f84",
        ),
        (
            "sprite_manager",
            0x34a610,
            "488d05c94d090bc3cccccccccccccccc",
        ),
        (
            "sprite_create",
            0x34ab60,
            "48896c24104889742418574883ec2041",
        ),
        (
            "sprite_origin",
            0x34c450,
            "40534883ec20660f6ec2488bd98b8910",
        ),
        ("sprite_scale", 0x34c990, "48895c2410574883ec408b9910020000"),
        ("sprite_clip", 0x34c6a0, "4883ec38488b01448b520c448b4a0844"),
        (
            "sprite_position",
            0x34c300,
            "4883ec288b4908660f6ec2488d542430",
        ),
        ("sprite_alpha", 0x34c8a0, "8b49080f28c3f30f105c24280f28e2f3"),
        ("graph_marker", 0x8322c0, "4885c974094885d20f85d214b1ff33c0"),
        ("cover_init", 0x90adf0, "48895c241048896c2418488974242057"),
        (
            "graph_parent_reset",
            0xa45000,
            "488bc45541564157488da878fbffff48",
        ),
        (
            "graph_storage_release",
            0xa3bdc0,
            "40574883ec20488b01488bf9488b0848",
        ),
        (
            "layer_dimensions",
            0x343950,
            "4885d2743b48895c2408574883ec6049",
        ),
    ]);
    Profile {
        label: "IIDX 33 / 010-20260819".into(),
        sha256: "c61b6dcb8894062e56d60da8ca90053b27f129e1a8e8da5e54457aa42602397d".into(),
        image_size: 0xbb95000,
        rvas,
        guards,
        layout: Layout {
            scene_vtable: 0xda4800,
            scene_background: 256,
            background_archive: 272,
            background_movie: 296,
        },
        tables: vec![
            Table {
                rva: 0xda2188,
                slot: 2,
                target: 0x91c120,
            },
            Table {
                rva: 0xda4800,
                slot: 14,
                target: 0x92d970,
            },
            Table {
                rva: 0xda4800,
                slot: 7,
                target: 0x92add0,
            },
        ],
        surface_data: offsets(&[
            ("afp_group_get", 0xc91be0),
            ("afp_group_set", 0xc91be8),
            ("select_vtable", 0xd84788),
            ("selection_fade", 0x1),
            ("select_update_extended", 0x1),
            ("select_frame_chara", 0x110),
            ("layer_vtable", 0xdc86e0),
            ("afp_property_get", 0xc91bb0),
            ("afp_property_set", 0xc91bc8),
            ("select_frame_priority", 0xb4),
        ]),
    }
}
