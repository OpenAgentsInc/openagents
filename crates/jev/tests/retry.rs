//! Retries: the defaults both official SDKs ship, the delay for one attempt,
//! the server delay headers, and what each field refuses.

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use jev::{ApiError, ApiErrorKind, Error, RetryPolicy, parse_retry_after, parse_retry_after_at};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

type Outcome = Result<(), Box<dyn std::error::Error>>;

/// Headers with one value, as a response sends them.
fn headers(name: &'static str, value: &str) -> Result<HeaderMap, Box<dyn std::error::Error>> {
    let mut headers = HeaderMap::new();
    headers.insert(HeaderName::from_static(name), HeaderValue::from_str(value)?);
    Ok(headers)
}

/// One failed response, for the tests that ask whether a status is retried.
fn api(status: u16) -> Error {
    Error::from(ApiError {
        status,
        headers: HeaderMap::new(),
        body: None,
        request_id: None,
        endpoint: "POST https://api.typesafe.ai/v1/systemone".to_string(),
        kind: ApiErrorKind::of(status, &HeaderMap::new()),
    })
}

#[test]
fn the_defaults_are_the_ones_both_official_sdks_ship() {
    let policy = RetryPolicy::default();
    assert_eq!(policy.max_retries, 2);
    assert_eq!(policy.backoff_initial, Duration::from_millis(500));
    assert_eq!(policy.backoff_max, Duration::from_secs(5));
    assert!((policy.backoff_jitter - 0.25).abs() < f64::EPSILON);
    assert!(policy.respect_retry_after);
    assert_eq!(policy.max_retry_after, Duration::from_secs(60));
    assert!(policy.connection_errors);
    assert!(policy.timeouts);
    assert_eq!(policy.budget, None);
    assert!(policy.predicate.is_none());
    assert_eq!(policy.http_statuses.len(), 102);
}

#[test]
fn the_retried_statuses_are_408_429_and_every_5xx() {
    let policy = RetryPolicy::default();
    for status in [408, 429, 500, 503, 529, 599] {
        assert!(policy.retries_status(status), "{status}");
    }
    for status in [200, 400, 401, 403, 404, 422, 499, 600] {
        assert!(!policy.retries_status(status), "{status}");
    }
}

/// The delay doubles from the first value up to the cap. The random draw is
/// supplied so the reading is fixed.
#[test]
fn the_delay_doubles_up_to_the_cap() {
    let policy = RetryPolicy::default();
    let expected = [
        (0, Duration::from_millis(500)),
        (1, Duration::from_secs(1)),
        (2, Duration::from_secs(2)),
        (3, Duration::from_secs(4)),
        (4, Duration::from_secs(5)),
        (9, Duration::from_secs(5)),
        (64, Duration::from_secs(5)),
    ];
    for (attempt, delay) in expected {
        assert_eq!(policy.delay_with(attempt, None, 0.0), delay, "{attempt}");
    }
}

#[test]
fn the_jitter_takes_a_fraction_off_the_delay() {
    let policy = RetryPolicy::default();
    assert_eq!(
        policy.delay_with(0, None, 1.0),
        Duration::from_millis(375),
        "a full draw takes a quarter off"
    );
    assert_eq!(
        policy.delay_with(0, None, 0.5),
        Duration::from_millis(437) + Duration::from_micros(500)
    );
    assert_eq!(
        policy.delay_with(1, None, 0.5),
        Duration::from_millis(875),
        "attempt one doubles the base before the draw scales it"
    );
    let steady = RetryPolicy {
        backoff_jitter: 0.0,
        ..RetryPolicy::default()
    };
    assert_eq!(steady.delay_with(0, None, 1.0), Duration::from_millis(500));
    let drawn = policy.delay(0, None);
    assert!(drawn <= Duration::from_millis(500) && drawn >= Duration::from_millis(375));
}

