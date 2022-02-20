use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("market group")]
    EsiApi(#[from] crate::requests::error::EsiApiError),
    #[error("Logger initialization failure")]
    Log(#[from] fern::InitError),
    #[error("Logger initialization failure")]
    File(#[from] std::io::Error),
    #[error("Serialization failure")]
    Serialization(#[from] serde_json::Error),
    #[error("Reqwest error")]
    Reqwest(#[from] reqwest::Error),
    #[error("Rusqlite error")]
    Rusqlite(#[from] rusqlite::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
