use std::fmt::Display;

use reqwest::StatusCode;
use rust_eveonline_esi::apis::{
    self,
    killmails_api::GetKillmailsKillmailIdKillmailHashError,
    market_api::{
        GetMarketsGroupsError, GetMarketsPricesError, GetMarketsRegionIdHistoryError,
        GetMarketsRegionIdOrdersError, GetMarketsRegionIdTypesError,
        GetMarketsStructuresStructureIdError,
    },
    routes_api::GetRouteOriginDestinationError,
    search_api::GetCharactersCharacterIdSearchError,
    universe_api::GetUniverseTypesTypeIdError,
    wallet_api::GetCharactersCharacterIdWalletTransactionsError,
    ResponseContent,
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
    #[error("Market groups error: {0}")]
    MarketGroups(#[from] apis::Error<GetMarketsGroupsError>),
    #[error("Regional market error: {0}")]
    MarketOrders(#[from] apis::Error<GetMarketsRegionIdOrdersError>),
    #[error("Structure search error: {0}")]
    StructSearch(#[from] apis::Error<GetCharactersCharacterIdSearchError>),
    #[error("Structure market error: {0}")]
    StructureMarket(#[from] apis::Error<GetMarketsStructuresStructureIdError>),
    #[error("Region type IDs error: {0}")]
    RegionTypeId(#[from] apis::Error<GetMarketsRegionIdTypesError>),
    #[error("Route error: {0}")]
    Route(#[from] apis::Error<GetRouteOriginDestinationError>),
    #[error("Killmails data error: {0}")]
    KillmailData(#[from] apis::Error<GetKillmailsKillmailIdKillmailHashError>),
    #[error("Universe types type ID error: {0}")]
    UniverseTypesTypeId(#[from] apis::Error<GetUniverseTypesTypeIdError>),
    #[error("Market history error: {0}")]
    MarketHistory(#[from] apis::Error<GetMarketsRegionIdHistoryError>),
    #[error("Market prices error: {0}")]
    MarketsPrices(#[from] apis::Error<GetMarketsPricesError>),
    #[error("Character wallet transactions error: {0}")]
    CharacterWalletTransactions(
        #[from] apis::Error<GetCharactersCharacterIdWalletTransactionsError>,
    ),
}

impl From<apis::Error<GetMarketsPricesError>> for EsiApiError {
    fn from(x: apis::Error<GetMarketsPricesError>) -> Self {
        let code = match &x {
            apis::Error::ResponseError(x) => x.status,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        EsiApiError {
            internal: x.into(),
            status: code,
        }
    }
}

impl From<apis::Error<GetMarketsGroupsError>> for EsiApiError {
    fn from(x: apis::Error<GetMarketsGroupsError>) -> Self {
        let code = match &x {
            apis::Error::ResponseError(x) => x.status,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
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
            _ => StatusCode::INTERNAL_SERVER_ERROR,
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
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        EsiApiError {
            internal: x.into(),
            status: code,
        }
    }
}

impl From<apis::Error<GetMarketsStructuresStructureIdError>> for EsiApiError {
    fn from(x: apis::Error<GetMarketsStructuresStructureIdError>) -> Self {
        // this particular endpoint returns 500 code on invalid page
        // so we have to extract error message
        let code = match &x {
            apis::Error::ResponseError(ResponseContent {
                entity: Some(GetMarketsStructuresStructureIdError::Status400(r)),
                ..
            }) if r.error.contains("Undefined 404 response") => StatusCode::NOT_FOUND,
            apis::Error::ResponseError(x) => x.status,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        EsiApiError {
            internal: x.into(),
            status: code,
        }
    }
}

impl From<apis::Error<GetMarketsRegionIdTypesError>> for EsiApiError {
    fn from(x: apis::Error<GetMarketsRegionIdTypesError>) -> Self {
        // this particular endpoint returns 500 code on invalid page
        // so we have to extract error message
        let code = match &x {
            apis::Error::ResponseError(ResponseContent {
                entity: Some(GetMarketsRegionIdTypesError::Status400(r)),
                ..
            }) if r.error.contains("Undefined 404 response") => StatusCode::NOT_FOUND,
            apis::Error::ResponseError(x) => x.status,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        EsiApiError {
            internal: x.into(),
            status: code,
        }
    }
}

impl From<apis::Error<GetRouteOriginDestinationError>> for EsiApiError {
    fn from(x: apis::Error<GetRouteOriginDestinationError>) -> Self {
        let code = match &x {
            apis::Error::ResponseError(x) => x.status,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        EsiApiError {
            internal: x.into(),
            status: code,
        }
    }
}

impl From<apis::Error<GetKillmailsKillmailIdKillmailHashError>> for EsiApiError {
    fn from(x: apis::Error<GetKillmailsKillmailIdKillmailHashError>) -> Self {
        let code = match &x {
            apis::Error::ResponseError(x) => x.status,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        EsiApiError {
            internal: x.into(),
            status: code,
        }
    }
}

impl From<apis::Error<GetUniverseTypesTypeIdError>> for EsiApiError {
    fn from(x: apis::Error<GetUniverseTypesTypeIdError>) -> Self {
        let code = match &x {
            apis::Error::ResponseError(x) => x.status,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        EsiApiError {
            internal: x.into(),
            status: code,
        }
    }
}

impl From<apis::Error<GetMarketsRegionIdHistoryError>> for EsiApiError {
    fn from(x: apis::Error<GetMarketsRegionIdHistoryError>) -> Self {
        let code = match &x {
            apis::Error::ResponseError(ResponseContent {
                entity: Some(GetMarketsRegionIdHistoryError::Status400(r)),
                ..
            }) if r.error.contains("Undefined 429 response") => StatusCode::TOO_MANY_REQUESTS,
            apis::Error::ResponseError(x) => x.status,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        EsiApiError {
            internal: x.into(),
            status: code,
        }
    }
}

impl From<apis::Error<GetCharactersCharacterIdWalletTransactionsError>> for EsiApiError {
    fn from(x: apis::Error<GetCharactersCharacterIdWalletTransactionsError>) -> Self {
        let code = match &x {
            apis::Error::ResponseError(x) => x.status,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        EsiApiError {
            internal: x.into(),
            status: code,
        }
    }
}