#[test]
fn a_server_delay_replaces_the_computed_one() -> Outcome {
    let policy = RetryPolicy::default();
    let asked = headers("retry-after-ms", "1500")?;
    assert_eq!(
        policy.delay_with(0, Some(&asked), 0.0),
        Duration::from_millis(1500)
    );

    let seconds = headers("retry-after", "2")?;
    assert_eq!(
        policy.delay_with(0, Some(&seconds), 0.0),
        Duration::from_secs(2)
    );

    let long = headers("retry-after", "600")?;
    assert_eq!(
        policy.delay_with(0, Some(&long), 0.0),
        Duration::from_millis(500),
        "a delay past the longest honored one falls back to the computed delay"
    );

    let ignoring = RetryPolicy {
        respect_retry_after: false,
        ..RetryPolicy::default()
    };
    assert_eq!(
        ignoring.delay_with(0, Some(&asked), 0.0),
        Duration::from_millis(500)
    );
    Ok(())
}

#[test]
fn the_millisecond_header_wins_over_the_second_one() -> Outcome {
    let mut both = headers("retry-after-ms", "250")?;
    both.insert(
        HeaderName::from_static("retry-after"),
        HeaderValue::from_static("30"),
    );
    assert_eq!(parse_retry_after(&both), Some(Duration::from_millis(250)));
    Ok(())
}

#[test]
fn a_server_delay_reads_seconds_a_date_and_nothing() -> Outcome {
    assert_eq!(parse_retry_after(&HeaderMap::new()), None);
    assert_eq!(
        parse_retry_after(&headers("retry-after", "0")?),
        Some(Duration::ZERO)
    );
    assert_eq!(
        parse_retry_after(&headers("retry-after", " 3 ")?),
        Some(Duration::from_secs(3))
    );
    assert_eq!(
        parse_retry_after(&headers("retry-after", "-5")?),
        None,
        "a negative count asks for no wait"
    );
    assert_eq!(
        parse_retry_after(&headers("retry-after", "soon")?),
        None,
        "a value that is neither a count nor a date asks for no wait"
    );
    assert_eq!(
        parse_retry_after(&headers("retry-after-ms", "")?),
        Some(Duration::ZERO),
        "a blank count reads as zero"
    );
    assert_eq!(
        parse_retry_after(&headers("retry-after-ms", "-1")?),
        None,
        "an unreadable millisecond count falls through"
    );
    Ok(())
}

/// A millisecond count that cannot be read falls through to the seconds
/// header, and a seconds count reads fractions.
#[test]
fn an_unreadable_millisecond_count_falls_through_to_seconds() -> Outcome {
    for bad in ["NaN", "-1", "bad", "inf"] {
        let mut both = headers("retry-after-ms", bad)?;
        both.insert(
            HeaderName::from_static("retry-after"),
            HeaderValue::from_static("1.5"),
        );
        assert_eq!(
            parse_retry_after(&both),
            Some(Duration::from_millis(1500)),
            "retry-after-ms {bad:?} falls through to the seconds header"
        );
    }
    assert_eq!(
        parse_retry_after(&headers("retry-after-ms", "inf")?),
        None,
        "an infinite count asks for no wait"
    );
    assert_eq!(
        parse_retry_after(&headers("retry-after", "")?),
        Some(Duration::ZERO),
        "a blank seconds count reads as zero"
    );
    assert_eq!(
        parse_retry_after(&headers("retry-after", "1.5")?),
        Some(Duration::from_millis(1500)),
        "the seconds count reads fractions"
    );
    assert_eq!(
        parse_retry_after(&headers("retry-after", "1e308")?),
        None,
        "a count that overflows asks for no wait"
    );
    Ok(())
}

/// A date is read against the clock, so the test supplies one.
#[test]
fn a_server_delay_reads_an_http_date() -> Outcome {
    let minute = headers("retry-after", "Thu, 01 Jan 1970 00:01:00 GMT")?;
    assert_eq!(
        parse_retry_after_at(&minute, SystemTime::UNIX_EPOCH),
        Some(Duration::from_secs(60))
    );
    assert_eq!(
        parse_retry_after_at(&minute, SystemTime::UNIX_EPOCH + Duration::from_secs(120)),
        Some(Duration::ZERO),
        "a date already past asks for no wait"
    );
    Ok(())
}

