use anyhow::{Context, Result, ensure};
use clap::Parser;
use rayon::prelude::*;
use serde::Deserialize;
use std::{fs, path::PathBuf, process::Command};

#[derive(Parser)]
#[command(version, about = "Split Audible audiobooks into chapters")]
struct Args {
    audiobook: PathBuf,
    json: PathBuf,
    #[arg(short = 'j', long, default_value_t = 4)]
    workers: usize,
}

#[derive(Deserialize)]
struct Chapter {
    title: String,
    start_offset_ms: u64,
    length_ms: u64,
}

fn main() -> Result<()> {
    let args = Args::parse();
    ensure!(args.workers > 0, "workers must be greater than zero");

    let data: serde_json::Value = serde_json::from_slice(&fs::read(&args.json)?)?;

    let chapters: Vec<Chapter> = serde_json::from_value(
        data.pointer("/chapters/chapters")
            .or_else(|| data.get("chapters"))
            .unwrap_or(&data)
            .clone(),
    )?;

    let output = args.audiobook.with_extension("");
    fs::create_dir_all(&output)?;

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(args.workers)
        .build()?;

    let results: Vec<Result<()>> = pool.install(|| {
        chapters
            .par_iter()
            .enumerate()
            .map(|(i, chapter)| {
                let title = &chapter.title;
                let safe: String = title
                    .chars()
                    .map(|c| {
                        if r#"<>:"/\|?*"#.contains(c) || c.is_control() {
                            '_'
                        } else {
                            c
                        }
                    })
                    .collect();

                let file = output.join(format!(
                    "{:03} - {}.m4a",
                    i + 1,
                    safe.trim_matches([' ', '.'])
                ));

                let status = Command::new("ffmpeg")
                    .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y"])
                    .arg("-ss")
                    .arg(format!("{:.3}", chapter.start_offset_ms as f64 / 1000.0))
                    .arg("-i")
                    .arg(&args.audiobook)
                    .arg("-t")
                    .arg(format!("{:.3}", chapter.length_ms as f64 / 1000.0))
                    .args(["-map", "0:a:0", "-c:a", "copy"])
                    .args(["-metadata", &format!("title={title}")])
                    .args(["-metadata", &format!("track={}", i + 1)])
                    .arg(&file)
                    .status()
                    .with_context(|| format!("FFmpeg: chapter {}", i + 1))?;

                ensure!(status.success(), "Failed: chapter {} ({title})", i + 1);
                println!("✓ {:03} {title}", i + 1);
                Ok(())
            })
            .collect()
    });

    for result in results {
        result?;
    }

    println!("Done: {}", output.display());
    Ok(())
}
