use iidxbg::{
    config::{Config, Surface},
    pixels, profile,
    source::Provider,
};
use std::path::Path;

fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

fn run() -> iidxbg::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("mmd-inspect") if args.len() == 3 || args.len() == 4 => {
            let settings = iidxbg::mmd::Settings {
                model_path: args[2].clone().into(),
                motion_path: args.get(3).cloned().unwrap_or_default().into(),
                ..Default::default()
            };
            let model = iidxbg::mmd::load(&settings, Path::new("."))?;
            println!(
                "{}",
                serde_json::to_string_pretty(
                    &serde_json::json!({"name":model.parsed.metadata.name,"vertices":model.parsed.geometry.positions.len()/3,
                "bones":model.parsed.skeleton.bones.iter().map(|b|&b.name).collect::<Vec<_>>(),"morphs":model.parsed.morphs.iter().map(|m|&m.name).collect::<Vec<_>>(),"warnings":model.warnings})
                )?
            );
        }
        #[cfg(windows)]
        Some("mmd-preview") if args.len() == 4 || args.len() == 5 => {
            let path = Path::new(&args[2]);
            let config = Config::load(path)?;
            let model = iidxbg::mmd::load(&config.mmd, path.parent().unwrap_or(Path::new(".")))?;
            let seconds = args.get(4).map_or(Ok(0.), |s| {
                s.parse::<f32>()
                    .ok()
                    .filter(|s| s.is_finite() && *s >= 0.)
                    .ok_or(iidxbg::Error::Config(
                        "preview seconds must be a finite nonnegative number",
                    ))
            })?;
            let image = iidxbg::mmd::render::preview(model, seconds, [0., 0.], 0.)?;
            pixels::write_preview(&image, Path::new(&args[3]))?;
        }
        Some("profiles") if args.len() == 2 => {
            println!("{}", serde_json::to_string_pretty(&profile::supported())?);
        }
        #[cfg(windows)]
        Some("live-check") if args.len() == 5 => {
            use std::time::{Duration, Instant};
            let room = iidxbg::bilibili::live::room_id(&args[2])?;
            let out = Path::new(&args[3]);
            let seconds = args[4]
                .parse::<u64>()
                .ok()
                .filter(|n| (10..=300).contains(n))
                .ok_or(iidxbg::Error::Config(
                    "live-check duration must be 10..300 seconds",
                ))?;
            std::fs::create_dir_all(out)?;
            let video = iidxbg::video::load_live(room, 432, 723)?;
            pixels::write_preview(&video.first, &out.join("first.png"))?;
            let mut report = Vec::new();
            for visit in 0..2 {
                let epoch = video.begin();
                let start = Instant::now();
                let duration = if visit == 0 { seconds } else { 10 };
                let mut frames = 0;
                let mut samples = 0;
                while start.elapsed() < Duration::from_secs(duration) {
                    if let Some(frame) = video.frame(epoch, start.elapsed()) {
                        frames += 1;
                        if start.elapsed().as_secs() >= samples * 5 {
                            pixels::write_preview(
                                &frame,
                                &out.join(format!("visit-{visit}-{samples}.png")),
                            )?;
                            samples += 1;
                        }
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                video.stop();
                let error = video.take_error();
                let row = serde_json::json!({"visit":visit,"seconds":duration,"frames":frames,"last_error":error});
                println!("{row}");
                report.push(row);
                std::thread::sleep(Duration::from_secs(3));
            }
            drop(video);
            std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
            if report
                .iter()
                .any(|r| r["frames"].as_u64().unwrap_or(0) < 30)
            {
                return Err(iidxbg::Error::Video(
                    "insufficient live frames; see report".into(),
                ));
            }
        }
        Some("bilibili-check") if args.len() == 4 => {
            use iidxbg::bilibili::{Api, Mode};
            let config = Config::load(Path::new(&args[2]))?.for_surface(Surface::Results);
            let output = Path::new(&args[3]);
            std::fs::create_dir_all(output)?;
            let folders = Api::saved()?.folders()?;
            let mut reports = Vec::new();
            for mode in [
                Mode::Video,
                Mode::Recommendations,
                Mode::Playlist,
                Mode::Favourites,
            ] {
                let mut selected = config.clone();
                selected.source = iidxbg::config::Source::Bilibili;
                selected.bilibili.selection.mode = mode;
                if mode == Mode::Favourites && selected.bilibili.selection.favourites.is_empty() {
                    selected.bilibili.selection.favourites = folders
                        .first()
                        .map(|f| f.id.to_string())
                        .unwrap_or_default();
                }
                let result = Provider::new(selected)
                    .and_then(|mut p| p.prepare())
                    .and_then(|image| {
                        let file = output.join(format!("{}.png", mode.key()));
                        pixels::write_preview(&image, &file)?;
                        Ok(image.label)
                    });
                let report = match result {
                    Ok(label) => serde_json::json!({"mode":mode.key(),"ok":true,"media":label}),
                    Err(error) => {
                        serde_json::json!({"mode":mode.key(),"ok":false,"error":error.to_string()})
                    }
                };
                println!("{report}");
                reports.push(report);
            }
            let report = serde_json::json!({"account_verified":true,"folder_count":folders.len(),"checks":reports});
            std::fs::write(
                output.join("report.json"),
                serde_json::to_vec_pretty(&report)?,
            )?;
            if reports.iter().any(|r| r["ok"] != true) {
                return Err(iidxbg::Error::Bilibili(
                    "one or more checks failed; see report.json".into(),
                ));
            }
        }
        Some("verify") if args.len() == 3 => {
            let profile = profile::verify_file(&std::fs::read(&args[2])?)?;
            println!("Supported {} DLL: {}", profile.label, profile.sha256);
        }
        Some("prepare") if args.len() == 4 => {
            let config = Config::load(Path::new(&args[2]))?;
            let image = Provider::new(config.for_surface(Surface::Results))?.prepare()?;
            pixels::write_preview(&image, Path::new(&args[3]))?;
            println!(
                "Prepared {} ({}x{}) -> {}",
                image.label, image.width, image.height, args[3]
            );
        }
        Some("test-image") if args.len() == 3 => {
            let image = image::RgbaImage::from_fn(1920, 1080, |x, y| {
                if !(8..1912).contains(&x) || !(8..1072).contains(&y) {
                    image::Rgba([255, 255, 255, 255])
                } else if x.abs_diff(960) < 3 || y.abs_diff(540) < 3 {
                    image::Rgba([255, 220, 0, 255])
                } else {
                    let checker = ((x / 120 + y / 120) % 2) as u8;
                    image::Rgba([
                        30 + checker * 30,
                        (x * 180 / 1920) as u8,
                        (y * 180 / 1080) as u8,
                        255,
                    ])
                }
            });
            image.save(&args[2])?;
            println!("Created diagnostic image {}", args[2]);
        }
        _ => {
            eprintln!(
                "iidxbg-tool profiles\niidxbg-tool verify <bm2dx.dll>\niidxbg-tool prepare <iidxbg.toml> <preview.png>\niidxbg-tool test-image <output.png>\niidxbg-tool bilibili-check <iidxbg.toml> <output-directory>\niidxbg-tool live-check <room-id> <output-directory> <seconds>\niidxbg-tool mmd-inspect <model.pmx> [motion.vmd]\niidxbg-tool mmd-preview <iidxbg.toml> <preview.png> [seconds]"
            );
            std::process::exit(2);
        }
    }
    Ok(())
}
