use std::{future::Future, path::Path};

use serde::{de::DeserializeOwned, Serialize};
use serde_json;

#[derive(Debug)]
pub struct CachedData<T> {
    pub data: T,
}

impl<T> CachedData<T>
where
    T: Serialize + DeserializeOwned,
{
    pub fn load_or_create(path: impl AsRef<Path>, gen: impl FnOnce() -> T) -> Self {
        let data = if path.as_ref().exists() {
            let str = std::fs::read_to_string(path).unwrap();
            serde_json::from_str(str.as_str()).unwrap()
        } else {
            gen()
        };
        Self { data }
    }

    pub async fn load_or_create_async<F, FO>(path: impl AsRef<Path>, gen: F) -> Self
    where
        F: FnOnce() -> FO,
        FO: Future<Output = T>,
    {
        let data = if path.as_ref().exists() {
            let str = std::fs::read_to_string(path).unwrap();
            serde_json::from_str(str.as_str()).unwrap()
        } else {
            gen().await
        };
        Self { data }
    }
}
