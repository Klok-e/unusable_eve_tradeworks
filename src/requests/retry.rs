use std::time::Duration;

use super::error::{EsiApiError, Result};
use crate::consts::RETRIES;
use futures::Future;
use reqwest::StatusCode;

#[track_caller]
pub fn retry_smart<T, Fut, F>(func: F) -> impl Future<Output = Result<Option<T>>>
where
    Fut: Future<Output = Result<Retry<T>>>,
    F: Fn() -> Fut,
{
    let caller = std::panic::Location::caller();
    async move {
        let res = retry_simple(|| async {
            let out = func().await;
            match out {
                Ok(Retry::Success(x)) => Ok(Retry::Success(x)),
                Ok(Retry::Retry) => Ok(Retry::Retry),

                // error limited
                Err(e @ EsiApiError { status, .. })
                    if status == StatusCode::from_u16(420).unwrap() =>
                {
                    log::warn!(
                        "[{}] Error limited: {}. Retrying in 60 seconds...",
                        caller,
                        e
                    );
                    tokio::time::sleep(Duration::from_secs_f32(60.)).await;
                    Ok(Retry::Retry)
                }

                // common errors for ccp servers
                Err(EsiApiError {
                    status:
                        status @ (StatusCode::BAD_GATEWAY
                        | StatusCode::SERVICE_UNAVAILABLE
                        | StatusCode::GATEWAY_TIMEOUT),
                    ..
                }) => {
                    log::warn!("[{}] Error: {}. Retrying...", caller, status);
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
                    tokio::time::sleep(Duration::from_secs_f32(0.1)).await;
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
