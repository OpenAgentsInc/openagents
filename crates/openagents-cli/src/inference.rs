//! `openagents inference`: run a model through the OpenAgents API
//! (`docs/inference/gateway.md`), list its models, and read its rate card.
//!
//! It is a plain HTTP client of the public API: `POST /v1/responses` (or
//! `/v1/chat/completions` with `--api chat`), `GET /v1/models`, and
//! `GET /v1/rates`, at `https://api.openagents.com/v1` unless `--base` or
//! `OPENAGENTS_API_BASE` names another. A key comes from `--key` or
//! `OPENAGENTS_API_KEY`; with none, `--pay x402` pays per request over
//! Lightning from the wallet, as `openagents x402 fetch` does.

use std::io::{BufRead, BufReader, Read, Write};
use std::time::Duration;

use serde_json::{Map, Value, json};

use crate::{Args, Output};
#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};

pub(crate) const USAGE: &str = "usage: openagents inference COMMAND [OPTIONS]
  run MODEL [TEXT] [--input TEXT] [--json BODY] [--instructions TEXT] [--stream]
      [--format text|json|events] [--api responses|chat] [--max-output-tokens N]
      [--max-price IN/OUT] [--base URL] [--key KEY] [--pay x402 --max-msat N]
        Run a model through the OpenAgents API and print its answer.
        `openagents inference MODEL [TEXT]` is the same. MODEL is a model id
        (google/gemini-3.8-flash, or just gemini-3.8-flash) or a task class we
        route for you: openagents/auto, openagents/chat, openagents/fast,
        openagents/code, openagents/long, openagents/reason, openagents/classify.
        TEXT or --input is the prompt; `-` reads it from stdin. --json BODY is
        the whole request body as JSON (`-` reads stdin); MODEL fills its
        `model` when it has none. --stream prints the answer as it arrives.
        --format: text (the answer's words, the default), json (the whole
        answer), or events (each streamed event as one JSON line). --api:
        responses (Open Responses, the default) or chat (OpenAI Chat
        Completions). --max-price: the most to pay per million tokens, in
        dollars, such as 0.50/2.00. --base: the API (default
        https://api.openagents.com/v1, or OPENAGENTS_API_BASE). --key: your
        API key (default OPENAGENTS_API_KEY); keys draw on your account's
        credit. --pay x402: with no key, pay for this one request over
        Lightning from your wallet, at most --max-msat N (also --max-fee-msat N
        and --pay-with wallet|node).
  models [--base URL] [--key KEY]
        Every model, its providers, and its price.
  rates [--base URL]
        The rate card: each provider's price, our margin, and what you pay,
        per million tokens.
The API's description is at BASE/openapi.json.";

/// What each command above does, for the chat router's command tree.
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    // A request is charged to the key's credit, or paid over Lightning.
    Declared::computer("run", Effect::Spends),
    Declared::computer("models", Effect::ReadOnly),
    Declared::computer("rates", Effect::ReadOnly),
];

const DEFAULT_BASE: &str = "https://api.openagents.com/v1";
const SWITCHES: &[&str] = &["stream"];

/// How to print the answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Format {
    Text,
    Json,
    Events,
}

/// Which API to call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Api {
    Responses,
    Chat,
}

impl Api {
    fn path(self) -> &'static str {
        match self {
            Self::Responses => "/responses",
            Self::Chat => "/chat/completions",
        }
    }
}

/// One request, ready to send.
#[derive(Clone, Debug)]
pub(crate) struct Call {
    pub base: String,
    pub key: Option<String>,
    pub api: Api,
    pub format: Format,
    pub body: Value,
}

impl Call {
    fn url(&self) -> String {
        format!("{}{}", self.base, self.api.path())
    }
}

