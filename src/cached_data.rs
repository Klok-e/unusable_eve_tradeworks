use std::{future::Future, path::Path};

use rmp_serde;
use serde::{de::DeserializeOwned, Serialize};

#[derive(Debug)]
pub struct CachedData<T> {
    pub data: T,
}

impl<T> CachedData<T>
where
    T: Serialize + DeserializeOwned,
{
    pub async fn load_or_create_async<F, FO>(path: impl AsRef<Path>, gen: F) -> Self
    where
        F: FnOnce() -> FO,
        FO: Future<Output = T>,
    {
        let data = if path.as_ref().exists() {
            println!("loading path {:?} ...", path.as_ref());
            let str = std::fs::read(path.as_ref()).unwrap();
            let deser = rmp_serde::from_read(str.as_slice()).unwrap();
            deser
        } else {
            println!("gen path {:?}", path.as_ref());
            let generated = gen().await;
            let s = rmp_serde::to_vec(&generated).unwrap();
            let mut comp = path.as_ref().to_path_buf();
            comp.pop();
            std::fs::create_dir_all(comp).unwrap();
            std::fs::write(path.as_ref(), s).unwrap();
            generated
        };
        println!("finished loading or gen {:?}", path.as_ref());
        Self { data }
    }
}
