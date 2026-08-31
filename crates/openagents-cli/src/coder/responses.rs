//! Client for a Coder `/v1/responses` Open Responses door.
//!
//! `openagents responses` posts one prompt and renders the returned
//! Server-Sent Events stream event by event: assistant text prints as it
//! arrives, reasoning summaries print as secondary lines, tool calls and
//! their outputs print as one line each, and the terminal event either
//! reports usage or fails the command with the server's typed error.

use clap::Args;
use serde_json::{Value, json};
use std::io::Write;

/// Where the CLI points when `--origin` is absent.
const DEFAULT_ORIGIN: &str = "https://coder.openagents.com";

/// The longest tool-argument or tool-output summary printed on one line.
const SUMMARY_LIMIT: usize = 120;

/// Command-line arguments for `openagents responses`.
#[derive(Args, Debug)]
pub struct ResponsesArgs {
    /// Prompt to send to the Coder Responses endpoint.
    pub prompt: String,

    /// Coder origin to post to. Defaults to the hosted Coder service.
    #[arg(long)]
    pub origin: Option<String>,

    /// Continue an existing response. Sent as `previous_response_id`.
    #[arg(long = "response", value_name = "ID")]
    pub response: Option<String>,

    /// Replay stream events after this sequence number. Sent as
    /// `starting_after` so a dropped stream can be resumed manually.
    #[arg(long = "starting-after", value_name = "SEQ")]
    pub starting_after: Option<u64>,
}

/// Post a prompt to the Coder Responses endpoint and render the stream.
pub async fn run(args: ResponsesArgs) -> Result<(), reqwest::Error> {
    let origin = args
        .origin
        .clone()
        .unwrap_or_else(|| DEFAULT_ORIGIN.to_string());
    // The bearer is the one stored for the Coder origin itself, not the one
    // stored for the platform API endpoint. An origin with no stored
    // credential sends none.
    let token = crate::auth::CredentialStore::for_origin(&origin).get_token();
    let client = Client::new(origin, token);

    let mut body = json!({
        "input": [{"type": "message", "role": "user", "content": args.prompt}]
    });
    if let Some(id) = &args.response {
        body["previous_response_id"] = json!(id);
    }
    if let Some(seq) = args.starting_after {
        body["starting_after"] = json!(seq);
    }

    let response = client.post(&body).await?;
    let status = response.status();
    if !status.is_success() {
        let message = response.text().await.unwrap_or_default();
        let message = if message.trim().is_empty() {
            format!("the Coder origin answered {status}")
        } else {
            message.trim().to_string()
        };
        crate::errors::fail(&crate::errors::CliError::Api {
            status: status.as_u16(),
            code: None,
            message,
            request_id: None,
        });
    }
    render_stream(response).await
}

/// Read the Server-Sent Events body and render each event as it arrives.
async fn render_stream(mut response: reqwest::Response) -> Result<(), reqwest::Error> {
    let mut renderer = Renderer::new(std::io::stdout(), crate::diag::color());
    let mut buffer = String::new();
    let mut failure = None;
    while let Some(chunk) = response.chunk().await? {
        buffer.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(end) = buffer.find('\n') {
            let line: String = buffer.drain(..=end).collect();
            if let Some(found) = renderer.line(line.trim_end_matches(['\r', '\n'])) {
                failure = Some(found);
            }
        }
    }
    if let Some(found) = renderer.line(buffer.trim_end_matches('\r')) {
        failure = Some(found);
    }
    renderer.finish();
    if let Some(failure) = failure {
        // The stream carried `response.failed`: the server's typed error is
        // the reason this command stops, printed on stderr with a nonzero
        // exit. The HTTP exchange itself succeeded, so the stored status is
        // the one the stream came in on.
        crate::errors::fail(&crate::errors::CliError::Api {
            status: 200,
            code: Some(failure.code),
            message: failure.message,
            request_id: None,
        });
    }
    Ok(())
}

/// The typed error a `response.failed` event carried.
struct Failure {
    code: String,
    message: String,
}

/// What the current unterminated output line holds.
#[derive(PartialEq)]
enum Segment {
    None,
    Text,
    Reasoning,
}