pub fn run(output: &Output, words: &[String]) -> u8 {
    if words.first().is_some_and(|word| word == "help")
        || words.iter().any(|word| word == "--help" || word == "-h")
    {
        println!("{USAGE}");
        return 0;
    }
    let args = match Args::parse(words, SWITCHES) {
        Ok(args) => args,
        Err(message) => return output.usage("inference", &message, USAGE),
    };
    let Some(first) = args.positional().first().cloned() else {
        return output.usage("inference", "name a model, or `models` or `rates`", USAGE);
    };
    let base = base(&args);
    let key = key(&args);
    match first.as_str() {
        "models" => match get_json(&format!("{base}/models"), key.as_deref()) {
            Ok(models) => {
                output.emit(&models, render_models);
                0
            }
            Err(message) => output.fail("inference", &message),
        },
        "rates" => match get_json(&format!("{base}/rates"), None) {
            Ok(card) => {
                output.emit(&card, render_rates);
                0
            }
            Err(message) => output.fail("inference", &message),
        },
        first => {
            // `run MODEL ...`, or `MODEL ...` for short.
            let args = if first == "run" {
                let rest = args.positional()[1..].to_vec();
                Args::from_positional(&rest, &args)
            } else {
                args
            };
            let Some(model) = args.positional().first() else {
                return output.usage("inference", "run needs a model", USAGE);
            };
            let model = resolve_model(&base, key.as_deref(), model);
            let call = match build(&args, &model, output.json(), &mut std::io::stdin()) {
                Ok(call) => Call { base, key, ..call },
                Err(message) => return output.usage("inference", &message, USAGE),
            };
            let pay = args.option("pay");
            let mut stdout = std::io::stdout();
            let result = match pay {
                None => send(&call, &mut stdout, None),
                Some("x402") => pay_x402(&args, &call, &mut stdout),
                Some(other) => {
                    return output.usage(
                        "inference",
                        &format!("--pay takes x402, not {other}"),
                        USAGE,
                    );
                }
            };
            match result {
                Ok(()) => 0,
                Err(message) => output.fail("inference", &message),
            }
        }
    }
}

fn base(args: &Args) -> String {
    args.option("base")
        .map(str::to_owned)
        .or_else(|| std::env::var("OPENAGENTS_API_BASE").ok())
        .filter(|base| !base.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_BASE.to_owned())
        .trim_end_matches('/')
        .to_owned()
}

fn key(args: &Args) -> Option<String> {
    args.option("key")
        .map(str::to_owned)
        .or_else(|| std::env::var("OPENAGENTS_API_KEY").ok())
        .filter(|key| !key.trim().is_empty())
}

/// A bare model name (`gemini-3.8-flash`) is the catalog's one id that
/// ends with it (`google/gemini-3.8-flash`); anything else is sent as
/// given.
fn resolve_model(base: &str, key: Option<&str>, model: &str) -> String {
    if model.contains('/') {
        return model.to_owned();
    }
    let Ok(catalog) = get_json(&format!("{base}/models"), key) else {
        return model.to_owned();
    };
    let suffix = format!("/{model}");
    let matches: Vec<&str> = catalog["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry["id"].as_str())
        .filter(|id| id.ends_with(&suffix))
        .collect();
    match matches.as_slice() {
        [one] => (*one).to_owned(),
        _ => model.to_owned(),
    }
}

fn read_all(stdin: &mut dyn Read) -> Result<String, String> {
    let mut text = String::new();
    stdin
        .read_to_string(&mut text)
        .map_err(|error| format!("read stdin: {error}"))?;
    Ok(text)
}

/// The request the options describe. `json_output` is the global `--json`.
pub(crate) fn build(
    args: &Args,
    model: &str,
    json_output: bool,
    stdin: &mut dyn Read,
) -> Result<Call, String> {
    let api = match args.option("api").unwrap_or("responses") {
        "responses" => Api::Responses,
        "chat" => Api::Chat,
        other => return Err(format!("--api takes responses or chat, not {other}")),
    };
    let format = match args.option("format") {
        None if json_output => Format::Json,
        None | Some("text") => Format::Text,
        Some("json") => Format::Json,
        Some("events") => Format::Events,
        Some(other) => return Err(format!("--format takes text, json, or events, not {other}")),
    };
    let mut body = match args.option("json") {
        Some(text) => {
            let text = if text == "-" {
                read_all(stdin)?
            } else {
                text.to_owned()
            };
            let value: Value = serde_json::from_str(&text)
                .map_err(|error| format!("--json is not valid JSON: {error}"))?;
            match value {
                Value::Object(map) => map,
                _ => return Err("--json must be a JSON object".into()),
            }
        }
        None => Map::new(),
    };
    body.entry("model")
        .or_insert_with(|| Value::String(model.to_owned()));
    let input = match (
        args.option("input"),
        args.positional().get(1).map(String::as_str),
    ) {
        (Some(_), Some(_)) => return Err("give the prompt once: TEXT or --input".into()),
        (Some(text), None) | (None, Some(text)) => Some(text),
        (None, None) => None,
    };
    let input = match input {
        Some("-") => Some(read_all(stdin)?),
        Some(text) => Some(text.to_owned()),
        None => None,
    };
    let instructions = args.option("instructions");
    match api {
        Api::Responses => {
            if let Some(text) = input {
                body.insert("input".into(), Value::String(text));
            }
            if let Some(text) = instructions {
                body.insert("instructions".into(), Value::String(text.to_owned()));
            }
        }
        Api::Chat => {
            let mut messages: Vec<Value> = body
                .get("messages")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            if let Some(text) = instructions {
                messages.insert(0, json!({"role": "system", "content": text}));
            }
            if let Some(text) = input {
                messages.push(json!({"role": "user", "content": text}));
            }
            if !messages.is_empty() {
                body.insert("messages".into(), Value::Array(messages));
            }
        }
    }
    let has_input = match api {
        Api::Responses => body.contains_key("input"),
        Api::Chat => body.contains_key("messages"),
    };
    if !has_input {
        return Err("give a prompt: TEXT, --input, or --json with one".into());
    }
    if let Some(text) = args.option("max-output-tokens") {
        let tokens: u64 = text
            .parse()
            .map_err(|_| format!("--max-output-tokens takes a number, not {text}"))?;
        let field = match api {
            Api::Responses => "max_output_tokens",
            Api::Chat => "max_completion_tokens",
        };
        body.insert(field.into(), json!(tokens));
    }
    if let Some(text) = args.option("max-price") {
        let (input, output) = text
            .split_once('/')
            .ok_or("--max-price takes IN/OUT dollars per million tokens, such as 0.50/2.00")?;
        let mut max_price = Map::new();
        for (name, value) in [("input", input), ("output", output)] {
            let value = value.trim().trim_start_matches('$');
            if value.is_empty() {
                continue;
            }
            if value.parse::<f64>().is_err() {
                return Err(format!("--max-price: {value} is not a dollar amount"));
            }
            max_price.insert(name.into(), Value::String(value.to_owned()));
        }
        let options = body
            .entry("openagents")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .ok_or("--json's `openagents` must be an object")?;
        options.insert("max_price".into(), Value::Object(max_price));
    }
    if args.switch("stream") || format == Format::Events {
        body.insert("stream".into(), Value::Bool(true));
        if api == Api::Chat {
            body.insert("stream_options".into(), json!({"include_usage": true}));
        }
    }
    Ok(Call {
        base: DEFAULT_BASE.to_owned(),
        key: None,
        api,
        format,
        body: Value::Object(body),
    })
}

fn client(timeout: Option<Duration>) -> Result<reqwest::blocking::Client, String> {
    let mut builder = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(15));
    builder = builder.timeout(timeout);
    builder.build().map_err(|error| error.to_string())
}