#[test]
fn a_failure_is_retried_by_its_kind() {
    let policy = RetryPolicy::default();
    assert!(policy.retries_error(&Error::Timeout {
        timeout: Duration::from_secs(1)
    }));
    assert!(policy.retries_error(&Error::Connection {
        message: "connection error: reset".to_string(),
        source: None,
    }));
    assert!(policy.retries_error(&api(503)));
    assert!(!policy.retries_error(&api(400)));
    assert!(!policy.retries_error(&Error::Config("no key".to_string())));

    let narrow = RetryPolicy {
        timeouts: false,
        connection_errors: false,
        ..RetryPolicy::default()
    };
    assert!(!narrow.retries_error(&Error::Timeout {
        timeout: Duration::from_secs(1)
    }));
    assert!(!narrow.retries_error(&Error::Connection {
        message: "connection error: reset".to_string(),
        source: None,
    }));
}

#[test]
fn a_predicate_adds_to_the_built_in_rules() {
    let policy = RetryPolicy {
        predicate: Some(Arc::new(|error| matches!(error, Error::Config(_)))),
        ..RetryPolicy::default()
    };
    assert!(policy.retries_error(&Error::Config("no key".to_string())));
    assert!(
        policy.retries_error(&api(503)),
        "the built-in rules still hold"
    );
    assert!(!policy.retries_error(&api(400)));
}

#[test]
fn every_field_is_checked() {
    assert!(RetryPolicy::default().validate().is_ok());
    for jitter in [-0.1, 1.1, f64::NAN] {
        let policy = RetryPolicy {
            backoff_jitter: jitter,
            ..RetryPolicy::default()
        };
        assert!(
            matches!(policy.validate(), Err(Error::Config(_))),
            "{jitter}"
        );
    }
    let wide = RetryPolicy {
        http_statuses: [99].into_iter().collect(),
        ..RetryPolicy::default()
    };
    assert!(matches!(wide.validate(), Err(Error::Config(_))));
    let spent = RetryPolicy {
        budget: Some(Duration::ZERO),
        ..RetryPolicy::default()
    };
    assert!(matches!(spent.validate(), Err(Error::Config(_))));
}

/// A rate-limit error carries the wait the server asked for, and the accessor
/// reads it back out.
#[test]
fn a_rate_limit_error_names_the_wait() -> Outcome {
    let waiting = headers("retry-after-ms", "125")?;
    let error = ApiError {
        status: 429,
        headers: waiting.clone(),
        body: None,
        request_id: None,
        endpoint: "GET https://api.typesafe.ai/v1/models".to_string(),
        kind: ApiErrorKind::of(429, &waiting),
    };
    assert_eq!(
        error.kind,
        ApiErrorKind::RateLimit {
            retry_after: Some(Duration::from_millis(125))
        }
    );
    assert_eq!(error.retry_after(), Some(Duration::from_millis(125)));

    let quiet = ApiError {
        status: 429,
        headers: HeaderMap::new(),
        body: None,
        request_id: None,
        endpoint: "GET https://api.typesafe.ai/v1/models".to_string(),
        kind: ApiErrorKind::of(429, &HeaderMap::new()),
    };
    assert_eq!(quiet.kind, ApiErrorKind::RateLimit { retry_after: None });
    assert_eq!(quiet.retry_after(), None);
    Ok(())
}

/// The policy carries a caller's closure, which has no reading of its own, so
/// `Debug` says whether one is set and nothing more.
#[test]
fn the_policy_reads_without_its_predicate() {
    let policy = RetryPolicy {
        predicate: Some(Arc::new(|_| true)),
        ..RetryPolicy::default()
    };
    let rendered = format!("{policy:?}");
    assert!(rendered.contains("predicate: Some(\"set\")"), "{rendered}");
    assert!(rendered.contains("max_retries: 2"), "{rendered}");
}
