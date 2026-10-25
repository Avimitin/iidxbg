use super::*;

// Original procedural fixture: a coloured triangle with a head bone and smile.
// No third-party model, motion or game asset is stored in the repository.
fn string(bytes: &mut Vec<u8>, s: &str) {
    bytes.extend((s.len() as i32).to_le_bytes());
    bytes.extend(s.as_bytes());
}
fn floats(bytes: &mut Vec<u8>, values: &[f32]) {
    for v in values {
        bytes.extend(v.to_le_bytes());
    }
}
fn pmx() -> Vec<u8> {
    let mut b = b"PMX ".to_vec();
    floats(&mut b, &[2.]);
    b.extend([8, 1, 0, 1, 1, 1, 1, 1, 1]);
    for s in [
        "iidxbg test triangle",
        "",
        "Original procedural test fixture",
        "",
    ] {
        string(&mut b, s);
    }
    b.extend(3i32.to_le_bytes());
    for (p, uv) in [
        ([-1., 0., 0.], [0., 0.]),
        ([1., 0., 0.], [1., 0.]),
        ([0., 2., 0.], [0.5, 1.]),
    ] {
        floats(&mut b, &p);
        floats(&mut b, &[0., 0., -1.]);
        floats(&mut b, &uv);
        b.extend([0, 0]);
        floats(&mut b, &[1.]);
    }
    b.extend(3i32.to_le_bytes());
    b.extend([0, 2, 1]);
    b.extend(0i32.to_le_bytes());
    b.extend(1i32.to_le_bytes());
    string(&mut b, "face");
    string(&mut b, "");
    floats(&mut b, &[0.3, 0.7, 1., 1., 0., 0., 0., 1., 0.5, 0.5, 0.5]);
    b.push(1);
    floats(&mut b, &[0., 0., 0., 1., 1.]);
    b.extend([255, 255, 0, 1, 0]);
    string(&mut b, "");
    b.extend(3i32.to_le_bytes());
    b.extend(1i32.to_le_bytes());
    string(&mut b, "head");
    string(&mut b, "");
    floats(&mut b, &[0., 0., 0.]);
    b.push(255);
    b.extend(0i32.to_le_bytes());
    b.extend(30u16.to_le_bytes());
    floats(&mut b, &[0., 1., 0.]);
    b.extend(1i32.to_le_bytes());
    string(&mut b, "smile");
    string(&mut b, "");
    b.extend([4, 1]);
    b.extend(1i32.to_le_bytes());
    b.push(2);
    floats(&mut b, &[0.4, 0.2, 0.]);
    b.extend([0; 12]);
    b
}
fn fixed(b: &mut Vec<u8>, s: &[u8], n: usize) {
    b.extend(s);
    b.resize(b.len() + n - s.len(), 0);
}
fn vmd() -> Vec<u8> {
    let mut b = Vec::new();
    fixed(&mut b, b"Vocaloid Motion Data 0002", 30);
    fixed(&mut b, b"test", 20);
    b.extend(2u32.to_le_bytes());
    for (frame, x) in [(0u32, 0.), (30, 0.5)] {
        fixed(&mut b, b"head", 15);
        b.extend(frame.to_le_bytes());
        floats(&mut b, &[x, 0., 0., 0., 0., 0., 1.]);
        // Linear Bezier controls (x==y for each channel).
        b.extend([20u8; 64]);
    }
    b.extend(2u32.to_le_bytes());
    for (frame, w) in [(0u32, 0.), (30, 1.)] {
        fixed(&mut b, b"smile", 15);
        b.extend(frame.to_le_bytes());
        floats(&mut b, &[w]);
    }
    b.extend([0; 16]);
    b
}
fn fixture() -> (tempfile::TempDir, Arc<Model>) {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("model.pmx"), pmx()).unwrap();
    fs::write(dir.path().join("dance.vmd"), vmd()).unwrap();
    let settings = Settings {
        model_path: "model.pmx".into(),
        motion_path: "dance.vmd".into(),
        look_bone: "head".into(),
        happy_morph: "smile".into(),
        blink_morph: String::new(),
        ..Default::default()
    };
    let model = load(&settings, dir.path()).unwrap();
    (dir, model)
}
#[test]
fn import_loop_motion_and_expression_are_distinct() {
    let (_dir, model) = fixture();
    let start = model.evaluate(0., [0., 0.], 0.).unwrap();
    let moved = model.evaluate(0.5, [0., 0.], 0.).unwrap();
    assert!((moved.vertices[0].position[0] - start.vertices[0].position[0]).abs() > 10.);
    let smile = model.evaluate(0., [0., 0.], 1.).unwrap();
    assert_eq!(start.vertices[0].position, smile.vertices[0].position);
    assert_ne!(start.vertices[2].position, smile.vertices[2].position);
    let turned = model.evaluate(0., [0.3, 0.2], 0.).unwrap();
    assert_ne!(turned.vertices[2].position, start.vertices[2].position);
    let looped = model.evaluate(31. / 30., [0., 0.], 0.).unwrap();
    assert_eq!(looped.vertices[0].position, start.vertices[0].position);
}
#[test]
fn malformed_models_and_outside_textures_fail() {
    let dir = tempfile::tempdir().unwrap();
    let settings = Settings {
        model_path: "bad.pmx".into(),
        ..Default::default()
    };
    for bytes in [vec![], b"PMX bad".to_vec(), pmx()[..90].to_vec()] {
        fs::write(dir.path().join("bad.pmx"), bytes).unwrap();
        assert!(load(&settings, dir.path()).is_err());
    }
    assert!(texture_path(dir.path(), "../secret.png").is_err());
    assert!(texture_path(dir.path(), "C:\\secret.png").is_err());
}
#[test]
fn session_first_pose_is_ready_without_animation_or_game_access() {
    let (_dir, model) = fixture();
    let a = model.lease().unwrap();
    let b = model.lease().unwrap();
    assert!(Arc::ptr_eq(&a, &b));
    assert_eq!(a.frame().unwrap().vertices.len(), 3);
    let mailbox = a.mailbox.clone();
    drop(a);
    assert!(!mailbox.stop.load(Ordering::Acquire));
    drop(b);
    assert!(mailbox.stop.load(Ordering::Acquire));
}
#[test]
fn invalid_settings_are_rejected() {
    let mut s = Settings {
        fps: 120,
        ..Default::default()
    };
    assert!(s.validate().is_err());
    s.fps = 30;
    s.scale = f32::NAN;
    assert!(s.validate().is_err());
    for motion_bpm in [0., -1., f32::NAN, f32::INFINITY, 1001.] {
        assert!(
            Settings {
                motion_bpm,
                ..Default::default()
            }
            .validate()
            .is_err()
        );
    }
}