fn get_json(url: &str, key: Option<&str>) -> Result<Value, String> {
    let mut request = client(Some(Duration::from_secs(30)))?.get(url);
    if let Some(key) = key {
        request = request.bearer_auth(key);
    }
    let response = request
        .send()
        .map_err(|error| format!("GET {url}: {error}"))?;
    let status = response.status().as_u16();
    let text = response.text().map_err(|error| error.to_string())?;
    let value: Value = serde_json::from_str(&text).unwrap_or(Value::String(text));
    if status >= 400 {
        return Err(refusal(status, &value, false));
    }
    Ok(value)
}

/// One plain sentence for a refusal.
fn refusal(status: u16, body: &Value, keyless: bool) -> String {
    let message = body["error"]["message"]
        .as_str()
        .or_else(|| body["error"].as_str())
        .or_else(|| body.as_str())
        .unwrap_or("no message")
        .to_owned();
    if status == 402 && keyless {
        return format!(
            "{message} (Set OPENAGENTS_API_KEY to use your account's credit, or add --pay x402 --max-msat N to pay for this request.)"
        );
    }
    if status == 401 && keyless {
        return format!("{message} (Set OPENAGENTS_API_KEY, or pass --key.)");
    }
    format!("{status}: {message}")
}

/// Send `call` and print its answer to `out`. `signature` is an x402
/// `PAYMENT-SIGNATURE` for a paid retry.
pub(crate) fn send(
    call: &Call,
    out: &mut dyn Write,
    signature: Option<&str>,
) -> Result<(), String> {
    let body = serde_json::to_vec(&call.body).map_err(|error| error.to_string())?;
    let reply = post(call, &body, signature)?;
    print_reply(call, reply, out)
}

fn post(
    call: &Call,
    body: &[u8],
    signature: Option<&str>,
) -> Result<reqwest::blocking::Response, String> {
    let url = call.url();
    let mut request = client(None)?
        .post(&url)
        .header("content-type", "application/json")
        .body(body.to_vec());
    if let Some(key) = &call.key {
        request = request.bearer_auth(key);
    }
    if let Some(signature) = signature {
        request = request.header("PAYMENT-SIGNATURE", signature);
    }
    request
        .send()
        .map_err(|error| format!("POST {url}: {error}"))
}

