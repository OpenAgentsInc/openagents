//! Retry logic with exponential backoff for database operations
//!
//! Provides retry mechanisms for transient database errors like lock contention.

use rusqlite::{Error as SqliteError, ErrorCode};
use openagents_utils::backoff::{ExponentialBackoff, Jitter};
use std::thread;
use std::time::Duration;
use tracing::{debug, warn};

/// Maximum number of retry attempts
const MAX_RETRIES: u32 = 3;

/// Initial retry delay in milliseconds
const INITIAL_DELAY_MS: u64 = 100;

/// Maximum backoff delay in milliseconds
const MAX_BACKOFF_MS: u64 = 5000;

/// Check if an error is transient and should be retried
fn is_transient_error(error: &SqliteError) -> bool {
    match error {
        SqliteError::SqliteFailure(err, _) => matches!(
            err.code,
            ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked
        ),
        _ => false,
    }
}

/// Execute a database operation with retry logic and exponential backoff
///
/// This function will retry transient errors (SQLITE_BUSY, SQLITE_LOCKED) with
/// exponential backoff. Permanent errors are returned immediately.
///
/// # Example
///
/// ```no_run
/// use rusqlite::{Connection, Result};
/// use issues::retry::with_retry;
///
/// fn get_count(conn: &Connection) -> Result<i64> {
///     with_retry(|| {
///         conn.query_row("SELECT COUNT(*) FROM issues", [], |row| row.get(0))
///     })
/// }
/// ```
pub fn with_retry<F, T>(mut operation: F) -> rusqlite::Result<T>
where
    F: FnMut() -> rusqlite::Result<T>,
{
    let mut backoff = ExponentialBackoff::new(
        Duration::from_millis(INITIAL_DELAY_MS),
        Duration::from_millis(MAX_BACKOFF_MS),
        MAX_RETRIES.saturating_sub(1),
    )
    .with_jitter(Jitter::None);

    for attempt in 0..MAX_RETRIES {
        match operation() {
            Ok(result) => {
                if attempt > 0 {
                    debug!("Operation succeeded after {} retries", attempt);
                }
                return Ok(result);
            }
            Err(e) if is_transient_error(&e) => {
                let Some(delay) = backoff.next_delay() else {
                    warn!(
                        "Max retries ({}) exceeded for transient error: {}",
                        MAX_RETRIES, e
                    );
                    return Err(e);
                };

                warn!(
                    "Transient database error (attempt {}/{}): {}. Retrying in {}ms",
                    attempt + 1,
                    MAX_RETRIES,
                    e,
                    delay.as_millis()
                );

                thread::sleep(delay);
            }
            Err(e) => {
                // Permanent error - don't retry
                debug!("Non-transient error, not retrying: {}", e);
                return Err(e);
            }
        }
    }

    unreachable!("Loop should always return from within")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_successful_operation_no_retry() {
        let call_count = Arc::new(Mutex::new(0));
        let count_clone = call_count.clone();

        let result = with_retry(|| {
            *count_clone.lock().unwrap() += 1;
            Ok::<i32, SqliteError>(42)
        });

        assert_eq!(result.unwrap(), 42);
        assert_eq!(*call_count.lock().unwrap(), 1);
    }

    #[test]
    fn test_transient_error_retries() {
        let call_count = Arc::new(Mutex::new(0));
        let count_clone = call_count.clone();

        let result = with_retry(|| {
            let mut count = count_clone.lock().unwrap();
            *count += 1;

            if *count < 3 {
                // Return SQLITE_BUSY for first two attempts
                Err(SqliteError::SqliteFailure(
                    rusqlite::ffi::Error {
                        code: ErrorCode::DatabaseBusy,
                        extended_code: 5, // SQLITE_BUSY
                    },
                    None,
                ))
            } else {
                Ok::<i32, SqliteError>(42)
            }
        });

        assert_eq!(result.unwrap(), 42);
        assert_eq!(*call_count.lock().unwrap(), 3);
    }

    #[test]
    fn test_max_retries_exceeded() {
        let call_count = Arc::new(Mutex::new(0));
        let count_clone = call_count.clone();

        let result = with_retry(|| {
            *count_clone.lock().unwrap() += 1;
            // Always return SQLITE_BUSY
            Err::<i32, SqliteError>(SqliteError::SqliteFailure(
                rusqlite::ffi::Error {
                    code: ErrorCode::DatabaseBusy,
                    extended_code: 5, // SQLITE_BUSY
                },
                None,
            ))
        });

        assert!(result.is_err());
        assert_eq!(*call_count.lock().unwrap(), MAX_RETRIES as i32);
    }

    #[test]
    fn test_permanent_error_no_retry() {
        let call_count = Arc::new(Mutex::new(0));
        let count_clone = call_count.clone();

        let result = with_retry(|| {
            *count_clone.lock().unwrap() += 1;
            // Return a permanent error (constraint violation)
            Err::<i32, SqliteError>(SqliteError::SqliteFailure(
                rusqlite::ffi::Error {
                    code: ErrorCode::ConstraintViolation,
                    extended_code: 19, // SQLITE_CONSTRAINT
                },
                Some("UNIQUE constraint failed".to_string()),
            ))
        });

        assert!(result.is_err());
        // Should only be called once (no retries for permanent errors)
        assert_eq!(*call_count.lock().unwrap(), 1);
    }

    #[test]
    fn test_database_locked_retries() {
        let call_count = Arc::new(Mutex::new(0));
        let count_clone = call_count.clone();

        let result = with_retry(|| {
            let mut count = count_clone.lock().unwrap();
            *count += 1;

            if *count < 2 {
                Err(SqliteError::SqliteFailure(
                    rusqlite::ffi::Error {
                        code: ErrorCode::DatabaseLocked,
                        extended_code: 6, // SQLITE_LOCKED
                    },
                    None,
                ))
            } else {
                Ok::<i32, SqliteError>(42)
            }
        });

        assert_eq!(result.unwrap(), 42);
        assert_eq!(*call_count.lock().unwrap(), 2);
    }

    #[test]
    fn test_is_transient_error_busy() {
        let err = SqliteError::SqliteFailure(
            rusqlite::ffi::Error {
                code: ErrorCode::DatabaseBusy,
                extended_code: 5, // SQLITE_BUSY
            },
            None,
        );
        assert!(is_transient_error(&err));
    }

    #[test]
    fn test_is_transient_error_locked() {
        let err = SqliteError::SqliteFailure(
            rusqlite::ffi::Error {
                code: ErrorCode::DatabaseLocked,
                extended_code: 6, // SQLITE_LOCKED
            },
            None,
        );
        assert!(is_transient_error(&err));
    }

    #[test]
    fn test_is_not_transient_error() {
        let err = SqliteError::SqliteFailure(
            rusqlite::ffi::Error {
                code: ErrorCode::ConstraintViolation,
                extended_code: 19, // SQLITE_CONSTRAINT
            },
            None,
        );
        assert!(!is_transient_error(&err));
    }

    #[test]
    fn test_with_real_database() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE test (id INTEGER PRIMARY KEY)", [])
            .unwrap();

        let result = with_retry(|| {
            conn.query_row("SELECT COUNT(*) FROM test", [], |row| row.get::<_, i64>(0))
        });

        assert_eq!(result.unwrap(), 0);
    }
}
