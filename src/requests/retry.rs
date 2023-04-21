use std::time::Duration;

use super::error::EsiApiError;
use crate::consts::RETRIES;
use futures::Future;
use reqwest::StatusCode;

const ERROR_LIMITED_RETRY_DELAY: u64 = 60;
const RETRY_DELAY: f32 = 0.1;
const MAX_RETRY_DELAY: f32 = 120.;

#[track_caller]
pub fn retry_smart<T, Fut, F, E>(func: F) -> impl Future<Output = Result<Option<T>, E>>
where
    Fut: Future<Output = Result<Retry<T>, E>>,
    F: Fn() -> Fut,
    E: RetryableError + std::fmt::Display,
{
    let caller = std::panic::Location::caller();
    async move {
        let res = retry_simple(|| async {
            let out = func().await;
            match out {
                Ok(Retry::Success(x)) => Ok(Retry::Success(x)),
                Ok(Retry::Retry) => Ok(Retry::Retry),

                // error limited
                Err(ref e) if e.is_error_limited() => {
                    log::warn!(
                        "[{}] Error limited: {}. Retrying in 60 seconds...",
                        caller,
                        e
                    );
                    tokio::time::sleep(Duration::from_secs(ERROR_LIMITED_RETRY_DELAY)).await;
                    Ok(Retry::Retry)
                }

                // common errors for ccp servers
                Err(ref err) if err.should_retry() => {
                    log::warn!("[{}] Error: {}. Retrying...", caller, err);
                    Ok(Retry::Retry)
                }

                Err(e) => Err(e),
            }
        })
        .await?;
        Ok(res)
    }
}

pub async fn retry_simple<T, Fut, F, E>(func: F) -> std::result::Result<Option<T>, E>
where
    Fut: Future<Output = std::result::Result<Retry<T>, E>>,
    F: Fn() -> Fut,
{
    let mut retries = 0;
    loop {
        let out = func().await?;
        break Ok(match out {
            Retry::Retry => {
                if retries > RETRIES {
                    None
                } else {
                    retries += 1;
                    // don't make too many retries sequentially
                    let delay = get_exponential_backoff(retries, RETRY_DELAY, MAX_RETRY_DELAY);
                    tokio::time::sleep(Duration::from_secs_f32(delay)).await;
                    continue;
                }
            }
            Retry::Success(v) => Some(v),
        });
    }
}

pub enum Retry<T> {
    Retry,
    Success(T),
}

fn get_exponential_backoff(retries: u32, base: f32, cap: f32) -> f32 {
    let delay = (2f32.powf(retries as f32) - 1.0) * base;
    delay.min(cap)
}

pub trait RetryableError {
    fn should_retry(&self) -> bool;
    fn is_error_limited(&self) -> bool;
}

impl RetryableError for EsiApiError {
    fn should_retry(&self) -> bool {
        matches!(
            self.status,
            StatusCode::BAD_GATEWAY | StatusCode::SERVICE_UNAVAILABLE | StatusCode::GATEWAY_TIMEOUT
        )
    }

    fn is_error_limited(&self) -> bool {
        self.status == StatusCode::from_u16(420).expect("Invalid status code")
    }
}