fn print_reply(
    call: &Call,
    reply: reqwest::blocking::Response,
    out: &mut dyn Write,
) -> Result<(), String> {
    let status = reply.status().as_u16();
    if status >= 400 {
        let text = reply.text().unwrap_or_default();
        let value: Value = serde_json::from_str(&text).unwrap_or(Value::String(text));
        return Err(refusal(status, &value, call.key.is_none()));
    }
    let streamed = reply
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("text/event-stream"));
    if streamed {
        return print_stream(call, reply, out);
    }
    let value: Value = reply
        .json()
        .map_err(|error| format!("the answer isn't JSON: {error}"))?;
    let write = |out: &mut dyn Write, text: String| {
        writeln!(out, "{text}").map_err(|error| error.to_string())
    };
    match call.format {
        Format::Json | Format::Events => write(
            out,
            serde_json::to_string_pretty(&value).unwrap_or_default(),
        ),
        Format::Text => write(out, answer_text(call.api, &value)),
    }
}

/// The words of a finished answer.
fn answer_text(api: Api, value: &Value) -> String {
    match api {
        Api::Chat => value["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
        Api::Responses => value["output"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|item| item["type"] == "message")
            .flat_map(|item| item["content"].as_array().into_iter().flatten())
            .filter(|part| part["type"] == "output_text")
            .filter_map(|part| part["text"].as_str())
            .collect::<Vec<_>>()
            .join(""),
    }
}

/// Read server-sent events as they arrive.
fn print_stream(call: &Call, reply: impl Read, out: &mut dyn Write) -> Result<(), String> {
    let reader = BufReader::new(reply);
    let mut last: Option<Value> = None;
    let mut chat = ChatFold::default();
    let mut wrote_text = false;
    let mut data = String::new();
    let mut failure: Option<String> = None;
    let mut handle = |data: &str, out: &mut dyn Write| -> Result<(), String> {
        if data.is_empty() || data == "[DONE]" {
            return Ok(());
        }
        let Ok(event) = serde_json::from_str::<Value>(data) else {
            return Ok(());
        };
        match call.format {
            Format::Events => {
                writeln!(out, "{event}").map_err(|error| error.to_string())?;
            }
            Format::Text => {
                let delta = match call.api {
                    Api::Responses if event["type"] == "response.output_text.delta" => {
                        event["delta"].as_str()
                    }
                    Api::Chat => event["choices"][0]["delta"]["content"].as_str(),
                    Api::Responses => None,
                };
                if let Some(delta) = delta {
                    write!(out, "{delta}").map_err(|error| error.to_string())?;
                    out.flush().map_err(|error| error.to_string())?;
                    wrote_text = true;
                }
            }
            Format::Json => {}
        }
        if call.api == Api::Chat {
            chat.push(&event);
        }
        if matches!(
            event["type"].as_str(),
            Some("response.failed") | Some("error")
        ) {
            failure = Some(
                event["response"]["error"]["message"]
                    .as_str()
                    .or_else(|| event["error"]["message"].as_str())
                    .or_else(|| event["message"].as_str())
                    .unwrap_or("the answer failed")
                    .to_owned(),
            );
        }
        if event["response"].is_object() {
            last = Some(event["response"].clone());
        }
        Ok(())
    };
    for line in reader.lines() {
        let line = line.map_err(|error| format!("the stream broke: {error}"))?;
        if line.is_empty() {
            handle(&std::mem::take(&mut data), out)?;
        } else if let Some(rest) = line.strip_prefix("data:") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(rest.strip_prefix(' ').unwrap_or(rest));
        }
    }
    handle(&std::mem::take(&mut data), out)?;
    match call.format {
        Format::Text if wrote_text => {
            writeln!(out).map_err(|error| error.to_string())?;
        }
        Format::Json => {
            let value = match call.api {
                Api::Responses => last.unwrap_or(Value::Null),
                Api::Chat => chat.completion(),
            };
            writeln!(
                out,
                "{}",
                serde_json::to_string_pretty(&value).unwrap_or_default()
            )
            .map_err(|error| error.to_string())?;
        }
        _ => {}
    }
    match failure {
        Some(message) => Err(message),
        None => Ok(()),
    }
}

/// Chat Completions chunks folded into one completion, for `--format json`.
#[derive(Default)]
struct ChatFold {
    id: Option<Value>,
    model: Option<Value>,
    content: String,
    finish: Option<Value>,
    usage: Option<Value>,
}

