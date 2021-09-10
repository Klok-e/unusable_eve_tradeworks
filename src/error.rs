use rust_eveonline_esi::apis::{self, market_api::GetMarketsGroupsError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("market group")]
    MarketGroups(#[from] apis::Error<GetMarketsGroupsError>),
}

pub type Result<T> = std::result::Result<T, Error>;
