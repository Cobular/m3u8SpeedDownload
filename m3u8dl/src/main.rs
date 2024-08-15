use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;

use clap::Parser;
use futures::stream::{self, StreamExt};
use reqwest::Client;
use url::Url;

use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// URL of the M3U8 file to download
    #[clap(value_parser)]
    url: String,

    /// Output file name
    #[clap(short, long, default_value = "output.mp4")]
    output: String,

    /// Enable compression
    #[clap(short, long)]
    compress: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Usage
    download_m3u8(&args.url, "output").await?;
    create_file_list("output")?;

    // Execute the ffmpeg command
    execute_ffmpeg_command("file_list.txt", &args.output, args.compress)?;

    if args.compress {
        println!("Video compressed using libx264 and aac audio.");
    }

    // Clean up the output folder
    fs::remove_dir_all("output").context("Failed to remove output folder")?;

    Ok(())
}

async fn download_m3u8(
    m3u8_url: &str,
    output_folder: &str,
) -> Result<()> {
    let client = Arc::new(Client::new());

    // Get the m3u8 file content
    let m3u8_content = client.get(m3u8_url).send().await?.text().await?;

    // Ensure the output folder exists
    fs::create_dir_all(output_folder)?;

    // Find all the .ts files
    let base_url = Url::parse(m3u8_url)?;
    let ts_urls: Vec<String> = m3u8_content
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .map(|line| base_url.join(line).unwrap().to_string())
        .collect();

    // Download each .ts file in parallel with progress bar and ETA
    let total_segments = ts_urls.len();
    let pb = ProgressBar::new(total_segments as u64);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} ({eta})")
        .unwrap()
        .progress_chars("#>-"));

    let results = stream::iter(ts_urls)
        .map(|ts_url| {
            let client = Arc::clone(&client);
            let output_folder = output_folder.to_string();
            let pb = pb.clone();
            tokio::spawn(async move {
                let result = download_ts_segment(&ts_url, &output_folder, &client).await;
                pb.inc(1);
                result
            })
        })
        .buffer_unordered(10)
        .collect::<Vec<_>>()
        .await;

    pb.finish_with_message("Download completed");

    // Check for any errors during download
    for result in results {
        result??;
    }

    println!(
        "Downloaded all segments to the '{}' folder successfully.",
        output_folder
    );
    Ok(())
}

async fn download_ts_segment(
    ts_url: &str,
    output_folder: &str,
    client: &Client,
) -> Result<()> {
    // Extract the filename from the URL
    let url = Url::parse(ts_url).context("Failed to parse TS URL")?;
    let filename = url
        .path_segments()
        .and_then(|segments| segments.last())
        .context("Failed to extract filename from URL")?;
    let output_path = Path::new(output_folder).join(filename);

    // Download the segment
    let ts_content = client.get(ts_url).send().await?.bytes().await?;

    // Save the segment to the specified output path
    fs::write(output_path, ts_content).context("Failed to write TS segment to file")?;

    Ok(())
}

fn create_file_list(output_folder: &str) -> Result<()> {
    let list_file_name = "file_list.txt";
    let mut ts_files: Vec<PathBuf> = fs::read_dir(output_folder)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("ts"))
        .collect();

    ts_files.sort();

    let mut file_list = File::create(list_file_name).context("Failed to create file list")?;
    for ts_file in ts_files.iter() {
        writeln!(file_list, "file '{}'", ts_file.display())
            .context("Failed to write to file list")?;
    }

    println!(
        "Created {} with {} files listed.",
        list_file_name,
        ts_files.len()
    );
    Ok(())
}

fn execute_ffmpeg_command(input_file: &str, output_file: &str, compress: bool) -> Result<()> {
    let mut command = Command::new("ffmpeg");
    command
        .arg("-f")
        .arg("concat")
        .arg("-safe")
        .arg("0")
        .arg("-i")
        .arg(input_file);

    if compress {
        command
            .arg("-c:v")
            .arg("libx264")
            .arg("-crf")
            .arg("23")
            .arg("-preset")
            .arg("medium")
            .arg("-c:a")
            .arg("aac")
            .arg("-b:a")
            .arg("128k");
    } else {
        command.arg("-c").arg("copy");
    }

    command.arg(output_file);

    sleep(Duration::from_secs(100));

    let output = command.output().context("Failed to execute ffmpeg command")?;

    if output.status.success() {
        println!("Successfully created {}", output_file);
        Ok(())
    } else {
        let error_message = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Error executing ffmpeg command: {}", error_message);
    }
}