use std::{collections::BTreeMap, env, error::Error, io::Cursor, time::Duration};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use image::{Delay, DynamicImage, Frame, ImageBuffer, Rgb, Rgba, codecs::gif::GifEncoder};
use reqwest::blocking::{Client, Response};
use serde::Serialize;
use serde_json::{Value, json};

#[derive(Serialize)]
struct JsonResponseCapture {
    status: u16,
    headers: BTreeMap<String, String>,
    body: Value,
}

#[derive(Serialize)]
struct StreamResponseCapture {
    status: u16,
    headers: BTreeMap<String, String>,
    event_count: usize,
    output_text: String,
    events: Vec<Value>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let base_url = args
        .next()
        .ok_or("usage: qwen38_openai_media_smoke <base-url>")?;
    if args.next().is_some() {
        return Err("unexpected trailing arguments".into());
    }

    let client = Client::builder()
        .timeout(Duration::from_secs(600))
        .build()?;
    let health = get_json(&client, format!("{base_url}/health"))?;
    let models = get_json(&client, format!("{base_url}/v1/models"))?;
    let model_card = models.body["data"]
        .as_array()
        .and_then(|models| {
            models.iter().find(|model| {
                model["psionic_model_family"] == Value::String(String::from("qwen38"))
            })
        })
        .ok_or("server did not publish a Qwen3.8 model card")?;
    let model_id = model_card["id"]
        .as_str()
        .ok_or("Qwen3.8 model card is missing its id")?
        .to_string();

    let image_url = deterministic_png_data_url()?;
    let video_url = deterministic_gif_data_url()?;
    let chat_url = format!("{base_url}/v1/chat/completions");
    let responses_url = format!("{base_url}/v1/responses");

    let image_chat_tools = post_json(
        &client,
        chat_url.as_str(),
        &json!({
            "model": model_id,
            "messages": [{
                "role": "user",
                "content": [
                    {"type": "image_url", "image_url": {"url": image_url}},
                    {"type": "text", "text": "Describe the visual input in one short sentence."}
                ]
            }],
            "tools": [{
                "type": "function",
                "function": {
                    "name": "record_visual_summary",
                    "description": "Record a short visual summary.",
                    "parameters": {
                        "type": "object",
                        "properties": {"summary": {"type": "string"}},
                        "required": ["summary"],
                        "additionalProperties": false
                    }
                }
            }],
            "tool_choice": "auto",
            "temperature": 0.0,
            "max_tokens": 64,
            "psionic_enable_thinking": false
        }),
    )?;

    let video_chat_stream = post_stream(
        &client,
        chat_url.as_str(),
        &json!({
            "model": model_id,
            "messages": [{
                "role": "user",
                "content": [
                    {"type": "video_url", "video_url": {"url": video_url}},
                    {"type": "text", "text": "Describe the visual input in one short sentence."}
                ]
            }],
            "stream": true,
            "temperature": 0.0,
            "max_tokens": 2,
            "psionic_enable_thinking": false
        }),
    )?;

    let responses_image = post_json(
        &client,
        responses_url.as_str(),
        &json!({
            "model": model_id,
            "input": [{
                "type": "message",
                "role": "user",
                "content": [
                    {"type": "input_image", "image_url": image_url},
                    {"type": "input_text", "text": "Describe the visual input in one short sentence."}
                ]
            }],
            "temperature": 0.0,
            "max_output_tokens": 2,
            "psionic_enable_thinking": false
        }),
    )?;
    let responses_media_continuation = if let Some(response_id) =
        responses_image.body["id"].as_str()
    {
        post_json(
            &client,
            responses_url.as_str(),
            &json!({
                "model": model_id,
                "previous_response_id": response_id,
                "input": "Continue without replaying the attachment.",
                "max_output_tokens": 1
            }),
        )?
    } else {
        JsonResponseCapture {
            status: 0,
            headers: BTreeMap::new(),
            body: json!({
                "error": {
                    "message": "continuation probe skipped because Responses image generation returned no id"
                }
            }),
        }
    };
    let remote_image = post_json(
        &client,
        chat_url.as_str(),
        &single_media_chat_request(
            model_id.as_str(),
            "image_url",
            "image_url",
            "https://example.invalid/image.png",
        ),
    )?;
    let mp4_video = post_json(
        &client,
        chat_url.as_str(),
        &single_media_chat_request(
            model_id.as_str(),
            "video_url",
            "video_url",
            "data:video/mp4;base64,bm90LWFuLW1wNA==",
        ),
    )?;
    let malformed_base64 = post_json(
        &client,
        chat_url.as_str(),
        &single_media_chat_request(
            model_id.as_str(),
            "image_url",
            "image_url",
            "data:image/png;base64,not-valid-base64!",
        ),
    )?;
    let five_attachments = post_json(
        &client,
        chat_url.as_str(),
        &json!({
            "model": model_id,
            "messages": [{
                "role": "user",
                "content": (0..5).map(|_| json!({
                    "type": "image_url",
                    "image_url": {"url": image_url}
                })).collect::<Vec<_>>()
            }],
            "max_tokens": 1
        }),
    )?;

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "schema_version": "psionic.qwen38.openai_media_smoke.v1",
            "status": "implemented_early",
            "phase": "R11",
            "model_id": model_id,
            "server": {
                "health": health,
                "models": models,
            },
            "generation": {
                "chat_image_with_tools": image_chat_tools,
                "chat_video_stream": video_chat_stream,
                "responses_image": responses_image,
            },
            "refusals": {
                "responses_media_continuation": responses_media_continuation,
                "remote_image": remote_image,
                "mp4_video": mp4_video,
                "malformed_base64": malformed_base64,
                "five_attachments": five_attachments,
            },
            "request_contract": {
                "image_transport": "base64_png_data_url",
                "video_transport": "base64_animated_gif_data_url",
                "video_source_frames": 8,
                "video_source_fps": 4.0,
                "chat_tools_present": true,
                "chat_tool_choice": "auto",
                "chat_max_tokens": 64,
                "responses_official_input_parts": ["input_image", "input_text"],
            },
            "claim_boundary": {
                "cpu_vision": true,
                "cuda_decoder": true,
                "chat_image_generation": true,
                "chat_video_streaming": true,
                "responses_image_generation": true,
                "metal_decoder_integration": false,
                "remote_media_fetch": false,
                "mp4_decode": false,
                "responses_binary_media_replay": false,
                "performance_claim": false,
            }
        }))?
    );
    Ok(())
}

