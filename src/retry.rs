use std::{fmt::Display, time::Duration};

use crate::consts::RETRIES;
use futures::Future;
use rust_eveonline_esi::apis::{
    market_api::{GetMarketsRegionIdHistoryError, GetMarketsRegionIdOrdersError},
    universe_api::GetUniverseTypesTypeIdError,
};
use thiserror::Error;

#[track_caller]
pub fn retry<T, Fut, F, E>(func: F) -> impl Future<Output = Option<T>>
where
    Fut: Future<Output = Result<T, E>>,
    F: Fn() -> Fut,
    E: IntoCcpError + Display,
{
    let caller = std::panic::Location::caller();
    async move {
        let mut retries = 0;
        loop {
            break match func().await {
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
        }
    }
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
            rust_eveonline_esi::apis::Error::Reqwest(error) => dbg!(error)
                .status()
                .map(|status| {
                    if status.as_u16() == 420 {
                        CcpError::ErrorLimited
                    } else {
                        CcpError::Other
                    }
                })
                .unwrap_or(CcpError::Other),
            _ => CcpError::Other,
        }
    }
}
impl IntoCcpError for rust_eveonline_esi::apis::Error<GetMarketsRegionIdHistoryError> {
    fn as_ccp_error(&self) -> CcpError {
        match self {
            rust_eveonline_esi::apis::Error::Reqwest(error) => error
                .status()
                .map(|status| {
                    if status.as_u16() == 420 {
                        CcpError::ErrorLimited
                    } else {
                        CcpError::Other
                    }
                })
                .unwrap_or(CcpError::Other),
            _ => CcpError::Other,
        }
    }
}
impl IntoCcpError for rust_eveonline_esi::apis::Error<GetMarketsRegionIdOrdersError> {
    fn as_ccp_error(&self) -> CcpError {
        match self {
            rust_eveonline_esi::apis::Error::Reqwest(error) => error
                .status()
                .map(|status| {
                    if status.as_u16() == 420 {
                        CcpError::ErrorLimited
                    } else {
                        CcpError::Other
                    }
                })
                .unwrap_or(CcpError::Other),
            _ => CcpError::Other,
        }
    }
}
