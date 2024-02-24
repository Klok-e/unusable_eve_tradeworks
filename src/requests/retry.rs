use std::{num::NonZeroU32, panic::Location, time::Duration};

use super::error::EsiApiError;
use crate::consts::RETRIES;
use futures::Future;
use governor::{DefaultDirectRateLimiter, Jitter, Quota, RateLimiter};
use reqwest::StatusCode;

const ERROR_LIMITED_RETRY_DELAY: u64 = 60;

#[track_caller]
pub fn retry_smart_with_error_limiter<'a, T, Fut, F, E>(
    limiter: &'a DefaultDirectRateLimiter,
    func: F,
) -> impl Future<Output = Result<Option<T>, E>> + 'a
where
    Fut: Future<Output = Result<RetryResult<T>, E>>,
    F: Fn() -> Fut + 'a,
    E: RetryableError + std::fmt::Display + std::fmt::Debug,
{
    let caller = std::panic::Location::caller();
    async move { retry_internal(func, *caller, limiter).await }
}

#[track_caller]
pub fn retry_smart<T, Fut, F, E>(func: F) -> impl Future<Output = Result<Option<T>, E>>
where
    Fut: Future<Output = Result<RetryResult<T>, E>>,
    F: Fn() -> Fut,
    E: RetryableError + std::fmt::Display + std::fmt::Debug,
{
    let error_limiter = RateLimiter::direct(Quota::per_minute(NonZeroU32::new(100).unwrap()));
    let caller = std::panic::Location::caller();
    async move { retry_internal(func, *caller, &error_limiter).await }
}

async fn retry_internal<'a, T, Fut, F, E>(
    func: F,
    caller: Location<'a>,
    limiter: &'a DefaultDirectRateLimiter,
) -> Result<Option<T>, E>
where
    Fut: Future<Output = Result<RetryResult<T>, E>>,
    F: Fn() -> Fut,
    E: RetryableError + std::fmt::Display + std::fmt::Debug,
{
    let jitter = Jitter::up_to(std::time::Duration::from_millis(100));

    let mut retries = 0;
    loop {
        log::trace!("[{caller}] Trying...");
        let out = func().await;
        match out {
            Ok(RetryResult::Success(x)) => break Ok(Some(x)),
            Ok(RetryResult::Retry) => {
                if retries > RETRIES {
                    log::debug!("Retries finished. Retried {retries} times.");
                    break Ok(None);
                }

                retries += 1;
                log::debug!("Retrying in {ERROR_LIMITED_RETRY_DELAY}s...");
                tokio::time::sleep(Duration::from_secs_f32(ERROR_LIMITED_RETRY_DELAY as f32)).await;
            }
            Err(ref e) if e.is_error_limited() => {
                log::debug!("[{caller}] Retry: Error limited: {e:?}");
                limiter.until_ready_with_jitter(jitter).await;
            }
            Err(ref e) if e.is_too_many_requests() => {
                log::debug!("[{caller}] Retry: Too many requests: {e:?}");
                limiter.until_ready_with_jitter(jitter).await;
            }
            Err(ref e) if e.is_common_ccp_error() => {
                log::debug!("[{caller}] Retry: Error: {e:?}");
                limiter.until_ready_with_jitter(jitter).await;
            }
            Err(e) => {
                log::debug!("[{caller}] Error broke out: {e:?}");
                break Err(e);
            }
        }
    }
}

pub enum RetryResult<T> {
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