#[test]
fn idle_tempo_is_opt_in_bounded_and_preserves_phase_and_reaction_duration() {
    let (dir, base) = fixture();
    assert_eq!(idle_rate(&base.settings, 240), 1.);
    let model = load(
        &Settings {
            bpm_sync: true,
            motion_bpm: 120.,
            ..base.settings.clone()
        },
        dir.path(),
    )
    .unwrap();
    assert_eq!(idle_rate(&model.settings, 240), 2.);
    assert_eq!(idle_rate(&model.settings, 60), 0.5);
    assert_eq!(idle_rate(&model.settings, 0), 1.);
    assert_eq!(idle_rate(&model.settings, 100_001), 1.);
    assert_eq!(idle_rate(&model.settings, 1), 0.25);
    assert_eq!(idle_rate(&model.settings, 100_000), 4.);
    let mut clock = IdleClock::default();
    let normal = Sample {
        motion: Motion::Normal,
        seconds: 999.,
    };
    assert!((clock.sample(&model, 0.2, 120, normal).seconds - 0.2).abs() < 0.0001);
    assert!((clock.sample(&model, 0.1, 240, normal).seconds - 0.4).abs() < 0.0001);
    let reaction = Sample {
        motion: Motion::ScrollUp,
        seconds: 0.3,
    };
    assert_eq!(clock.sample(&model, 0.2, 60, reaction).seconds, 0.3);
    assert!((clock.sample(&model, 0.1, 60, normal).seconds - 0.55).abs() < 0.0001);
    assert!((clock.sample(&model, 0.1, 0, normal).seconds - 0.65).abs() < 0.0001);
}

