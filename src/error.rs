use rust_eveonline_esi::apis::{
    self, market_api::GetMarketsGroupsError, search_api::GetSearchError,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("market group")]
    MarketGroups(#[from] apis::Error<GetMarketsGroupsError>),
    #[error("search")]
    Search(#[from] apis::Error<GetSearchError>),
}

pub type Result<T> = std::result::Result<T, Error>;