/// Renders one Server-Sent Events stream onto one writer.
///
/// Text deltas concatenate onto one line until another event interrupts.
/// Everything that is not assistant text prints as its own line, dimmed when
/// colour is on and prefixed the same either way.
struct Renderer<W: Write> {
    out: W,
    color: bool,
    segment: Segment,
}

impl<W: Write> Renderer<W> {
    fn new(out: W, color: bool) -> Self {
        Self {
            out,
            color,
            segment: Segment::None,
        }
    }

    /// Consume one line of the stream. A line that is not a well-formed
    /// `data:` event is skipped, and an unknown event type is skipped too,
    /// so a newer server does not break an older client.
    fn line(&mut self, line: &str) -> Option<Failure> {
        let data = line.strip_prefix("data:")?;
        let event: Value = serde_json::from_str(data.trim()).ok()?;
        self.event(&event)
    }

    fn event(&mut self, event: &Value) -> Option<Failure> {
        match event["type"].as_str().unwrap_or_default() {
            "response.output_text.delta" => {
                self.text(event["delta"].as_str().unwrap_or_default());
            }
            "response.reasoning_summary_text.delta" => {
                self.reasoning(event["delta"].as_str().unwrap_or_default());
            }
            "response.output_item.done" => self.item(&event["item"]),
            "response.completed" => {
                self.break_segment();
                let usage = &event["response"]["usage"];
                let line = format!(
                    "completed: input_tokens={} output_tokens={}",
                    usage["input_tokens"].as_u64().unwrap_or(0),
                    usage["output_tokens"].as_u64().unwrap_or(0)
                );
                self.secondary(&line);
            }
            "response.failed" => {
                self.break_segment();
                let error = &event["response"]["error"];
                return Some(Failure {
                    code: error["code"].as_str().unwrap_or("error").to_string(),
                    message: error["message"]
                        .as_str()
                        .unwrap_or("the run failed and the server sent no message")
                        .to_string(),
                });
            }
            "response.cancelled" => {
                self.break_segment();
                self.secondary("cancelled");
            }
            "response.resumed" => {
                self.break_segment();
                let mut line = "resumed:".to_string();
                if let Some(run) = event["run"].as_str() {
                    line.push_str(&format!(" run={run}"));
                }
                if let Some(seq) = event["replay_from"].as_u64() {
                    line.push_str(&format!(" replay_from={seq}"));
                }
                self.secondary(&line);
            }
            // Unknown event types are skipped, not failed: the contract may
            // grow events this client does not know yet.
            _ => {}
        }
        None
    }

    /// One complete output item. Text arrived as deltas already, so a
    /// `message` item prints nothing; the tool items print one line each.
    fn item(&mut self, item: &Value) {
        match item["type"].as_str().unwrap_or_default() {
            "function_call" => {
                self.break_segment();
                let name = item["name"].as_str().unwrap_or("tool");
                let arguments = summarize(item["arguments"].as_str().unwrap_or_default());
                self.secondary(&format!("tool: {name} {arguments}"));
            }
            "function_call_output" => {
                self.break_segment();
                let output = summarize(item["output"].as_str().unwrap_or_default());
                self.secondary(&format!("tool output: {output}"));
            }
            _ => {}
        }
    }

    fn text(&mut self, delta: &str) {
        if self.segment != Segment::Text {
            self.break_segment();
            self.segment = Segment::Text;
        }
        let _ = write!(self.out, "{delta}");
        let _ = self.out.flush();
    }

    fn reasoning(&mut self, delta: &str) {
        if self.segment != Segment::Reasoning {
            self.break_segment();
            self.segment = Segment::Reasoning;
            if self.color {
                let _ = write!(self.out, "\x1b[2m");
            }
            let _ = write!(self.out, "thinking: ");
        }
        let _ = write!(self.out, "{delta}");
        let _ = self.out.flush();
    }

    /// One whole secondary line: dim when colour is on, plain otherwise.
    fn secondary(&mut self, line: &str) {
        if self.color {
            let _ = writeln!(self.out, "\x1b[2m{line}\x1b[0m");
        } else {
            let _ = writeln!(self.out, "{line}");
        }
        let _ = self.out.flush();
    }

