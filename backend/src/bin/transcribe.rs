use anyhow::{Context, Result};
use chrono::Local;
use reqwest::multipart;
use serde_json::json;
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

    // Get file extension and name
    let extension = input_path
        .extension()
        .and_then(|e| e.to_str())
        .context("Invalid file extension")?
        .to_lowercase();

    let file_name = input_path
        .file_name()
        .and_then(|n| n.to_str())
        .context("Invalid filename")?
        .to_string();

    println!("Processing file: {}", file_name);

    // Prepare audio file
    let audio_file = if extension == "mp4" {
        println!("Converting video to audio...");
        // For MP4, extract audio to temporary FLAC file
        let temp_dir = env::temp_dir();
        let temp_file = temp_dir.join("temp_audio.flac");

        let status = Command::new("ffmpeg")
            .args([
                "-i",
                input_path.to_str().unwrap(),
                "-vn", // Skip video
                "-acodec",
                "flac", // Use FLAC codec
                "-ar",
                "16000", // 16kHz sample rate
                "-ac",
                "1", // Mono audio
                temp_file.to_str().unwrap(),
            ])
            .status()
            .context("Failed to run ffmpeg")?;

        if !status.success() {
            anyhow::bail!("ffmpeg failed to extract audio");
        }
        println!("Audio extraction complete");

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

    // Generate slug using llama-3.1-8b-instant
    println!("Generating filename slug...");
    let client = reqwest::Client::new();
    let slug_response = client
        .post("https://api.groq.com/openai/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&json!({
            "model": "llama-3.1-8b-instant",
            "messages": [
                {
                    "role": "system",
                    "content": "You are a filename generator. Generate a slug from the given filename, keeping numbers and converting spaces and special characters to hyphens. Example: 'My Video 123.mp4' -> 'my-video-123'"
                },
                {
                    "role": "user",
                    "content": file_name
                }
            ],
            "temperature": 0.0
        }))
        .send()
        .await?;

    let slug = if slug_response.status().is_success() {
        let json: serde_json::Value = slug_response.json().await?;
        json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("transcript")
            .trim()
            .to_string()
    } else {
        println!("Warning: Failed to generate slug, using 'transcript'");
        "transcript".to_string()
    };

    // Generate timestamp in military time (Central US)
    let central_time = Local::now();
    let timestamp = central_time.format("%Y%m%d-%H%M");
    let output_path = transcripts_dir.join(format!("{}-{}.md", timestamp, slug));

    println!("Transcribing audio...");

    // Read file contents
    let file_contents = fs::read(&audio_file)?;
    let file_name = audio_file.file_name().unwrap().to_string_lossy();

    // Prepare multipart form
    let file_part = multipart::Part::bytes(file_contents)
        .file_name(file_name.to_string())
        .mime_str("audio/flac")?;

    let form = multipart::Form::new()
        .text("model", "whisper-large-v3")
        .text("response_format", "text")
        .part("file", file_part);

    // Make API request for transcription
    let response = client
        .post("https://api.groq.com/openai/v1/audio/transcriptions")
        .header("Authorization", format!("Bearer {}", api_key))
        .multipart(form)
        .send()
        .await?;

    // Check for errors
    if !response.status().is_success() {
        let error = response.text().await?;
        anyhow::bail!("Transcription API request failed: {}", error);
    }

    // Get transcription text
    let transcription: String = response.text().await?;
    println!("Transcription complete");

    // Format transcription with line breaks using llama-3.3-70b-versatile
    println!("Formatting transcription for Markdown...");
    let format_response = client
        .post("https://api.groq.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&serde_json::json!({
            "model": "mixtral-8x7b-32768",
            "messages": [{
                "role": "user",
                "content": format!("Format this transcription with proper punctuation and paragraphs:\n\n{}", transcription)
            }]
        }))
        .send()
        .await?;

    let formatted_text = if format_response.status().is_success() {
        let json: serde_json::Value = format_response.json().await?;
        json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or(&transcription)
            .to_string()
    } else {
        transcription.to_string()
    };

    // Write output file with metadata
    let output_content = format!(
        "---\nsource_file: {}\ntimestamp: {}\nmodel: whisper-large-v3\n---\n\n{}",
        file_name,
        central_time.to_rfc3339(),
        formatted_text
    );
    fs::write(&output_path, output_content)?;

    // Clean up temp file if we created one
    if extension == "mp4" {
        fs::remove_file(audio_file)?;
    }

    println!("Transcription saved to {}", output_path.display());
    Ok(())
}
