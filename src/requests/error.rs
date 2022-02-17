use std::fmt::Display;

use reqwest::StatusCode;
use rust_eveonline_esi::apis::{
    self,
    market_api::{
        GetMarketsGroupsError, GetMarketsRegionIdOrdersError, GetMarketsRegionIdOrdersSuccess,
    },
    search_api::{GetCharactersCharacterIdSearchError, GetSearchError},
};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, EsiApiError>;

#[derive(Debug)]
pub struct EsiApiError {
    internal: EsiApiErrorEnum,
    pub status: reqwest::StatusCode,
}

impl std::error::Error for EsiApiError {}

impl Display for EsiApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.internal.fmt(f)
    }
}

#[derive(Error, Debug)]
enum EsiApiErrorEnum {
    #[error("market group")]
    MarketGroups(#[from] apis::Error<GetMarketsGroupsError>),
    #[error("regional market orders")]
    MarketOrders(#[from] apis::Error<GetMarketsRegionIdOrdersError>),
    #[error("search")]
    Search(#[from] apis::Error<GetSearchError>),
    #[error("structure search")]
    StructSearch(#[from] apis::Error<GetCharactersCharacterIdSearchError>),
}

impl From<apis::Error<GetMarketsGroupsError>> for EsiApiError {
    fn from(x: apis::Error<GetMarketsGroupsError>) -> Self {
        let code = match &x {
            apis::Error::ResponseError(x) => x.status,
            _ => panic!(),
        };
        EsiApiError {
            internal: x.into(),
            status: code,
        }
    }
}

impl From<apis::Error<GetMarketsRegionIdOrdersError>> for EsiApiError {
    fn from(x: apis::Error<GetMarketsRegionIdOrdersError>) -> Self {
        let code = match &x {
            apis::Error::ResponseError(x) => x.status,
            _ => panic!(),
        };
        EsiApiError {
            internal: x.into(),
            status: code,
        }
    }
}

impl From<apis::Error<GetSearchError>> for EsiApiError {
    fn from(x: apis::Error<GetSearchError>) -> Self {
        let code = match &x {
            apis::Error::ResponseError(x) => x.status,
            _ => panic!(),
        };
        EsiApiError {
            internal: x.into(),
            status: code,
        }
    }
}
impl From<apis::Error<GetCharactersCharacterIdSearchError>> for EsiApiError {
    fn from(x: apis::Error<GetCharactersCharacterIdSearchError>) -> Self {
        let code = match &x {
            apis::Error::ResponseError(x) => x.status,
            _ => panic!(),
        };
        EsiApiError {
            internal: x.into(),
            status: code,
        }
    }
}
