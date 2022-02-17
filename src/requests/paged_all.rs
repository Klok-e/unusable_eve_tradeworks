use std::{fmt::Debug, future::Future, time::Duration};

use reqwest::StatusCode;
use rust_eveonline_esi::{
    apis::{
        killmails_api::GetKillmailsKillmailIdKillmailHashSuccess,
        market_api::{
            GetMarketsRegionIdHistorySuccess, GetMarketsRegionIdOrdersSuccess,
            GetMarketsRegionIdTypesSuccess, GetMarketsStructuresStructureIdSuccess,
        },
        routes_api::GetRouteOriginDestinationSuccess,
        search_api::{GetCharactersCharacterIdSearchSuccess, GetSearchSuccess},
        universe_api::{
            GetUniverseStationsStationIdSuccess, GetUniverseStructuresStructureIdSuccess,
            GetUniverseTypesTypeIdSuccess,
        },
    },
    models::{
        GetCharactersCharacterIdSearchOk, GetKillmailsKillmailIdKillmailHashOk,
        GetMarketsRegionIdHistory200Ok, GetMarketsRegionIdOrders200Ok,
        GetMarketsStructuresStructureId200Ok, GetSearchOk, GetUniverseStationsStationIdOk,
        GetUniverseStructuresStructureIdOk, GetUniverseTypesTypeIdOk,
    },
};

use crate::requests::retry::{self, Retry};

use super::error::EsiApiError;

pub async fn get_all_pages_simple<T, F, TOS>(get: F) -> Result<Vec<TOS>, super::error::EsiApiError>
where
    F: Fn(i32) -> T,
    T: Future<Output = Result<Vec<TOS>, super::error::EsiApiError>>,
{
    let mut all_types = Vec::new();
    let mut page = 1;
    loop {
        let types = retry::retry_simple(|| async {
            match get(page).await {
                Ok(x) => Ok(Retry::Success(x)),

                // 404 means that page is empty
                Err(EsiApiError {
                    status: StatusCode::NOT_FOUND,
                    ..
                }) => Ok(Retry::Success(Vec::new())),

                // error limited
                Err(e @ EsiApiError { status, .. })
                    if status == StatusCode::from_u16(420).unwrap() =>
                {
                    log::warn!("Error limited: {}. Retrying in 30 seconds...", e);
                    tokio::time::sleep(Duration::from_secs_f32(30.)).await;
                    Ok(Retry::Retry)
                }

                // common error for ccp servers
                Err(
                    e @ EsiApiError {
                        status: StatusCode::BAD_GATEWAY,
                        ..
                    },
                ) => {
                    log::warn!("Error: {}. Retrying...", e);
                    Ok(Retry::Retry)
                }
                Err(e) => Err(e),
            }
        })
        .await?;
        let mut types = types.unwrap_or_else(|| {
            log::warn!("Max retry count exceeded and error wasn't resolved.");
            Vec::new()
        });
        all_types.append(&mut types);
        if types.is_empty() {
            break;
        }

        page += 1;
    }
    Ok(all_types)
}

pub trait OnlyOk<T, E>: Sized {
    fn into_ok(self) -> Result<T, E>;
}

impl OnlyOk<Vec<i32>, GetMarketsRegionIdTypesSuccess> for GetMarketsRegionIdTypesSuccess {
    fn into_ok(self) -> Result<Vec<i32>, GetMarketsRegionIdTypesSuccess> {
        if let GetMarketsRegionIdTypesSuccess::Status200(types) = self {
            Ok(types)
        } else {
            Err(self)
        }
    }
}
impl OnlyOk<Vec<GetMarketsRegionIdHistory200Ok>, GetMarketsRegionIdHistorySuccess>
    for GetMarketsRegionIdHistorySuccess
{
    fn into_ok(
        self,
    ) -> Result<Vec<GetMarketsRegionIdHistory200Ok>, GetMarketsRegionIdHistorySuccess> {
        if let GetMarketsRegionIdHistorySuccess::Status200(ok) = self {
            Ok(ok)
        } else {
            Err(self)
        }
    }
}
impl OnlyOk<GetUniverseTypesTypeIdOk, GetUniverseTypesTypeIdSuccess>
    for GetUniverseTypesTypeIdSuccess
{
    fn into_ok(self) -> Result<GetUniverseTypesTypeIdOk, GetUniverseTypesTypeIdSuccess> {
        if let GetUniverseTypesTypeIdSuccess::Status200(ok) = self {
            Ok(ok)
        } else {
            Err(self)
        }
    }
}

