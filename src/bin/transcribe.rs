use anyhow::{Context, Result};
use chrono::Local;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs};

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables
    dotenvy::dotenv().ok();
    
    // Get GROQ_API_KEY from environment
    let api_key = env::var("GROQ_API_KEY").context("GROQ_API_KEY not set")?;

    // Get input file from command line args
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        anyhow::bail!("Usage: {} <audio/video file>", args[0]);
    }
    
    let input_path = PathBuf::from(&args[1]);
    if !input_path.exists() {
        anyhow::bail!("File not found: {}", input_path.display());
    }

    // Get file extension
    let extension = input_path
        .extension()
        .and_then(|e| e.to_str())
        .context("Invalid file extension")?
        .to_lowercase();

    // Prepare audio file
    let audio_file = if extension == "mp4" {
        // For MP4, extract audio to temporary FLAC file
        let temp_dir = env::temp_dir();
        let temp_file = temp_dir.join("temp_audio.flac");
        
        let status = Command::new("ffmpeg")
            .args([
                "-i", input_path.to_str().unwrap(),
                "-vn",                // Skip video
                "-acodec", "flac",    // Use FLAC codec
                "-ar", "16000",       // 16kHz sample rate
                "-ac", "1",           // Mono audio
                temp_file.to_str().unwrap(),
            ])
            .status()
            .context("Failed to run ffmpeg")?;

        if !status.success() {
            anyhow::bail!("ffmpeg failed to extract audio");
        }

        temp_file
    } else {
        // For audio files, use directly
        match extension.as_str() {
            "flac" | "mp3" | "mpeg" | "mpga" | "m4a" | "ogg" | "wav" | "webm" => input_path.clone(),
            _ => anyhow::bail!("Unsupported file format: {}", extension),
        }
    };

    // Create transcripts directory if it doesn't exist
    let transcripts_dir = Path::new("docs/transcripts");
    fs::create_dir_all(transcripts_dir)?;

    // Generate output filename with timestamp
    let timestamp = Local::now().format("%Y%m%d-%H%M%S");
    let output_path = transcripts_dir.join(format!("{}.md", timestamp));

    println!("Transcribing {}...", input_path.display());

    // Prepare multipart form
    let form = reqwest::multipart::Form::new()
        .text("model", "whisper-large-v3")
        .text("response_format", "text")
        .file("file", &audio_file)?;

    // Make API request
    let client = reqwest::Client::new();
    let response = client
        .post("https://api.groq.com/openai/v1/audio/transcriptions")
        .header("Authorization", format!("Bearer {}", api_key))
        .multipart(form)
        .send()
        .await?;

    // Check for errors
    if !response.status().is_success() {
        let error = response.text().await?;
        anyhow::bail!("API request failed: {}", error);
    }

    // Get transcription text
    let transcription = response.text().await?;

    // Write output file with metadata
    let output_content = format!(
        "---\nsource_file: {}\ntimestamp: {}\nmodel: whisper-large-v3\n---\n\n{}",
        input_path.display(),
        Local::now().to_rfc3339(),
        transcription
    );
    fs::write(&output_path, output_content)?;

    // Clean up temp file if we created one
    if extension == "mp4" {
        fs::remove_file(audio_file)?;
    }

    println!("Transcription saved to {}", output_path.display());
    Ok(())
}