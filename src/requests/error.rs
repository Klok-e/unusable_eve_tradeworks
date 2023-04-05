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
    #[error("market group")]
    MarketGroups(#[from] apis::Error<GetMarketsGroupsError>),
    #[error("regional market")]
    MarketOrders(#[from] apis::Error<GetMarketsRegionIdOrdersError>),
    #[error("structure search")]
    StructSearch(#[from] apis::Error<GetCharactersCharacterIdSearchError>),
    #[error("structure market")]
    StructureMarket(#[from] apis::Error<GetMarketsStructuresStructureIdError>),
    #[error("region type ids")]
    RegionTypeId(#[from] apis::Error<GetMarketsRegionIdTypesError>),
    #[error("route")]
    Route(#[from] apis::Error<GetRouteOriginDestinationError>),
    #[error("killmails")]
    KillmailData(#[from] apis::Error<GetKillmailsKillmailIdKillmailHashError>),
    #[error("type ids")]
    UniverseTypesTypeId(#[from] apis::Error<GetUniverseTypesTypeIdError>),
    #[error("market history")]
    MarketHistory(#[from] apis::Error<GetMarketsRegionIdHistoryError>),
    #[error("market prices")]
    MarketsPrices(#[from] apis::Error<GetMarketsPricesError>),
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
            apis::Error::ResponseError(x) => x.status,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        EsiApiError {
            internal: x.into(),
            status: code,
        }
    }
}
