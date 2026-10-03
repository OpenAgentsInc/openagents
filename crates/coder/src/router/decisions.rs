//! Capture the gates policy actually evaluates, without rerunning inference.

use route_contract::decision::DecisionReading;
use serde_json::{Number, Value};
use std::cell::RefCell;

#[derive(Default)]
struct Capture {
    model: String,
    answers: Value,
    map: bool,
    readings: Vec<DecisionReading>,
}
thread_local! { static CAPTURE: RefCell<Option<Capture>> = const { RefCell::new(None) }; }

/// Run synchronous policy with a fresh capture. Shadow policy is captured
/// separately from the served policy. The guard also clears on panic.
pub fn capture<T>(
    response: &jev::SystemOneResponse,
    mapped: bool,
    run: impl FnOnce() -> T,
) -> (T, Vec<DecisionReading>) {
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            CAPTURE.with(|c| *c.borrow_mut() = None);
        }
    }
    let answers = serde_json::from_str::<Value>(&response.raw().text())
        .ok()
        .and_then(|v| v.get("answers").cloned())
        .unwrap_or_default();
    CAPTURE.with(|c| {
        *c.borrow_mut() = Some(Capture {
            model: response.model.clone(),
            answers,
            map: mapped,
            readings: Vec::new(),
        })
    });
    let guard = Guard;
    let result = run();
    let readings = CAPTURE.with(|c| c.borrow_mut().take().unwrap().readings);
    drop(guard);
    (result, readings)
}

/// Evaluate and retain one comparison, including its direction and boundary.
pub fn test(question: &str, site: &str, p: f64, threshold: f64, comparison: &str) -> bool {
    let decision = match comparison {
        "lt" => p < threshold,
        "le" => p <= threshold,
        "gt" => p > threshold,
        _ => p >= threshold,
    };
    CAPTURE.with(|c| {
        let mut c = c.borrow_mut();
        let Some(c) = c.as_mut() else { return };
        let answer = &c.answers[question];
        let option = match site {
            "CLARIFY_WINS" => Some("clarify"),
            "CAPABILITY_MISSING" => Some("none"),
            _ => answer["choice"].as_str(),
        };
        let raw = option
            .and_then(|o| answer["probabilities"][o].as_f64())
            .or_else(|| answer["noul"].as_f64());
        // Defaults used for an absent answer are not model probabilities.
        let Some(raw) = raw else { return };
        let Some(mut r) = DecisionReading::new(question, site, &c.model, raw, threshold, decision)
        else {
            return;
        };
        r.option = option.map(str::to_owned);
        r.policy_probability = Number::from_f64(p).unwrap();
        r.comparison = comparison.into();
        r.question_set = super::set_id();
        if c.map
            && matches!(question, "route" | "answer")
            && site != "CLARIFY_WINS"
            && site != "CLOSE_MARGIN"
        {
            r.calibrated_probability = Number::from_f64(p);
        }
        c.readings.push(r);
    });
    decision
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn capture_keeps_raw_mapped_threshold_direction_and_served_model() {
        let response = jev::SystemOneResponse::decode(jev::RawResponse {
            status: 200, headers: Default::default(),
            bytes: json!({"model":"jev-fixture-pinned", "answers": {
                "route": {"type":"choice", "choice":"general", "confidence":0.81, "probabilities":{"general":0.81,"clarify":0.19}},
                "read_only": {"type":"noul", "noul":0.7}
            }}).to_string().into_bytes()
        }).unwrap();
        let (_, readings) = capture(&response, true, || {
            assert!(!test("route", "ROUTE_CONFIDENCE", 0.75, 0.8, "ge"));
            assert!(test("read_only", "READ_ONLY_CONFIDENCE", 0.7, 0.7, "ge"));
            assert!(!test("read_only", "exclusive", 0.7, 0.7, "lt"));
            test("missing", "unknown", 0.0, 0.5, "ge");
        });
        assert_eq!(readings.len(), 3);
        let r = &readings[0];
        assert_eq!(r.raw_probability.as_f64(), Some(0.81));
        assert_eq!(
            r.calibrated_probability.as_ref().and_then(Number::as_f64),
            Some(0.75)
        );
        assert_eq!(r.threshold.as_f64(), Some(0.8));
        assert_eq!(r.model, "jev-fixture-pinned");
        assert!(!r.decision);
        assert_eq!(readings[2].comparison, "lt");
        let (_, next) = capture(&response, false, || {
            test("route", "ROUTE_CONFIDENCE", 0.81, 0.8, "ge")
        });
        assert_eq!(next.len(), 1);
        assert!(next[0].calibrated_probability.is_none());
    }
}
