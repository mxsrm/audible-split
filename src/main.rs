use anyhow::{Context, Result, ensure};
use clap::Parser;
use rayon::prelude::*;
use serde::Deserialize;
use std::{fs, path::PathBuf, process::Command};

#[derive(Parser)]
#[command(version, about = "Split Audible audiobooks into chapters")]
struct Args {
    /// Input audiobook; FFmpeg must be able to read it.
    audiobook: PathBuf,

    /// Chapter metadata JSON.
    json: PathBuf,

    /// Maximum number of FFmpeg processes running simultaneously.
    #[arg(short = 'j', long, default_value_t = 4)]
    workers: usize,
}

#[derive(Deserialize)]
struct Chapter {
    title: String,
    start_offset_ms: u64,
    length_ms: u64,
}

/// Format milliseconds as decimal seconds without floating-point rounding.
fn timestamp(ms: u64) -> String {
    format!("{}.{:03}", ms / 1000, ms % 1000)
}

/// Replace common filesystem-invalid characters and remove trailing spaces/dots.
fn safe_title(title: &str) -> String {
    let sanitized: String = title
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
                '_'
            } else {
                c
            }
        })
        .collect();

    let trimmed = sanitized.trim_matches([' ', '.']);

    // Keep a usable filename even when the original title is empty.
    if trimmed.is_empty() {
        "Untitled".to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    ensure!(args.workers > 0, "workers must be greater than zero");

    let bytes = fs::read(&args.json).with_context(|| format!("Reading {}", args.json.display()))?;

    let mut data: serde_json::Value = serde_json::from_slice(&bytes).context("Parsing chapter JSON")?;

    // Accept three layouts:
    //   {"chapters": {"chapters": [...]}}
    //   {"chapters": [...]}
    //   [...]
    // take() moves the selected JSON subtree, avoiding a deep clone.
    let chapter_data = if let Some(value) = data.pointer_mut("/chapters/chapters") {
        value.take()
    } else if let Some(value) = data.get_mut("chapters") {
        value.take()
    } else {
        data
    };

    let chapters: Vec<Chapter> = serde_json::from_value(chapter_data).context("Invalid chapter data")?;

    // Validate before launching any FFmpeg processes.
    ensure!(!chapters.is_empty(), "No chapters found");
    for (i, chapter) in chapters.iter().enumerate() {
        ensure!(
            chapter.length_ms > 0,
            "Chapter {} ({}) has zero duration",
            i + 1,
            chapter.title
        );
    }

    // Example: /books/My Book.m4b -> /books/My Book/
    let output = args.audiobook.with_extension("");
    fs::create_dir_all(&output).with_context(|| format!("Creating {}", output.display()))?;

    // Each Rayon worker waits for one FFmpeg process.
    // Avoid creating more workers than there are chapters.
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(args.workers.min(chapters.len()))
        .build()
        .context("Creating worker pool")?;

    pool.install(|| {
        chapters
            .par_iter()
            .enumerate()
            // Return an error without collecting a separate results vector.
            // Already running processes finish; further work may be skipped.
            .try_for_each(|(i, chapter)| -> Result<()> {
                let number = i + 1;

                // Numbering preserves chapter order and distinguishes duplicate titles.
                let file = output.join(format!("{number:03} - {}.m4a", safe_title(&chapter.title)));

                let result = Command::new("ffmpeg")
                    // Suppress routine output, disable stdin, overwrite existing files.
                    .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y"])
                    // Input-side seeking avoids reading from the beginning each time.
                    .arg("-ss")
                    .arg(timestamp(chapter.start_offset_ms))
                    .arg("-i")
                    .arg(&args.audiobook)
                    // Limit the output to this chapter's duration.
                    .arg("-t")
                    .arg(timestamp(chapter.length_ms))
                    // Copy only the first audio stream without re-encoding.
                    // Cut boundaries remain subject to compressed packet timing.
                    .args(["-map", "0:a:0", "-c:a", "copy"])
                    // Omit the original book's full chapter table.
                    .args(["-map_chapters", "-1"])
                    // Set chapter-specific tags while retaining other default metadata.
                    .arg("-metadata")
                    .arg(format!("title={}", chapter.title))
                    .arg("-metadata")
                    .arg(format!("track={number}"))
                    .arg(&file)
                    // Capture stderr so parallel failures have separate diagnostics.
                    .output()
                    .with_context(|| format!("Starting FFmpeg for chapter {number}"))?;

                ensure!(
                    result.status.success(),
                    "Chapter {number} ({}) failed ({}):\n{}",
                    chapter.title,
                    result.status,
                    String::from_utf8_lossy(&result.stderr).trim()
                );

                // Parallel processing means completion messages can appear out of order.
                println!("✓ {number:03} {}", chapter.title);
                Ok(())
            })
    })?;

    // Reached only if every chapter succeeded.
    println!("Done: {}", output.display());
    Ok(())
}
