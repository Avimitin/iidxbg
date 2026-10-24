use super::*;

#[test]
fn binary_xml_separate_alignment_lanes_and_names() {
    // Synthetic fixture generated with kbinxml, no game data.
    let hex = "a042807f0000002c0104df4d39030198fe05019cfe0301a0fe1e01a4fe0105bb29aca82e04ce6caa2706ebbdeaa390fefefeff00000000240709000004010000ffffffff000000030000000800000004616263000000000400020006";
    let b: Vec<_> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect();
    let (root, _) = xml::parse(&b).unwrap();
    assert_eq!(root.name, "root");
    assert_eq!(root.child("a").unwrap().text, "7");
    assert_eq!(root.child("b").unwrap().text, "1025");
    assert_eq!(root.child("c").unwrap().text, "9");
    assert_eq!(root.child("d").unwrap().text, "-1 3 8");
    let image = root.child("image").unwrap();
    assert_eq!(image.attr("name").unwrap(), "abc");
    assert_eq!(image.child("uvrect").unwrap().text, "0 4 2 6");
    for len in 0..b.len() {
        assert!(xml::parse(&b[..len]).is_err());
    }
}

#[test]
fn mislabeled_utf8_metadata_recovers_name_hash_encoding() {
    let hex = "a042807f0000000c0104df4d392e04ce6caafeff0000000c00000007e4b8ade696870000";
    let b: Vec<_> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect();
    let (node, encoding) = xml::parse(&b).unwrap();
    assert_eq!(encoding, 0xa0);
    assert_eq!(node.attr("name").unwrap(), "中文");
}

#[test]
fn lz77_bounds_overlap_and_initial_zero_window() {
    assert_eq!(
        ifs::decompress(&[1, b'A', 0, 0x13, 0, 0], 7).unwrap(),
        b"AAAAAAA"
    );
    assert_eq!(ifs::decompress(&[0, 0, 0x30, 0, 0], 3).unwrap(), [0, 0, 0]);
    assert!(ifs::decompress(&[1, b'A', 0, 0x13, 0, 0], 2).is_err());
    assert!(ifs::decompress(&[0], 2).is_err());
    assert!(ifs::decompress(&[0, 0, 0], 3).is_err());
}

#[test]
fn texture_channels_and_uv_crop() {
    // Big-endian RGB565 red endpoints; all block indices select endpoint zero.
    let dxt = [0xf8, 0, 0xf8, 0, 0, 0, 0, 0];
    let image = ifs::texture_image(&dxt, "dxt1", false, &[0, 4, 0, 4], &[0, 4, 0, 4]).unwrap();
    assert!(image.pixels().all(|p| p.0 == [255, 0, 0, 255]));
    let dxt5 = [0, 255, 0, 0, 0, 0, 0, 0, 0xf8, 0, 0xf8, 0, 0, 0, 0, 0];
    let image = ifs::texture_image(&dxt5, "dxt5", false, &[0, 4, 0, 4], &[0, 4, 0, 4]).unwrap();
    assert!(image.pixels().all(|p| p.0 == [255, 0, 0, 255]));
    let image = ifs::texture_image(
        &[1, 2, 3, 4, 5, 6, 7, 8],
        "argb8888rev",
        false,
        &[10, 12, 20, 21],
        &[11, 12, 20, 21],
    )
    .unwrap();
    assert_eq!(image.as_raw(), &[7, 6, 5, 8]);
    let image = ifs::texture_image(
        &[0x12, 0x34],
        "argb4444",
        false,
        &[0, 1, 0, 1],
        &[0, 1, 0, 1],
    )
    .unwrap();
    assert_eq!(image.as_raw(), &[68, 17, 34, 51]);
    assert!(
        ifs::texture_image(&[0; 4], "argb8888rev", false, &[0, 1, 0, 1], &[0, 2, 0, 1]).is_err()
    );
}

