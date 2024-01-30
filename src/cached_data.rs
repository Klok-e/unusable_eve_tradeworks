use std::{
    collections::HashMap,
    future::Future,
    path::{Path, PathBuf},
};

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

#[derive(Debug)]
pub struct CachedStuff {
    caches_updated: HashMap<String, bool>,
    path: PathBuf,
}

impl Default for CachedStuff {
    fn default() -> Self {
        Self {
            caches_updated: Default::default(),
            path: "cache".into(),
        }
    }
}

impl CachedStuff {
    pub fn new() -> Self {
        Default::default()
    }

    pub async fn load_or_create_async<T, F, FO>(
        &mut self,
        path: impl AsRef<Path>,
        depends: Vec<&str>,
        timeout: Option<chrono::Duration>,
        gen: F,
    ) -> Result<T>
    where
        F: FnOnce(Option<T>) -> FO,
        FO: Future<Output = Result<T>>,
        T: Serialize + DeserializeOwned,
    {
        self.load_data_or_create_async(
            &self.path.join(path.as_ref()),
            depends,
            DataFormat::Bin,
            timeout,
            gen,
        )
        .await
    }

    pub async fn load_or_create_json_async<T, F, FO>(
        &mut self,
        path: impl AsRef<Path>,
        depends: Vec<&str>,
        timeout: Option<chrono::Duration>,
        gen: F,
    ) -> Result<T>
    where
        F: FnOnce(Option<T>) -> FO,
        FO: Future<Output = Result<T>>,
        T: Serialize + DeserializeOwned,
    {
        self.load_data_or_create_async(
            &self.path.join(path.as_ref()),
            depends,
            DataFormat::Json,
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
        timeout: Option<chrono::Duration>,
        gen: F,
    ) -> Result<T>
    where
        F: FnOnce(Option<T>) -> FO,
        FO: Future<Output = Result<T>>,
        T: Serialize + DeserializeOwned,
    {
        let were_depends_updated = depends
            .iter()
            .any(|&x| *self.caches_updated.get(x).unwrap_or(&false));
        let mut deser_opt: Option<Container<T>> = None;
        if path.exists() && !were_depends_updated {
            let str = std::fs::read(path)?;
            let deser: Result<Container<T>> = match format {
                DataFormat::Json => serde_json::from_slice(str.as_slice()).map_err(Into::into),
                DataFormat::Bin => rmp_serde::from_read(str.as_slice()).map_err(Into::into),
            };
            match deser {
                Ok(deser) => {
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
                    deser_opt = Some(deser);
                }
                Err(err) => {
                    log::warn!("Couldn't deserialize cached value for {path:?}: {err}");
                }
            }
        }
        if were_depends_updated {
            log::info!("Path {:?} deps were updated", path);
        }

        let cont = self.gen_and_save(&path, gen, format, deser_opt).await?;
        Ok(cont.data)
    }

    async fn gen_and_save<T, F, FO>(
        &mut self,
        path: &impl AsRef<Path>,
        gen: F,
        format: DataFormat,
        previous: Option<Container<T>>,
    ) -> Result<Container<T>>
    where
        F: FnOnce(Option<T>) -> FO,
        FO: Future<Output = Result<T>>,
        T: Serialize,
    {
        log::info!("Generating path {:?}", path.as_ref());
        let generated = gen(previous.map(|x| x.data)).await?;
        let generated = self.save(generated, format, path);
        Ok(generated)
    }

    pub fn save_json<T>(&mut self, generated: T, path: &impl AsRef<Path>) -> T
    where
        T: Serialize,
    {
        self.save(generated, DataFormat::Json, path).data
    }

    fn save<T>(&mut self, generated: T, format: DataFormat, path: &impl AsRef<Path>) -> Container<T>
    where
        T: Serialize,
    {
        self.caches_updated
            .insert(path.as_ref().to_str().unwrap().to_string(), true);

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
        generated
    }
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
