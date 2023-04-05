use std::{collections::HashMap, future::Future, path::Path};

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

#[derive(Debug, Default)]
pub struct CachedStuff {
    caches_updated: HashMap<String, bool>,
}

impl CachedStuff {
    pub fn new() -> Self {
        Default::default()
    }

    pub async fn load_or_create_async<T, F, FO>(
        &mut self,
        path: impl AsRef<Path>,
        depends: Vec<&str>,
        must_refresh: bool,
        timeout: Option<chrono::Duration>,
        gen: F,
    ) -> Result<T>
    where
        F: FnOnce() -> FO,
        FO: Future<Output = Result<T>>,
        T: Serialize + DeserializeOwned,
    {
        self.load_data_or_create_async(
            path.as_ref(),
            depends,
            DataFormat::Bin,
            must_refresh,
            timeout,
            gen,
        )
        .await
    }

    pub async fn load_or_create_json_async<T, F, FO>(
        &mut self,
        path: impl AsRef<Path>,
        depends: Vec<&str>,
        must_refresh: bool,
        timeout: Option<chrono::Duration>,
        gen: F,
    ) -> Result<T>
    where
        F: FnOnce() -> FO,
        FO: Future<Output = Result<T>>,
        T: Serialize + DeserializeOwned,
    {
        self.load_data_or_create_async(
            path.as_ref(),
            depends,
            DataFormat::Json,
            must_refresh,
            timeout,
            gen,
        )
        .await
    }

    async fn load_data_or_create_async<T, F, FO>(
        &mut self,
        path: &Path,
        depends: Vec<&str>,
        format: DataFormat,
        must_refresh: bool,
        timeout: Option<chrono::Duration>,
        gen: F,
    ) -> Result<T>
    where
        F: FnOnce() -> FO,
        FO: Future<Output = Result<T>>,
        T: Serialize + DeserializeOwned,
    {
        let path_str = path.to_str().unwrap().to_owned();
        self.caches_updated.insert(path_str.clone(), false);

        let were_depends_updated = depends.iter().any(|&x| self.caches_updated[x]);
        if path.exists() && !were_depends_updated && !must_refresh {
            let str = std::fs::read(path)?;
            let deser: Container<T> = match format {
                DataFormat::Json => serde_json::from_slice(str.as_slice())?,
                DataFormat::Bin => rmp_serde::from_read(str.as_slice()).unwrap(),
            };
            match timeout {
                Some(timeout) if deser.time + timeout > Utc::now() => {
                    log::info!("Path {:?} loaded", path);
                    return Ok(deser.data);
                }
                None => {
                    log::info!("Path {:?} loaded", path);
                    return Ok(deser.data);
                }
                _ => {}
            }
        }
        if were_depends_updated {
            log::info!("Path {:?} deps were updated", path);
        }

        self.caches_updated.insert(path_str, true);
        let cont = gen_and_save(&path, gen, format).await?;
        Ok(cont.data)
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
