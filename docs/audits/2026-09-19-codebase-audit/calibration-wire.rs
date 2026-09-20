//! Observe whether calibration and a served answer describe the same outcome.
//! This probe prints observations; it does not require a defect to persist.
use gym::calibrate::{Map, Observation};
use gym::eval::{Disposition, mapped_observations, read_answer};
use gym::row::Row;
use lev::Kind;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let map = Map::fit(&[Observation::new(0.8, false)], 1);
    let cases = [
        (
            "choice",
            Kind::Choice,
            vec![("yes", 0.8), ("no", 0.2)],
            "no",
        ),
        ("noul", Kind::Noul, vec![("no", 0.2), ("yes", 0.8)], "no"),
        (
            "score",
            Kind::Score,
            vec![("0", 0.8), ("1", 0.15), ("2", 0.05)],
            "1",
        ),
    ];
    for (name, kind, pairs, truth) in cases {
        let raw = pairs
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect();
        let selected = lev::estimator::argmax(&raw)?;
        let mapped = map.apply_distribution(&raw);
        let typed = lev::estimator::answer(kind, &mapped, &Default::default(), &selected)?;
        let bytes = serde_json::to_vec(&serde_json::json!({
            "model": "audit-local", "answers": {"q": typed}, "usage": {}
        }))?;
        let response = jev::SystemOneResponse::decode(jev::RawResponse {
            status: 200,
            headers: Default::default(),
            bytes,
        })?;
        let Disposition::Answered { chosen, .. } = read_answer(&response.answers["q"]) else {
            return Err("the served answer was not decoded as an answer".into());
        };
        let row = Row::new("audit", "audit", name, "audit").scored(raw, selected == truth);
        let observations = mapped_observations(&[row], &map);
        let observation = observations.first().ok_or("no mapped observation")?;
        println!(
            "kind={name} raw_selected={selected} mapped_metric_correct={} wire_selected={chosen} wire_correct={}",
            observation.correct,
            chosen == truth,
        );
    }
    Ok(())
}
