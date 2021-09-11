use std::{fmt::Debug, future::Future};

use rust_eveonline_esi::{apis::{market_api::{GetMarketsRegionIdHistorySuccess, GetMarketsRegionIdTypesSuccess}, universe_api::GetUniverseTypesTypeIdSuccess}, models::{GetMarketsRegionIdHistory200Ok, GetUniverseTypesTypeIdOk}};

pub async fn get_all_pages<T, F, TO, TOS, E>(get: F, max_items_batch: usize) -> Vec<TOS>
where
    F: Fn(i32) -> T,
    T: Future<Output = TO>,
    TO: ToResult<Vec<TOS>, E>,
    E: Debug,
{
    let mut all_types = Vec::new();
    let mut page = 1;
    loop {
        println!("Get page {}", page);
        let types = get(page).await;
        let mut types = types.into_result().unwrap();
        let var_name = types.len() < max_items_batch;
        all_types.append(&mut types);
        if var_name {
            break;
        }

        page += 1;
    }
    all_types
}

pub trait ToResult<T, E>: Sized {
    fn into_result(self) -> Result<T, E>;
}

impl ToResult<Vec<i32>, GetMarketsRegionIdTypesSuccess> for GetMarketsRegionIdTypesSuccess {
    fn into_result(self) -> Result<Vec<i32>, GetMarketsRegionIdTypesSuccess> {
        if let GetMarketsRegionIdTypesSuccess::Status200(types) = self {
            Ok(types)
        } else {
            Err(self)
        }
    }
}
impl ToResult<Vec<GetMarketsRegionIdHistory200Ok>, GetMarketsRegionIdHistorySuccess>
    for GetMarketsRegionIdHistorySuccess
{
    fn into_result(
        self,
    ) -> Result<Vec<GetMarketsRegionIdHistory200Ok>, GetMarketsRegionIdHistorySuccess> {
        if let GetMarketsRegionIdHistorySuccess::Status200(ok) = self {
            Ok(ok)
        } else {
            Err(self)
        }
    }
}
impl ToResult<GetUniverseTypesTypeIdOk, GetUniverseTypesTypeIdSuccess>
    for GetUniverseTypesTypeIdSuccess
{
    fn into_result(
        self,
    ) -> Result<GetUniverseTypesTypeIdOk, GetUniverseTypesTypeIdSuccess> {
        if let GetUniverseTypesTypeIdSuccess::Status200(ok) = self {
            Ok(ok)
        } else {
            Err(self)
        }
    }
}