impl ChatFold {
    fn push(&mut self, chunk: &Value) {
        if self.id.is_none() && !chunk["id"].is_null() {
            self.id = Some(chunk["id"].clone());
        }
        if self.model.is_none() && !chunk["model"].is_null() {
            self.model = Some(chunk["model"].clone());
        }
        if let Some(text) = chunk["choices"][0]["delta"]["content"].as_str() {
            self.content.push_str(text);
        }
        if !chunk["choices"][0]["finish_reason"].is_null() {
            self.finish = Some(chunk["choices"][0]["finish_reason"].clone());
        }
        if chunk["usage"].is_object() {
            self.usage = Some(chunk["usage"].clone());
        }
    }

    fn completion(&self) -> Value {
        json!({
            "id": self.id,
            "object": "chat.completion",
            "model": self.model,
            "choices": [{"index": 0, "message": {"role": "assistant", "content": self.content},
                         "finish_reason": self.finish}],
            "usage": self.usage,
        })
    }
}

/// Pay for one keyless request over x402: send it, pay the `402`'s
/// invoice from the wallet within `--max-msat`, and send the same bytes
/// again with the proof.
#[cfg(unix)]
fn pay_x402(args: &Args, call: &Call, out: &mut dyn Write) -> Result<(), String> {
    use crate::x402::{Spend, buy, ceiling_present, flags, payer};
    if call.key.is_some() {
        return Err(
            "--pay x402 is for requests with no key; unset OPENAGENTS_API_KEY or drop --key".into(),
        );
    }
    let flags = flags(args).and_then(|flags| ceiling_present(flags).map(|()| flags))?;
    let payer = payer(args, false)?;
    let wait: u64 = args.number("wait", 60)?;
    let body = serde_json::to_vec(&call.body).map_err(|error| error.to_string())?;
    let url = call.url();
    let request_hash = nostr::x402::http_binding("POST", &url, &body, &[])
        .and_then(|binding| nostr::x402::binding_hash(&binding))
        .map_err(|_| format!("{url} can't be bound to a payment"))?;
    let first = post(call, &body, None)?;
    if first.status().as_u16() != 402 {
        return print_reply(call, first, out);
    }
    let required = first
        .headers()
        .get(openagents_x402::PAYMENT_REQUIRED)
        .and_then(|value| value.to_str().ok())
        .ok_or("the API answered 402 without x402 terms")?;
    let required = openagents_x402::wire::decode_payment_required(required)
        .map_err(|error| format!("PAYMENT-REQUIRED: {error}"))?;
    if required.resource.url != url {
        return Err("the payment terms name a different URL than the one called".into());
    }
    let spend = Spend {
        flags,
        capability: None,
        wait,
        binding: "http:1",
        resource: url.clone(),
        payer,
    };
    let (payload, proof, amount) = buy(
        &required,
        &request_hash,
        openagents_x402::facilitator::HTTP_ONLY,
        "http:1",
        None,
        spend,
    )?;
    let signature =
        openagents_x402::wire::encode_header(&payload).map_err(|error| error.to_string())?;
    eprintln!(
        "Paid {} sats (payment {}).",
        amount / 1_000,
        proof.payment_hash
    );
    let reply = post(call, &body, Some(&signature))?;
    print_reply(call, reply, out)
}

#[cfg(not(unix))]
fn pay_x402(_args: &Args, _call: &Call, _out: &mut dyn Write) -> Result<(), String> {
    Err("--pay x402 needs the wallet, which this platform doesn't have".into())
}

fn render_models(value: &Value) -> String {
    let mut rows = vec![vec![
        "MODEL".to_owned(),
        "PROVIDERS".to_owned(),
        "INPUT $/M".to_owned(),
        "OUTPUT $/M".to_owned(),
        "CONTEXT".to_owned(),
    ]];
    for model in value["data"].as_array().into_iter().flatten() {
        let providers: Vec<&Value> = model["openagents"]["providers"]
            .as_array()
            .map(|providers| providers.iter().collect())
            .unwrap_or_default();
        let names: Vec<&str> = providers
            .iter()
            .filter_map(|provider| provider["provider"].as_str())
            .collect();
        let cheapest = |field: &str| {
            providers
                .iter()
                .flat_map(|provider| provider["prices"].as_array().into_iter().flatten())
                .filter_map(|price| price[field]["price_usd"].as_str())
                .filter_map(|price| price.parse::<f64>().ok().map(|n| (n, price)))
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, price)| price.to_owned())
                .unwrap_or_else(|| "-".to_owned())
        };
        let context = providers
            .iter()
            .filter_map(|provider| provider["context"].as_u64())
            .max()
            .map_or_else(|| "-".to_owned(), |n| n.to_string());
        let mut id = model["id"].as_str().unwrap_or_default().to_owned();
        if model["openagents"]["free"] == true {
            id.push_str(" (free)");
        }
        rows.push(vec![
            id,
            if names.is_empty() {
                "-".to_owned()
            } else {
                names.join(", ")
            },
            cheapest("input"),
            cheapest("output"),
            context,
        ]);
    }
    crate::out::table(&rows)
}

