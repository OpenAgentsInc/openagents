//! Python's number formatting, so messages read exactly as the Lab's always
//! have (`{x:g}`, `{x:.2f}`, `str(x)` of a JSON number).

use serde_json::Value;

/// `format(x, "g")`: six significant digits, trailing zeros dropped,
/// scientific outside 1e-4 ..< 1e6.
pub fn g(x: f64) -> String {
    if x == 0.0 {
        return if x.is_sign_negative() {
            "-0".into()
        } else {
            "0".into()
        };
    }
    if !x.is_finite() {
        return if x.is_nan() {
            "nan".into()
        } else if x > 0.0 {
            "inf".into()
        } else {
            "-inf".into()
        };
    }
    let exp = format!("{:.5e}", x);
    let (mantissa, e) = exp.split_once('e').unwrap_or((&exp, "0"));
    let e: i32 = e.parse().unwrap_or(0);
    if !(-4..6).contains(&e) {
        let m = trim(mantissa);
        return format!("{m}e{}{:02}", if e < 0 { '-' } else { '+' }, e.abs());
    }
    let decimals = (5 - e).max(0) as usize;
    trim(&format!("{x:.decimals$}")).to_owned()
}

fn trim(s: &str) -> &str {
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.')
    } else {
        s
    }
}

/// `f"{x:.2f}"`.
pub fn f2(x: f64) -> String {
    format!("{x:.2}")
}

/// Python's `str()` of a JSON value as `json.loads` gives it (for schema
/// messages: `1`, `0.5`, `cut.v1`).
pub fn py(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.to_string()
            } else if let Some(u) = n.as_u64() {
                u.to_string()
            } else {
                let f = n.as_f64().unwrap_or_default();
                let s = format!("{f:?}");
                if s.contains('e') { s } else { s }
            }
        }
        Value::Bool(b) => {
            if *b {
                "True".into()
            } else {
                "False".into()
            }
        }
        Value::Null => "None".into(),
        other => other.to_string(),
    }
}

/// The Lab's clock: `m:ss.s`, or `m:ss` on whole seconds.
pub fn clock(t: f64) -> String {
    let t = t.max(0.0);
    let m = (t / 60.0).floor();
    let s = t - m * 60.0;
    if s.fract() != 0.0 {
        format!("{}:{:04.1}", m as u64, s)
    } else {
        format!("{}:{:02}", m as u64, s as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn g_matches_python() {
        for (x, want) in [
            (12.5, "12.5"),
            (12.0, "12"),
            (0.4, "0.4"),
            (2.0 / 3.0, "0.666667"),
            (1234567.0, "1.23457e+06"),
            (0.0001, "0.0001"),
            (0.00001, "1e-05"),
            (89.25, "89.25"),
            (100000.0, "100000"),
            (-3.5, "-3.5"),
        ] {
            assert_eq!(g(x), want, "{x}");
        }
    }

    #[test]
    fn clock_matches_python() {
        assert_eq!(clock(86.0), "1:26");
        assert_eq!(clock(86.5), "1:26.5");
        assert_eq!(clock(5.0), "0:05");
        assert_eq!(clock(-1.0), "0:00");
    }
}