#[test]
fn empty_look_bone_disables_tracking_instead_of_matching_empty_english_name() {
    let (dir, base) = fixture();
    let model = load(
        &Settings {
            look_bone: String::new(),
            ..base.settings.clone()
        },
        dir.path(),
    )
    .unwrap();
    assert!(model.head.is_none());
    assert_eq!(
        model.evaluate(0., [0., 0.], 0.).unwrap().vertices[2].position,
        model.evaluate(0., [-0.7, 0.1], 0.).unwrap().vertices[2].position
    );
}

#[test]
fn look_right_overrides_dance_head_yaw_and_compensates_parent() {
    let angle = 45f32.to_radians();
    let parent = Quat::from_rotation_y(0.2);
    let current = Quat::from_rotation_y(0.4);
    let local = aim_head(current, parent, Quat::IDENTITY, 0., [-angle, 0.], angle);
    let forward = parent * local * -Vec3::Z;
    assert!(forward.x > 0.65 && forward.z < -0.65);
    assert!(
        aim_head(current, parent, Quat::IDENTITY, 0., [0., 0.], angle)
            .angle_between(current)
            .abs()
            < 0.001
    );
    let rotated_canvas = aim_head(current, parent, Quat::IDENTITY, 0.1, [-angle, 0.], angle);
    let displayed = Quat::from_rotation_y(0.1) * parent * rotated_canvas * -Vec3::Z;
    assert!((displayed - forward).length() < 0.001);
    let equivalent = aim_head(current, -parent, Quat::IDENTITY, 0., [-angle, 0.], angle);
    assert!(equivalent.angle_between(local).abs() < 0.001);
    let grant = Quat::from_rotation_x(0.2) * Quat::from_rotation_y(0.3);
    let inherited = aim_head(current, parent, grant, 0., [-angle, 0.], angle);
    assert!((parent * inherited * grant * -Vec3::Z - forward).length() < 0.001);
}

#[test]
fn condition_motion_priority_fallback_and_retrigger() {
    let (dir, base) = fixture();
    let settings = Settings {
        scroll_up_motion_path: "dance.vmd".into(),
        scroll_down_motion_path: "dance.vmd".into(),
        happy_motion_path: "dance.vmd".into(),
        other_result_motion_path: "dance.vmd".into(),
        ..base.settings.clone()
    };
    let model = load(&settings, dir.path()).unwrap();
    let mut state = Reactions::default();
    let mut evaluator = Evaluator::new(&model);
    let condition = Sample {
        motion: Motion::ScrollUp,
        seconds: 0.5,
    };
    let clip_pose = evaluator.frame(&model, condition, [0., 0.], 0., 1).unwrap();
    let with_look = evaluator
        .frame(&model, condition, [-0.7, 0.2], 0., 2)
        .unwrap();
    assert_eq!(
        clip_pose.vertices[0].position,
        with_look.vertices[0].position
    );
    assert_eq!(state.sample(&model, 10., -1, 0).motion, Motion::ScrollUp);
    assert_eq!(state.sample(&model, 10.5, -1, 0).seconds, 0.5);
    assert_eq!(state.sample(&model, 10.6, 1, 0).motion, Motion::ScrollDown);
    assert_eq!(state.sample(&model, 10.7, 0, 1).motion, Motion::Happy);
    assert_eq!(state.sample(&model, 10.8, -1, 0).motion, Motion::Happy);
    let returned = state.sample(&model, 12., 0, 0);
    assert_eq!(returned.motion, Motion::Normal);
    assert_eq!(returned.seconds, 12.);
    assert_eq!(state.sample(&model, 13., 0, 2).motion, Motion::OtherResult);
    assert_eq!(
        Reactions::default().sample(&base, 1., -1, 1).motion,
        Motion::Normal
    );
    let disabled = load(
        &Settings {
            reactions: false,
            ..settings
        },
        dir.path(),
    )
    .unwrap();
    assert_eq!(state.sample(&disabled, 13.1, 1, 1).motion, Motion::Normal);
}

