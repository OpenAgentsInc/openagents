//! Brainstorm result copy from explicit, already-observed presentation values.
//! This projection performs no discovery, freshness check, or network call.

#[derive(Clone, Debug)]
pub struct Subject {
    pub pubkey: String,
    pub relevance: Option<f64>,
    pub influence: Option<f64>,
    pub coverage: Option<Coverage>,
    pub profile_url: String,
}
#[derive(Clone, Copy, Debug)]
pub enum Coverage {
    Unknown,
    Reported,
}
#[derive(Clone, Debug)]
pub struct Response {
    pub endpoint: String,
    pub status: u16,
    pub algorithm: String,
    pub fetched_ms: u64,
    pub expires_ms: u64,
    pub input_digest: String,
    pub output_digest: String,
}
#[derive(Clone, Debug)]
pub struct Observation {
    pub origin: String,
    pub house_key: String,
    pub discovered_ms: u64,
    pub expires_ms: u64,
    pub fresh: bool,
    pub partial: bool,
    pub subjects: Vec<Subject>,
    pub enrichment_error: Option<String>,
    pub responses: Vec<Response>,
}
#[derive(Clone, Debug)]
pub enum ResultDisplay {
    Demo,
    Observation(Observation),
    Discovery {
        origin: String,
        house_key: String,
        search: bool,
        rank: bool,
        discovered_ms: u64,
        expires_ms: u64,
    },
    Error {
        recipient: Option<String>,
        message: String,
    },
    Unavailable,
}

pub fn summary(result: &ResultDisplay) -> String {
    match result {
        ResultDisplay::Demo=>"Brainstorm demo fixture. No service read.\n\nPublic key: fixture account\nRelevance: 0.8 · Raw influence: 0.0 · Coverage: unknown\nHouse perspective and response times are fixture values, not live evidence.".into(),
        ResultDisplay::Observation(observation)=>{
            let mut text=format!("Brainstorm house perspective · {} · {}\n\nSource: {}\nHouse key: {}\nSeparate HTTPS observation; the API does not bind or sign its effective observer.\nDiscovered: {} ms · Combined expiry: {} ms\n\n| Public key | Relevance | Raw influence | Coverage | Profile |\n| --- | --- | --- | --- | --- |\n",if observation.partial {"partial scores"}else{"bounded request"},if observation.fresh {"fresh observation"}else{"expired observation"},observation.origin,observation.house_key,observation.discovered_ms,observation.expires_ms);
            for subject in &observation.subjects {
                let relevance=subject.relevance.map(|v|v.to_string()).unwrap_or_else(||"unavailable".into());let influence=subject.influence.map(|v|v.to_string()).unwrap_or_else(||"unavailable".into());
                let coverage=match subject.coverage {Some(Coverage::Unknown)=>"unknown",Some(Coverage::Reported)=>"reported",None=>"unavailable"};
                text.push_str(&format!("| {} | {relevance} | {influence} | {coverage} | {} |\n",subject.pubkey,subject.profile_url));
            }
            if let Some(error)=&observation.enrichment_error {text.push_str(&format!("\nInfluence unavailable: {error}.\n"));}
            text.push_str("\nRelevance and raw continuous influence are separate units; neither is a signed 0–100 score.\n");
            for response in &observation.responses {text.push_str(&format!("\n{} · HTTP {} · {} · fetched {} ms · expires {} ms\nInput hash: {}\nOutput hash: {}\n",response.endpoint,response.status,response.algorithm,response.fetched_ms,response.expires_ms,response.input_digest,response.output_digest));}text
        }
        ResultDisplay::Discovery{origin,house_key,search,rank,discovered_ms,expires_ms}=>format!("Brainstorm discovery from {origin}\nHouse key: {house_key}\nSearch available: {search} · Rank available: {rank}\nSeparate HTTPS identity observation at {discovered_ms} ms; not a signed or atomic observer binding.\nDiscovery expiry: {expires_ms} ms."),
        ResultDisplay::Error{recipient:Some(recipient),message}=>format!("Brainstorm lookup to {recipient}: {message}"),
        ResultDisplay::Error{recipient:None,message}=>message.clone(),ResultDisplay::Unavailable=>"Brainstorm result unavailable.".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn demo_keeps_public_source_units_and_attribution() {
        assert_eq!(
            summary(&ResultDisplay::Demo),
            "Brainstorm demo fixture. No service read.\n\nPublic key: fixture account\nRelevance: 0.8 · Raw influence: 0.0 · Coverage: unknown\nHouse perspective and response times are fixture values, not live evidence."
        );
        assert_eq!(
            summary(&ResultDisplay::Unavailable),
            "Brainstorm result unavailable."
        );
    }
}
