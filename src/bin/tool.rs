use iidxbg::{config::Config, pixels, profile, source::Provider};
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
        Some("verify") if args.len() == 3 => {
            let profile = profile::verify_file(&std::fs::read(&args[2])?)?;
            println!("Supported {} DLL: {}", profile.label, profile.sha256);
        }
        Some("prepare") if args.len() == 4 => {
            let config = Config::load(Path::new(&args[2]))?;
            let image = Provider::new(config)?.prepare()?;
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
                "iidxbg-tool verify <bm2dx.dll>\niidxbg-tool prepare <iidxbg.toml> <preview.png>\niidxbg-tool test-image <output.png>"
            );
            std::process::exit(2);
        }
    }
    Ok(())
}
