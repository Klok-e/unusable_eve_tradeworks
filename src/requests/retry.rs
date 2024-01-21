use std::time::Duration;

use super::error::EsiApiError;
use crate::consts::RETRIES;
use futures::Future;
use reqwest::StatusCode;

const ERROR_LIMITED_RETRY_DELAY: u64 = 60;

#[track_caller]
pub fn retry_smart<T, Fut, F, E>(func: F) -> impl Future<Output = Result<Option<T>, E>>
where
    Fut: Future<Output = Result<Retry<T>, E>>,
    F: Fn() -> Fut,
    E: RetryableError + std::fmt::Display + std::fmt::Debug,
{
    let caller = std::panic::Location::caller();
    async move {
        let mut retries = 0;
        loop {
            log::debug!("[{caller}] Trying...");
            let out = func().await;
            match out {
                Ok(Retry::Success(x)) => break Ok(Some(x)),
                Ok(Retry::Retry) => {
                    if retries > RETRIES {
                        log::debug!("Retries finished. Retried {retries} times.");
                        break Ok(None);
                    } else {
                        retries += 1;
                        log::debug!("Retrying in {ERROR_LIMITED_RETRY_DELAY}s...");
                        tokio::time::sleep(Duration::from_secs_f32(
                            ERROR_LIMITED_RETRY_DELAY as f32,
                        ))
                        .await;
                    }
                }
                Err(ref e) if e.is_error_limited() => {
                    log::debug!("[{caller}] Retry: Error limited: {e:?}");
                    tokio::time::sleep(Duration::from_secs(ERROR_LIMITED_RETRY_DELAY)).await;
                }
                Err(ref e) if e.is_too_many_requests() => {
                    log::debug!("[{caller}] Retry: Too many requests: {e:?}");
                    tokio::time::sleep(Duration::from_secs(ERROR_LIMITED_RETRY_DELAY)).await;
                }
                Err(ref e) if e.is_common_ccp_error() => {
                    log::debug!("[{caller}] Retry: Error: {e:?}");
                    tokio::time::sleep(Duration::from_secs(ERROR_LIMITED_RETRY_DELAY)).await;
                }
                Err(e) => {
                    log::debug!("[{caller}] Error broke out: {e:?}");
                    break Err(e);
                }
            }
        }
    }
}

pub enum Retry<T> {
    Retry,
    Success(T),
}

pub trait RetryableError {
    fn is_common_ccp_error(&self) -> bool;
    fn is_error_limited(&self) -> bool;
    fn is_too_many_requests(&self) -> bool;
}

impl RetryableError for EsiApiError {
    fn is_common_ccp_error(&self) -> bool {
        matches!(
            self.status,
            StatusCode::INTERNAL_SERVER_ERROR
                | StatusCode::BAD_GATEWAY
                | StatusCode::SERVICE_UNAVAILABLE
                | StatusCode::GATEWAY_TIMEOUT
        )
    }

    fn is_error_limited(&self) -> bool {
        self.status == StatusCode::from_u16(420).expect("Invalid status code")
    }

    fn is_too_many_requests(&self) -> bool {
        self.status == StatusCode::from_u16(429).expect("Invalid status code")
    }
}

impl RetryableError for reqwest::Error {
    fn is_common_ccp_error(&self) -> bool {
        matches!(
            self.status(),
            Some(
                StatusCode::INTERNAL_SERVER_ERROR
                    | StatusCode::BAD_GATEWAY
                    | StatusCode::SERVICE_UNAVAILABLE
                    | StatusCode::GATEWAY_TIMEOUT
            )
        )
    }

    fn is_error_limited(&self) -> bool {
        self.status() == Some(StatusCode::from_u16(420).expect("Invalid status code"))
    }

    fn is_too_many_requests(&self) -> bool {
        self.status() == Some(StatusCode::TOO_MANY_REQUESTS)
    }
}
