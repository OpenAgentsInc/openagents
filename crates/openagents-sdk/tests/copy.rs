use openagents_sdk::{Error, StorageError};

#[test]
fn errors_tell_people_what_happened_or_what_to_do() {
    for error in [
        Error::InvalidDeployment,
        Error::InvalidTimeout,
        Error::InvalidCredential,
        Error::WrongDeployment,
        Error::WrongAccount,
        Error::ExpiredSession,
        Error::Unauthorized,
        Error::Forbidden,
        Error::Denied,
        Error::ExpiredLogin,
        Error::FinishedLogin,
        Error::Cancelled,
        Error::UncertainLogin(Box::new(Error::InvalidResponse)),
        Error::Network { timeout: true },
        Error::InvalidResponse,
        Error::NotFound,
        Error::Changed,
        Error::InvalidCursor,
        Error::InvalidReference,
        Error::TooLarge,
        Error::Redirect,
        Error::RateLimited { retry_after: None },
        Error::Unavailable { retry_after: None },
        Error::Refused {
            status: 400,
            code: None,
        },
    ] {
        let text = error.to_string();
        assert!(oa_copy::violations(&text, &[]).is_empty(), "{text}");
    }
    for error in [StorageError::Denied, StorageError::Unavailable] {
        assert!(oa_copy::violations(&error.to_string(), &[]).is_empty());
    }
}

#[test]
fn computer_errors_use_plain_copy_and_keep_private_details_out_of_diagnostics() {
    use openagents_sdk::computers::Error;
    for error in [
        Error::InvalidInput,
        Error::InvalidResponse,
        Error::TooLarge,
        Error::Redirect,
        Error::Cancelled,
        Error::WaitTimedOut,
        Error::Unsupported,
        Error::RandomUnavailable,
        Error::Network { timeout: true },
        Error::Api {
            status: 401,
            code: None,
            retry_after: None,
        },
        Error::Api {
            status: 403,
            code: None,
            retry_after: None,
        },
        Error::Api {
            status: 404,
            code: None,
            retry_after: None,
        },
        Error::Api {
            status: 429,
            code: None,
            retry_after: None,
        },
        Error::Api {
            status: 503,
            code: Some("private_fixture".into()),
            retry_after: None,
        },
        Error::Uncertain {
            operation: "private_fixture",
            key: Some("private_fixture".into()),
            cause: Box::new(Error::InvalidResponse),
        },
    ] {
        let text = error.to_string();
        assert!(oa_copy::violations(&text, &[]).is_empty(), "{text}");
        assert!(!format!("{error:?} {error}").contains("private_fixture"));
    }
}
