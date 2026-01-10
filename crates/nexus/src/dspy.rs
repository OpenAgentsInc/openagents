//! DSPy signatures for Nexus semantic classification.

use dsrs::{example, Predict, Prediction, Predictor, Signature, GLOBAL_SETTINGS};

#[derive(Debug, Clone)]
pub struct EventIntentResult {
    pub intent: String,
    pub priority: String,
    pub requires_response: bool,
}

#[derive(Debug, Clone)]
pub struct JobKindResult {
    pub job_type: String,
    pub complexity: String,
}

#[Signature]
pub struct EventIntentClassifier {
    /// Classify Nostr event intent.

    /// Event kind
    #[input]
    pub event_kind: u32,

    /// Event content
    #[input]
    pub event_content: String,

    /// Event tags (JSON array)
    #[input]
    pub event_tags: String,

    /// Intent label
    #[output]
    pub intent: String,

    /// Priority label
    #[output]
    pub priority: String,

    /// Whether a response is required
    #[output]
    pub requires_response: bool,
}

#[Signature]
pub struct JobKindClassifier {
    /// Classify NIP-90 job types.

    /// Job content payload
    #[input]
    pub job_content: String,

    /// Job params/tags (JSON array)
    #[input]
    pub job_params: String,

    /// Job type label
    #[output]
    pub job_type: String,

    /// Complexity label
    #[output]
    pub complexity: String,
}

fn dspy_ready() -> bool {
    GLOBAL_SETTINGS.read().unwrap().is_some()
}

fn get_string(prediction: &Prediction, key: &str) -> String {
    let val = prediction.get(key, None);
    if let Some(s) = val.as_str() {
        s.to_string()
    } else {
        val.to_string().trim_matches('"').to_string()
    }
}

fn get_bool(prediction: &Prediction, key: &str) -> bool {
    let val = prediction.get(key, None);
    if let Some(b) = val.as_bool() {
        b
    } else if let Some(s) = val.as_str() {
        matches!(s.to_lowercase().as_str(), "true" | "yes" | "1")
    } else {
        false
    }
}

fn format_tags(tags: &[Vec<String>]) -> String {
    serde_json::to_string(tags).unwrap_or_default()
}

pub async fn classify_event_intent(event: &nostr::Event) -> Option<EventIntentResult> {
    if !dspy_ready() {
        return None;
    }

    let classifier = Predict::new(EventIntentClassifier::new());
    let example = example! {
        "event_kind": "input" => event.kind as u32,
        "event_content": "input" => event.content.clone(),
        "event_tags": "input" => format_tags(&event.tags),
    };

    let prediction = classifier.forward(example).await.ok()?;
    Some(EventIntentResult {
        intent: get_string(&prediction, "intent"),
        priority: get_string(&prediction, "priority"),
        requires_response: get_bool(&prediction, "requires_response"),
    })
}

pub async fn classify_job_kind(job_content: &str, job_params: &str) -> Option<JobKindResult> {
    if !dspy_ready() {
        return None;
    }

    let classifier = Predict::new(JobKindClassifier::new());
    let example = example! {
        "job_content": "input" => job_content.to_string(),
        "job_params": "input" => job_params.to_string(),
    };

    let prediction = classifier.forward(example).await.ok()?;
    Some(JobKindResult {
        job_type: get_string(&prediction, "job_type"),
        complexity: get_string(&prediction, "complexity"),
    })
}

pub async fn classify_job_kind_for_event(event: &nostr::Event) -> Option<JobKindResult> {
    classify_job_kind(&event.content, &format_tags(&event.tags)).await
}