fn single_media_chat_request(model_id: &str, part_type: &str, url_field: &str, url: &str) -> Value {
    json!({
        "model": model_id,
        "messages": [{
            "role": "user",
            "content": [{
                "type": part_type,
                (url_field): {"url": url}
            }]
        }],
        "max_tokens": 1
    })
}

fn get_json(client: &Client, url: String) -> Result<JsonResponseCapture, Box<dyn Error>> {
    capture_json_response(client.get(url).send()?)
}

fn post_json(
    client: &Client,
    url: &str,
    body: &Value,
) -> Result<JsonResponseCapture, Box<dyn Error>> {
    capture_json_response(client.post(url).json(body).send()?)
}

fn capture_json_response(response: Response) -> Result<JsonResponseCapture, Box<dyn Error>> {
    let status = response.status().as_u16();
    let headers = psionic_headers(response.headers());
    let text = response.text()?;
    let body =
        serde_json::from_str(text.as_str()).unwrap_or_else(|_| json!({"unparsed_body": text}));
    Ok(JsonResponseCapture {
        status,
        headers,
        body,
    })
}

fn post_stream(
    client: &Client,
    url: &str,
    body: &Value,
) -> Result<StreamResponseCapture, Box<dyn Error>> {
    let response = client.post(url).json(body).send()?;
    let status = response.status().as_u16();
    let headers = psionic_headers(response.headers());
    let text = response.text()?;
    let mut events = Vec::new();
    let mut output_text = String::new();
    for line in text.lines() {
        let Some(payload) = line.strip_prefix("data: ") else {
            continue;
        };
        if payload == "[DONE]" {
            continue;
        }
        let event = serde_json::from_str::<Value>(payload)?;
        if let Some(content) = event["choices"][0]["delta"]["content"].as_str() {
            output_text.push_str(content);
        }
        events.push(event);
    }
    Ok(StreamResponseCapture {
        status,
        headers,
        event_count: events.len(),
        output_text,
        events,
    })
}

fn psionic_headers(headers: &reqwest::header::HeaderMap) -> BTreeMap<String, String> {
    headers
        .iter()
        .filter_map(|(name, value)| {
            name.as_str()
                .starts_with("x-psionic-")
                .then(|| {
                    value
                        .to_str()
                        .ok()
                        .map(|value| (name.as_str().to_string(), value.to_string()))
                })
                .flatten()
        })
        .collect()
}

fn deterministic_png_data_url() -> Result<String, Box<dyn Error>> {
    let image = ImageBuffer::from_fn(256, 256, |x, y| {
        Rgb([
            (x % 256) as u8,
            (y % 256) as u8,
            (((x + y) / 2) % 256) as u8,
        ])
    });
    let mut bytes = Vec::new();
    DynamicImage::ImageRgb8(image)
        .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)?;
    Ok(format!(
        "data:image/png;base64,{}",
        BASE64_STANDARD.encode(bytes)
    ))
}

fn deterministic_gif_data_url() -> Result<String, Box<dyn Error>> {
    let mut bytes = Vec::new();
    {
        let mut encoder = GifEncoder::new(&mut bytes);
        for frame_index in 0..8_u32 {
            let image = ImageBuffer::from_fn(256, 256, |x, y| {
                Rgba([
                    ((x + frame_index * 17) % 256) as u8,
                    ((y + frame_index * 29) % 256) as u8,
                    (((x + y) / 2 + frame_index * 11) % 256) as u8,
                    255,
                ])
            });
            encoder.encode_frame(Frame::from_parts(
                image,
                0,
                0,
                Delay::from_numer_denom_ms(250, 1),
            ))?;
        }
    }
    Ok(format!(
        "data:image/gif;base64,{}",
        BASE64_STANDARD.encode(bytes)
    ))
}
