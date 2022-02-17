use std::{future::Future, path::Path};

use super::error::Result;
use chrono::{DateTime, Utc};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

#[derive(Debug)]
pub struct CachedData<T> {
    pub data: T,
}

impl<T> CachedData<T>
where
    T: Serialize + DeserializeOwned,
{
    pub async fn load_or_create_async<F, FO>(
        path: impl AsRef<Path>,
        refresh: bool,
        timeout: Option<chrono::Duration>,
        gen: F,
    ) -> Result<Self>
    where
        F: FnOnce() -> FO,
        FO: Future<Output = Result<T>>,
    {
        Self::load_data_or_create_async(path, DataFormat::Bin, refresh, timeout, gen).await
    }

    pub async fn load_or_create_json_async<F, FO>(
        path: impl AsRef<Path>,
        refresh: bool,
        timeout: Option<chrono::Duration>,
        gen: F,
    ) -> Result<Self>
    where
        F: FnOnce() -> FO,
        FO: Future<Output = Result<T>>,
    {
        Self::load_data_or_create_async(path, DataFormat::Json, refresh, timeout, gen).await
    }

    async fn load_data_or_create_async<F, FO>(
        path: impl AsRef<Path>,
        format: DataFormat,
        refresh: bool,
        timeout: Option<chrono::Duration>,
        gen: F,
    ) -> Result<Self>
    where
        F: FnOnce() -> FO,
        FO: Future<Output = Result<T>>,
    {
        let cont = if path.as_ref().exists() && !refresh {
            let str = std::fs::read(path.as_ref()).unwrap();
            let deser: Container<T> = match format {
                DataFormat::Json => serde_json::from_slice(str.as_slice()).unwrap(),
                DataFormat::Bin => rmp_serde::from_read(str.as_slice()).unwrap(),
            };
            match timeout {
                Some(timeout) if deser.time + timeout < Utc::now() => {
                    log::debug!(
                        "Save time ({}) + timeout ({}) = {} < {}",
                        deser.time,
                        timeout,
                        deser.time + timeout,
                        Utc::now()
                    );
                    gen_and_save(&path, gen, format).await?
                }
                _ => {
                    log::info!("Path {:?} loaded", path.as_ref());
                    deser
                }
            }
        } else {
            gen_and_save(&path, gen, format).await?
        };
        Ok(Self { data: cont.data })
    }
}

async fn gen_and_save<T, F, FO>(
    path: &impl AsRef<Path>,
    gen: F,
    format: DataFormat,
) -> Result<Container<T>>
where
    F: FnOnce() -> FO,
    FO: Future<Output = Result<T>>,
    T: Serialize,
{
    log::info!("Generating path {:?}", path.as_ref());
    let generated = gen().await?;
    let generated = Container {
        data: generated,
        time: Utc::now(),
    };
    let s = match format {
        DataFormat::Json => serde_json::to_vec(&generated).unwrap(),
        DataFormat::Bin => rmp_serde::to_vec(&generated).unwrap(),
    };
    let mut comp = path.as_ref().to_path_buf();
    comp.pop();
    std::fs::create_dir_all(comp).unwrap();
    std::fs::write(path.as_ref(), s).unwrap();
    Ok(generated)
}

enum DataFormat {
    Json,
    Bin,
}

#[derive(Debug, Serialize, Deserialize)]
struct Container<T> {
    data: T,
    time: DateTime<Utc>,
}