fn render_rates(value: &Value) -> String {
    let mut rows = vec![vec![
        "MODEL".to_owned(),
        "PROVIDER".to_owned(),
        "INPUT $/M".to_owned(),
        "OUTPUT $/M".to_owned(),
        "MARGIN".to_owned(),
    ]];
    for row in value["rows"].as_array().into_iter().flatten() {
        let mut model = row["model"].as_str().unwrap_or_default().to_owned();
        if let Some(label) = row["label"].as_str() {
            model.push_str(&format!(" ({label})"));
        }
        rows.push(vec![
            model,
            row["provider"].as_str().unwrap_or_default().to_owned(),
            row["input"]["price_usd"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            row["output"]["price_usd"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            format!("{}%", row["margin_percent"].as_str().unwrap_or("?")),
        ]);
    }
    crate::out::table(&rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    use axum::Router;
    use axum::body::Bytes;
    use axum::http::{HeaderMap, StatusCode};
    use axum::response::IntoResponse;
    use axum::routing::{get, post};

    /// A stub API on its own thread: answers like the gateway and keeps
    /// what it was sent.
    struct Stub {
        base: String,
        seen: Arc<Mutex<Vec<(Option<String>, Value)>>>,
    }

    fn stub() -> Stub {
        let seen: Arc<Mutex<Vec<(Option<String>, Value)>>> = Arc::default();
        let kept = seen.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                let keep = move |headers: &HeaderMap, body: &Bytes| -> Value {
                    let key = headers
                        .get("authorization")
                        .and_then(|value| value.to_str().ok())
                        .map(str::to_owned);
                    let value: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
                    kept.lock().unwrap().push((key, value.clone()));
                    value
                };
                let keep_chat = keep.clone();
                let app = Router::new()
                    .route(
                        "/v1/responses",
                        post(move |headers: HeaderMap, body: Bytes| {
                            let request = keep(&headers, &body);
                            async move {
                                if request["model"] == "broke/model" {
                                    return (
                                        StatusCode::PAYMENT_REQUIRED,
                                        axum::Json(json!({"error": {"type": "payment_required",
                                            "message": "This request costs up to 3 sats."}})),
                                    )
                                        .into_response();
                                }
                                let model = request["model"].clone();
                                if request["stream"] == true {
                                    let events = [
                                        json!({"type": "response.created", "sequence_number": 0,
                                               "response": {"id": "resp_1", "status": "in_progress", "model": model, "output": []}}),
                                        json!({"type": "response.output_text.delta", "sequence_number": 1, "delta": "hel"}),
                                        json!({"type": "response.output_text.delta", "sequence_number": 2, "delta": "lo"}),
                                        json!({"type": "response.completed", "sequence_number": 3,
                                               "response": {"id": "resp_1", "status": "completed", "model": model,
                                                            "output": [{"type": "message", "content": [{"type": "output_text", "text": "hello"}]}]}}),
                                    ];
                                    let mut text = String::new();
                                    for event in events {
                                        text.push_str(&format!("event: {}\ndata: {event}\n\n", event["type"].as_str().unwrap()));
                                    }
                                    text.push_str("data: [DONE]\n\n");
                                    return ([("content-type", "text/event-stream")], text).into_response();
                                }
                                axum::Json(json!({"id": "resp_1", "object": "response", "status": "completed",
                                    "model": model,
                                    "output": [{"type": "reasoning", "summary": []},
                                               {"type": "message", "content": [{"type": "output_text", "text": "hel"},
                                                                               {"type": "output_text", "text": "lo"}]}]}))
                                .into_response()
                            }
                        }),
                    )
                    .route(
                        "/v1/chat/completions",
                        post(move |headers: HeaderMap, body: Bytes| {
                            let request = keep_chat(&headers, &body);
                            async move {
                                if request["stream"] == true {
                                    let mut text = String::new();
                                    for (index, piece) in ["hi", " there"].iter().enumerate() {
                                        let chunk = json!({"id": "c1", "object": "chat.completion.chunk", "model": request["model"],
                                            "choices": [{"index": 0, "delta": {"content": piece}, "finish_reason": if index == 1 { json!("stop") } else { Value::Null }}]});
                                        text.push_str(&format!("data: {chunk}\n\n"));
                                    }
                                    text.push_str(&format!("data: {}\n\n", json!({"id": "c1", "object": "chat.completion.chunk", "choices": [], "usage": {"total_tokens": 7}})));
                                    text.push_str("data: [DONE]\n\n");
                                    return ([("content-type", "text/event-stream")], text).into_response();
                                }
                                axum::Json(json!({"id": "c1", "object": "chat.completion",
                                    "choices": [{"index": 0, "message": {"role": "assistant", "content": "hi there"}, "finish_reason": "stop"}]}))
                                .into_response()
                            }
                        }),
                    )
                    .route(
                        "/v1/models",
                        get(|| async {
                            axum::Json(json!({"object": "list", "data": [
                                {"id": "google/gemini-3.8-flash", "object": "model",
                                 "openagents": {"providers": [{"provider": "Google", "context": 1_000_000,
                                     "prices": [{"input": {"price_usd": "0.315"}, "output": {"price_usd": "2.625"}}]}]}},
                                {"id": "openai/gpt-5.6-luna", "object": "model", "openagents": {"free": true, "providers": []}},
                                {"id": "openagents/chat", "object": "model"}
                            ]}))
                        }),
                    )
                    .route(
                        "/v1/rates",
                        get(|| async {
                            axum::Json(json!({"v": "1", "rows": [
                                {"model": "google/gemini-3.8-flash", "provider": "Google", "margin_percent": "5",
                                 "input": {"price_usd": "0.315"}, "output": {"price_usd": "2.625"}}
                            ]}))
                        }),
                    );
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                tx.send(listener.local_addr().unwrap()).unwrap();
                axum::serve(listener, app).await.unwrap();
            });
        });
        let address = rx.recv().unwrap();
        Stub {
            base: format!("http://{address}/v1"),
            seen,
        }
    }

    fn words(text: &[&str]) -> Vec<String> {
        text.iter().map(|word| (*word).to_owned()).collect()
    }

    fn call_with(stub: &Stub, text: &[&str]) -> Call {
        let args = Args::parse(&words(text), SWITCHES).unwrap();
        let model = resolve_model(&stub.base, None, &args.positional()[0]);
        let call = build(&args, &model, false, &mut std::io::empty()).unwrap();
        Call {
            base: stub.base.clone(),
            key: Some("oak_test.secret".into()),
            ..call
        }
    }

    fn printed(stub: &Stub, text: &[&str]) -> Result<String, String> {
        let call = call_with(stub, text);
        let mut out = Vec::new();
        send(&call, &mut out, None)?;
        Ok(String::from_utf8(out).unwrap())
    }

    #[test]
    fn a_prompt_prints_the_answers_words() {
        let stub = stub();
        let out = printed(&stub, &["openagents/chat", "Say hello."]).unwrap();
        assert_eq!(out, "hello\n");
        let (key, sent) = stub.seen.lock().unwrap()[0].clone();
        assert_eq!(key.as_deref(), Some("Bearer oak_test.secret"));
        assert_eq!(
            sent,
            json!({"model": "openagents/chat", "input": "Say hello."})
        );
    }

    #[test]
    fn a_bare_model_name_is_the_catalogs_id() {
        let stub = stub();
        printed(&stub, &["gemini-3.8-flash", "--input", "Hi"]).unwrap();
        let sent = stub.seen.lock().unwrap()[0].1.clone();
        assert_eq!(sent["model"], "google/gemini-3.8-flash");
    }

    #[test]
    fn a_json_body_is_sent_whole_with_the_model_filled_in() {
        let stub = stub();
        let out = printed(
            &stub,
            &[
                "google/gemini-3.8-flash",
                "--json",
                r#"{"input": "Hi", "temperature": 0.2, "openagents": {"privacy": "strict"}}"#,
                "--max-output-tokens",
                "64",
                "--max-price",
                "0.50/2.00",
                "--format",
                "json",
            ],
        )
        .unwrap();
        let answer: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(answer["status"], "completed");
        let sent = stub.seen.lock().unwrap()[0].1.clone();
        assert_eq!(
            sent,
            json!({"model": "google/gemini-3.8-flash", "input": "Hi", "temperature": 0.2,
                   "max_output_tokens": 64,
                   "openagents": {"privacy": "strict", "max_price": {"input": "0.50", "output": "2.00"}}})
        );
    }

    #[test]
    fn streams_print_words_as_they_arrive() {
        let stub = stub();
        let out = printed(&stub, &["openagents/fast", "Hi", "--stream"]).unwrap();
        assert_eq!(out, "hello\n");
        assert_eq!(stub.seen.lock().unwrap()[0].1["stream"], true);
        let events = printed(&stub, &["openagents/fast", "Hi", "--format", "events"]).unwrap();
        let lines: Vec<Value> = events
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[3]["type"], "response.completed");
        let folded = printed(
            &stub,
            &["openagents/fast", "Hi", "--stream", "--format", "json"],
        )
        .unwrap();
        let folded: Value = serde_json::from_str(&folded).unwrap();
        assert_eq!(folded["status"], "completed");
    }

    #[test]
    fn chat_completions_stream_and_fold() {
        let stub = stub();
        let out = printed(
            &stub,
            &[
                "openagents/chat",
                "Hi",
                "--api",
                "chat",
                "--instructions",
                "Be brief.",
                "--stream",
            ],
        )
        .unwrap();
        assert_eq!(out, "hi there\n");
        let sent = stub.seen.lock().unwrap()[0].1.clone();
        assert_eq!(
            sent["messages"],
            json!([{"role": "system", "content": "Be brief."}, {"role": "user", "content": "Hi"}])
        );
        assert_eq!(sent["stream_options"]["include_usage"], true);
        let folded = printed(
            &stub,
            &[
                "openagents/chat",
                "Hi",
                "--api",
                "chat",
                "--stream",
                "--format",
                "json",
            ],
        )
        .unwrap();
        let folded: Value = serde_json::from_str(&folded).unwrap();
        assert_eq!(folded["choices"][0]["message"]["content"], "hi there");
        assert_eq!(folded["choices"][0]["finish_reason"], "stop");
        assert_eq!(folded["usage"]["total_tokens"], 7);
        let plain = printed(&stub, &["openagents/chat", "Hi", "--api", "chat"]).unwrap();
        assert_eq!(plain, "hi there\n");
    }

    #[test]
    fn a_refusal_is_one_plain_sentence() {
        let stub = stub();
        let mut call = call_with(&stub, &["broke/model", "Hi"]);
        call.key = None;
        let error = send(&call, &mut Vec::new(), None).unwrap_err();
        assert!(
            error.starts_with("This request costs up to 3 sats."),
            "{error}"
        );
        assert!(error.contains("--pay x402"), "{error}");
    }

    #[test]
    fn bad_options_are_usage_errors() {
        let parse = |text: &[&str]| {
            let args = Args::parse(&words(text), SWITCHES).unwrap();
            build(&args, "m/x", false, &mut std::io::empty())
        };
        assert!(parse(&["m/x"]).is_err(), "no prompt");
        assert!(parse(&["m/x", "Hi", "--input", "Hi"]).is_err());
        assert!(parse(&["m/x", "Hi", "--api", "nope"]).is_err());
        assert!(parse(&["m/x", "Hi", "--format", "nope"]).is_err());
        assert!(parse(&["m/x", "--json", "[1]"]).is_err());
        assert!(parse(&["m/x", "Hi", "--max-price", "cheap"]).is_err());
        let stdin = parse_stdin(&["m/x", "-"], "from stdin");
        assert_eq!(stdin.body["input"], "from stdin");
        let global_json = Args::parse(&words(&["m/x", "Hi"]), SWITCHES).unwrap();
        let call = build(&global_json, "m/x", true, &mut std::io::empty()).unwrap();
        assert_eq!(call.format, Format::Json);
    }

    fn parse_stdin(text: &[&str], input: &str) -> Call {
        let args = Args::parse(&words(text), SWITCHES).unwrap();
        build(&args, "m/x", false, &mut input.as_bytes()).unwrap()
    }

    #[test]
    fn models_and_rates_print_tables() {
        let stub = stub();
        let models = get_json(&format!("{}/models", stub.base), None).unwrap();
        let table = render_models(&models);
        assert!(table.contains("google/gemini-3.8-flash"));
        assert!(table.contains("Google"));
        assert!(table.contains("2.625"));
        assert!(table.contains("openai/gpt-5.6-luna (free)"));
        let rates = get_json(&format!("{}/rates", stub.base), None).unwrap();
        let table = render_rates(&rates);
        assert!(table.contains("0.315"));
        assert!(table.contains("5%"));
    }
}
