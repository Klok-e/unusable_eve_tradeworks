use std::{fmt::Display, time::Duration};

use super::error::{EsiApiError, Result};
use crate::consts::RETRIES;
use futures::Future;
use reqwest::StatusCode;
use rust_eveonline_esi::apis::{
    killmails_api::GetKillmailsKillmailIdKillmailHashError,
    market_api::{GetMarketsRegionIdHistoryError, GetMarketsRegionIdOrdersError},
    routes_api::GetRouteOriginDestinationError,
    universe_api::GetUniverseTypesTypeIdError,
    ResponseContent,
};
use thiserror::Error;

#[track_caller]
pub fn retry<T, Fut, F, E>(func: F) -> impl Future<Output = Option<T>>
where
    Fut: Future<Output = std::result::Result<T, E>>,
    F: Fn() -> Fut,
    E: IntoCcpError + Display,
{
    let caller = std::panic::Location::caller();
    async move {
        let mut retries = 0;
        loop {
            let res = match func().await {
                Ok(ok) => Some(ok),
                Err(e) => {
                    let err = e.as_ccp_error();
                    if let CcpError::ErrorLimited = err {
                        log::warn!("[{}] error limited: {}", caller, e);
                        log::warn!("Sleeping...");
                        tokio::time::sleep(Duration::from_secs_f32(30.)).await;
                        continue;
                    }
                    retries += 1;
                    if retries <= RETRIES {
                        // don't make too many retries sequentially
                        tokio::time::sleep(Duration::from_secs_f32(1.)).await;
                        continue;
                    }
                    log::warn!(
                        "[{}] error: {}; retry: {}. No more retries left.",
                        caller,
                        e,
                        retries
                    );
                    None
                }
            };
            break res;
        }
    }
}

pub async fn retry_smart<T, Fut, F>(func: F) -> Result<Option<T>>
where
    Fut: Future<Output = Result<Retry<T>>>,
    F: Fn() -> Fut,
{
    let res = retry_simple(|| async {
        let out = func().await;
        match out {
            Ok(Retry::Success(x)) => Ok(Retry::Success(x)),
            Ok(Retry::Retry) => Ok(Retry::Retry),

            // error limited
            Err(e @ EsiApiError { status, .. }) if status == StatusCode::from_u16(420).unwrap() => {
                log::warn!("Error limited: {}. Retrying in 30 seconds...", e);
                tokio::time::sleep(Duration::from_secs_f32(30.)).await;
                Ok(Retry::Retry)
            }

            // common error for ccp servers
            Err(
                e @ EsiApiError {
                    status: StatusCode::BAD_GATEWAY,
                    ..
                },
            ) => {
                log::warn!("Error: {}. Retrying...", e);
                Ok(Retry::Retry)
            }
            Err(e) => Err(e),
        }
    })
    .await?;
    Ok(res)
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
                    tokio::time::sleep(Duration::from_secs_f32(1.)).await;
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

#[derive(Debug, Error)]
pub enum CcpError {
    #[error("Error limited")]
    ErrorLimited,
    #[error("Other error")]
    Other,
}

pub trait IntoCcpError {
    fn as_ccp_error(&self) -> CcpError;
}

impl IntoCcpError for rust_eveonline_esi::apis::Error<GetUniverseTypesTypeIdError> {
    fn as_ccp_error(&self) -> CcpError {
        match self {
            rust_eveonline_esi::apis::Error::ResponseError(ResponseContent { status, .. }) => {
                if status.as_u16() == 420 {
                    CcpError::ErrorLimited
                } else {
                    CcpError::Other
                }
            }
            _ => CcpError::Other,
        }
    }
}
impl IntoCcpError for rust_eveonline_esi::apis::Error<GetMarketsRegionIdHistoryError> {
    fn as_ccp_error(&self) -> CcpError {
        match self {
            rust_eveonline_esi::apis::Error::ResponseError(ResponseContent { status, .. }) => {
                if status.as_u16() == 420 {
                    CcpError::ErrorLimited
                } else {
                    CcpError::Other
                }
            }
            _ => CcpError::Other,
        }
    }
}
impl IntoCcpError for rust_eveonline_esi::apis::Error<GetMarketsRegionIdOrdersError> {
    fn as_ccp_error(&self) -> CcpError {
        match self {
            rust_eveonline_esi::apis::Error::ResponseError(ResponseContent { status, .. }) => {
                if status.as_u16() == 420 {
                    CcpError::ErrorLimited
                } else {
                    CcpError::Other
                }
            }
            _ => CcpError::Other,
        }
    }
}
impl IntoCcpError for rust_eveonline_esi::apis::Error<GetKillmailsKillmailIdKillmailHashError> {
    fn as_ccp_error(&self) -> CcpError {
        match self {
            rust_eveonline_esi::apis::Error::ResponseError(ResponseContent { status, .. }) => {
                if status.as_u16() == 420 {
                    CcpError::ErrorLimited
                } else {
                    CcpError::Other
                }
            }
            _ => CcpError::Other,
        }
    }
}
impl IntoCcpError for rust_eveonline_esi::apis::Error<GetRouteOriginDestinationError> {
    fn as_ccp_error(&self) -> CcpError {
        match self {
            rust_eveonline_esi::apis::Error::ResponseError(ResponseContent { status, .. }) => {
                if status.as_u16() == 420 {
                    CcpError::ErrorLimited
                } else {
                    CcpError::Other
                }
            }
            _ => CcpError::Other,
        }
    }
}