#[cfg(windows)]
#[test]
#[ignore = "requires user PMX/VMD paths in IIDXBG_TEST_PMX and IIDXBG_TEST_VMD"]
fn supplied_model_motion_and_gpu_preview() {
    let settings = Settings {
        model_path: std::env::var_os("IIDXBG_TEST_PMX")
            .expect("IIDXBG_TEST_PMX")
            .into(),
        motion_path: std::env::var_os("IIDXBG_TEST_VMD")
            .expect("IIDXBG_TEST_VMD")
            .into(),
        // This plush rig's weighted head is the English "head" / Japanese
        // "頭-T" bone. Its usual "頭" controller only grants 30% rotation.
        look_bone: "head".into(),
        ..Default::default()
    };
    let model = load(&settings, Path::new(".")).unwrap();
    let mut evaluator = Evaluator::new(&model);
    let start = Instant::now();
    for i in 0..120 {
        evaluator
            .frame(
                &model,
                Sample {
                    motion: Motion::Normal,
                    seconds: i as f32 / 30.,
                },
                [0., 0.],
                0.,
                i + 1,
            )
            .unwrap();
    }
    eprintln!(
        "CPU pose + skinning: {:.3} ms/frame (120 samples; excludes GPU)",
        start.elapsed().as_secs_f64() * 1000. / 120.
    );
    fs::create_dir_all("target/mmd-validation").unwrap();
    let mut previous = None;
    let rig: Vec<_> = model
        .parsed
        .skeleton
        .bones
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let influenced = model
                .parsed
                .geometry
                .skin_indices
                .iter()
                .zip(&model.parsed.geometry.skin_weights)
                .filter(|(index, weight)| **index as usize == i && **weight > 0.)
                .count();
            serde_json::json!({"index":i,"bone":b,"weighted_vertices":influenced})
        })
        .collect();
    fs::write(
        "target/mmd-validation/rig.json",
        serde_json::to_vec_pretty(&rig).unwrap(),
    )
    .unwrap();
    for (label, seconds, look, happy) in [
        ("iris-0", 0., [0., 0.], 0.),
        ("iris-1", 1., [0., 0.], 0.),
        ("iris-3", 3., [0., 0.], 0.),
        ("iris-happy", 3., [-0.28, 0.16], 1.),
        ("iris-look-right", 3., [-45f32.to_radians(), 0.], 0.),
    ] {
        let image = render::preview(model.clone(), seconds, look, happy).unwrap();
        let visible = image
            .bgra
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[3] > 0)
            .count();
        assert!(visible > 1000 && visible < (image.width * image.height) as usize);
        if let Some(old) = &previous {
            assert_ne!(&image.bgra, old);
        }
        crate::pixels::write_preview(
            &image,
            &PathBuf::from(format!("target/mmd-validation/{label}.png")),
        )
        .unwrap();
        previous = Some(image.bgra);
    }
}
#[cfg(windows)]
#[test]
#[ignore = "requires a local D3D9 GPU; writes diagnostic PNGs under target"]
fn standalone_gpu_renders_animation_and_transparency() {
    let (_dir, model) = fixture();
    let a = render::preview(model.clone(), 0., [0., 0.], 0.).unwrap();
    let b = render::preview(model, 0.5, [0.2, 0.], 1.).unwrap();
    let visible = a
        .bgra
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| p[3] > 0)
        .count();
    assert!(visible > 10000 && visible < (a.width * a.height) as usize / 2);
    assert_eq!(a.bgra[3], 0);
    assert_ne!(a.bgra, b.bgra);
    let (_dir, mut translucent) = fixture();
    Arc::get_mut(&mut translucent).unwrap().parsed.materials[0].diffuse[3] = 0.5;
    let translucent = render::preview(translucent, 0., [0., 0.], 0.).unwrap();
    assert!(
        translucent
            .bgra
            .as_chunks::<4>()
            .0
            .iter()
            .any(|p| { (120..=135).contains(&p[3]) && p[0] > p[3] + 30 }),
        "native sprite copy must contain straight-alpha RGB, not premultiplied RGB"
    );
    fs::create_dir_all("target/mmd-validation").unwrap();
    crate::pixels::write_preview(&a, Path::new("target/mmd-validation/rest.png")).unwrap();
    crate::pixels::write_preview(&b, Path::new("target/mmd-validation/motion.png")).unwrap();
}