#[test]
fn geometry_names_bounds_endianness_and_alpha() {
    for little in [false, true] {
        let mut b = vec![0u8; 98];
        b[..4].copy_from_slice(if little { b"D2EG" } else { b"GE2D" });
        for (offset, value) in [
            (12, 98u32),
            (32, 52),
            (48, 76),
            (52, 0),
            (56, 0),
            (60, 4f32.to_bits()),
            (64, 0),
            (68, 0),
            (72, 4f32.to_bits()),
            (88, 92),
        ] {
            b[offset..offset + 4].copy_from_slice(&if little {
                value.to_le_bytes()
            } else {
                value.to_be_bytes()
            });
        }
        for (offset, value) in [(20, 3u16), (28, 1), (80, 3), (92, 0), (94, 1), (96, 2)] {
            b[offset..offset + 2].copy_from_slice(&if little {
                value.to_le_bytes()
            } else {
                value.to_be_bytes()
            });
        }
        b[77] = 9;
        b[84..88].copy_from_slice(&[1, 2, 3, 128]);
        let (w, h, image) = shape::preview(&b, true).unwrap().unwrap();
        assert_eq!((w, h), (4, 4));
        assert_eq!(image.get_pixel(0, 0).0, [1, 2, 3, 128]);
        assert_eq!(image.get_pixel(255, 255).0, [0, 0, 0, 0]);
        b[77] = 3;
        assert!(shape::preview(&b, true).unwrap().is_none());
        b[77] = 9;
        b[94..96].copy_from_slice(&65535u16.to_be_bytes());
        assert!(shape::preview(&b, true).is_err());
    }
}

fn fixture_ifs() -> Vec<u8> {
    let manifest =
        b"<imgfs><tex><texturelist_Exml __type=\"3s32\">0 1 0</texturelist_Exml></tex></imgfs>";
    let mut b = vec![0; 20];
    b[..4].copy_from_slice(&0x6cad8f89u32.to_be_bytes());
    b[4..6].copy_from_slice(&1u16.to_be_bytes());
    b[6..8].copy_from_slice(&(!1u16).to_be_bytes());
    b[16..20].copy_from_slice(&(20u32 + manifest.len() as u32).to_be_bytes());
    b.extend_from_slice(manifest);
    b.push(42);
    b
}
#[test]
fn archive_spans_names_and_truncation() {
    let bytes = fixture_ifs();
    assert_eq!(
        ifs::Archive::read(&bytes)
            .unwrap()
            .get("tex/texturelist.xml")
            .unwrap(),
        [42]
    );
    for len in 0..bytes.len() {
        assert!(ifs::Archive::read(&bytes[..len]).is_err());
    }
}

#[test]
fn cancelled_scan_preserves_catalog_and_cache_is_separate() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("game/data/graphic");
    fs::create_dir_all(&root).unwrap();
    image::RgbaImage::from_pixel(3, 7, image::Rgba([10, 20, 30, 255]))
        .save(root.join("sample.png"))
        .unwrap();
    let cache = dir.path().join("cache");
    let cancel = AtomicBool::new(false);
    let catalog = scan(&root, &cache, &cancel, |_| {}).unwrap();
    let before = fs::read(&catalog).unwrap();
    let entries: Vec<Entry> = serde_json::from_slice(&before).unwrap();
    assert!(entries.iter().any(|e| e.archive == "@file"
        && e.texture == "sample.png"
        && e.width == 3
        && e.height == 7));
    cancel.store(true, Ordering::Relaxed);
    assert!(scan(&root, &cache, &cancel, |_| {}).is_err());
    assert_eq!(fs::read(&catalog).unwrap(), before);
    cancel.store(false, Ordering::Relaxed);
    assert!(
        scan(&root, &cache, &cancel, |status| {
            if status.starts_with("1/") {
                cancel.store(true, Ordering::Relaxed);
            }
        })
        .is_err()
    );
    assert_eq!(fs::read(&catalog).unwrap(), before);
    assert!(scan(&root, &root.join("cache"), &AtomicBool::new(false), |_| {}).is_err());
    assert!(!root.join("cache").exists());
}

#[test]
#[ignore = "requires private local artwork inventory; never ship game data"]
fn private_inventory_scan() {
    let root = PathBuf::from(std::env::var_os("IIDXBG_SCAN_CORPUS").expect("IIDXBG_SCAN_CORPUS"));
    let output = PathBuf::from(std::env::var_os("IIDXBG_SCAN_OUTPUT").expect("IIDXBG_SCAN_OUTPUT"));
    let catalog = scan(&root, &output, &AtomicBool::new(false), |s| {
        eprintln!("{s}")
    })
    .unwrap();
    println!("CATALOG={}", catalog.display());
}