    /// End whatever line is open so the next event starts on its own line.
    fn break_segment(&mut self) {
        match self.segment {
            Segment::None => {}
            Segment::Text => {
                let _ = writeln!(self.out);
            }
            Segment::Reasoning => {
                if self.color {
                    let _ = write!(self.out, "\x1b[0m");
                }
                let _ = writeln!(self.out);
            }
        }
        self.segment = Segment::None;
        let _ = self.out.flush();
    }

    fn finish(&mut self) {
        self.break_segment();
    }
}

/// Collapse whitespace and truncate so a tool call or output fits one line.
fn summarize(text: &str) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= SUMMARY_LIMIT {
        return collapsed;
    }
    let truncated: String = collapsed.chars().take(SUMMARY_LIMIT).collect();
    format!("{truncated}...")
}

/// Client for the Coder Responses endpoint.
pub struct Client {
    http: reqwest::Client,
    origin: String,
    api_key: Option<String>,
}

impl Client {
    /// Make a new client for `origin` with an optional bearer token.
    pub fn new(origin: String, api_key: Option<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            origin,
            api_key,
        }
    }

    /// Post a request body to `/v1/responses` and return the raw response.
    pub async fn post(&self, body: &Value) -> Result<reqwest::Response, reqwest::Error> {
        let mut request = self
            .http
            .post(format!("{}/v1/responses", self.origin))
            .header("accept", "text/event-stream")
            .json(body);

        if let Some(key) = &self.api_key {
            request = request.header("authorization", format!("Bearer {key}"));
        }

        request.send().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(events: &[&str]) -> (String, Option<Failure>) {
        let mut out = Vec::new();
        let mut failure = None;
        {
            let mut renderer = Renderer::new(&mut out, false);
            for event in events {
                if let Some(found) = renderer.line(&format!("data: {event}")) {
                    failure = Some(found);
                }
            }
            renderer.finish();
        }
        (String::from_utf8(out).unwrap(), failure)
    }

    #[test]
    fn text_deltas_concatenate_and_other_events_take_their_own_line() {
        let (output, failure) = render(&[
            r#"{"type":"response.reasoning_summary_text.delta","seq":1,"delta":"Read the test."}"#,
            r#"{"type":"response.output_text.delta","seq":2,"delta":"I will "}"#,
            r#"{"type":"response.output_text.delta","seq":3,"delta":"fix it."}"#,
            r#"{"type":"response.output_item.done","seq":4,"item":{"type":"function_call","call_id":"c1","name":"read_file","arguments":"{\"path\": \"src/lib.rs\"}"}}"#,
            r#"{"type":"response.completed","seq":5,"response":{"id":"r","usage":{"input_tokens":12,"output_tokens":3}}}"#,
        ]);
        assert!(failure.is_none());
        assert_eq!(
            output,
            "thinking: Read the test.\n\
             I will fix it.\n\
             tool: read_file {\"path\": \"src/lib.rs\"}\n\
             completed: input_tokens=12 output_tokens=3\n"
        );
    }

    #[test]
    fn a_failed_event_surfaces_the_typed_error() {
        let (_, failure) = render(&[
            r#"{"type":"response.failed","seq":7,"response":{"id":"r","error":{"code":"insufficient_credit","message":"The spending grant is exhausted."}}}"#,
        ]);
        let failure = failure.expect("the failed event carries the error");
        assert_eq!(failure.code, "insufficient_credit");
        assert_eq!(failure.message, "The spending grant is exhausted.");
    }

    #[test]
    fn unknown_events_and_non_data_lines_are_skipped() {
        let (output, failure) = render(&[
            r#"{"type":"response.some_future_event","seq":1,"payload":true}"#,
            r#"not json at all"#,
        ]);
        assert!(failure.is_none());
        assert_eq!(output, "");
    }

    #[test]
    fn summaries_collapse_whitespace_and_truncate() {
        assert_eq!(summarize("a\n  b\tc"), "a b c");
        let long = "x".repeat(200);
        let summary = summarize(&long);
        assert_eq!(summary.chars().count(), SUMMARY_LIMIT + 3);
        assert!(summary.ends_with("..."));
    }
}
