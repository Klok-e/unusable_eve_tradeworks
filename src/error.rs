use rust_eveonline_esi::apis::{
    self,
    market_api::{GetMarketsGroupsError, GetMarketsRegionIdOrdersError},
    search_api::{GetCharactersCharacterIdSearchError, GetSearchError},
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("market group")]
    MarketGroups(#[from] apis::Error<GetMarketsGroupsError>),
    #[error("regional market orders")]
    MarketOrders(#[from] apis::Error<GetMarketsRegionIdOrdersError>),
    #[error("search")]
    Search(#[from] apis::Error<GetSearchError>),
    #[error("structure search")]
    StructSearch(#[from] apis::Error<GetCharactersCharacterIdSearchError>),
    #[error("Logger initialization failure")]
    Log(#[from] fern::InitError),
    #[error("Logger initialization failure")]
    File(#[from] std::io::Error),
    #[error("Serialization failure")]
    Serialization(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