impl OnlyOk<GetCharactersCharacterIdSearchOk, GetCharactersCharacterIdSearchSuccess>
    for GetCharactersCharacterIdSearchSuccess
{
    fn into_ok(
        self,
    ) -> Result<GetCharactersCharacterIdSearchOk, GetCharactersCharacterIdSearchSuccess> {
        if let GetCharactersCharacterIdSearchSuccess::Status200(ok) = self {
            Ok(ok)
        } else {
            Err(self)
        }
    }
}
impl OnlyOk<GetSearchOk, GetSearchSuccess> for GetSearchSuccess {
    fn into_ok(self) -> Result<GetSearchOk, GetSearchSuccess> {
        if let GetSearchSuccess::Status200(ok) = self {
            Ok(ok)
        } else {
            Err(self)
        }
    }
}
impl OnlyOk<GetUniverseStructuresStructureIdOk, GetUniverseStructuresStructureIdSuccess>
    for GetUniverseStructuresStructureIdSuccess
{
    fn into_ok(
        self,
    ) -> Result<GetUniverseStructuresStructureIdOk, GetUniverseStructuresStructureIdSuccess> {
        if let GetUniverseStructuresStructureIdSuccess::Status200(ok) = self {
            Ok(ok)
        } else {
            Err(self)
        }
    }
}
impl OnlyOk<GetUniverseStationsStationIdOk, GetUniverseStationsStationIdSuccess>
    for GetUniverseStationsStationIdSuccess
{
    fn into_ok(
        self,
    ) -> Result<GetUniverseStationsStationIdOk, GetUniverseStationsStationIdSuccess> {
        if let GetUniverseStationsStationIdSuccess::Status200(ok) = self {
            Ok(ok)
        } else {
            Err(self)
        }
    }
}
impl OnlyOk<Vec<GetMarketsStructuresStructureId200Ok>, GetMarketsStructuresStructureIdSuccess>
    for GetMarketsStructuresStructureIdSuccess
{
    fn into_ok(
        self,
    ) -> Result<Vec<GetMarketsStructuresStructureId200Ok>, GetMarketsStructuresStructureIdSuccess>
    {
        if let GetMarketsStructuresStructureIdSuccess::Status200(ok) = self {
            Ok(ok)
        } else {
            Err(self)
        }
    }
}
impl OnlyOk<Vec<GetMarketsRegionIdOrders200Ok>, GetMarketsRegionIdOrdersSuccess>
    for GetMarketsRegionIdOrdersSuccess
{
    fn into_ok(
        self,
    ) -> Result<Vec<GetMarketsRegionIdOrders200Ok>, GetMarketsRegionIdOrdersSuccess> {
        if let GetMarketsRegionIdOrdersSuccess::Status200(ok) = self {
            Ok(ok)
        } else {
            Err(self)
        }
    }
}
impl OnlyOk<GetKillmailsKillmailIdKillmailHashOk, GetKillmailsKillmailIdKillmailHashSuccess>
    for GetKillmailsKillmailIdKillmailHashSuccess
{
    fn into_ok(
        self,
    ) -> Result<GetKillmailsKillmailIdKillmailHashOk, GetKillmailsKillmailIdKillmailHashSuccess>
    {
        if let GetKillmailsKillmailIdKillmailHashSuccess::Status200(ok) = self {
            Ok(ok)
        } else {
            Err(self)
        }
    }
}

impl OnlyOk<Vec<i32>, GetRouteOriginDestinationSuccess> for GetRouteOriginDestinationSuccess {
    fn into_ok(self) -> Result<Vec<i32>, GetRouteOriginDestinationSuccess> {
        if let GetRouteOriginDestinationSuccess::Status200(ok) = self {
            Ok(ok)
        } else {
            Err(self)
        }
    }
}
