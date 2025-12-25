use gpt_oss::GptOssClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = GptOssClient::builder()
        .base_url("http://localhost:8000")
        .default_model("gpt-oss-20b")
        .build()?;

    let response = client
        .complete_simple("gpt-oss-20b", "Say hello from GPT-OSS.")
        .await?;

    println!("{}", response);
    Ok(())
}
