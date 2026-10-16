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
