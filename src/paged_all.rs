use std::{fmt::Debug, future::Future};

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

pub async fn get_all_pages<T, F, TO, TOS, E>(get: F) -> Vec<TOS>
where
    F: Fn(i32) -> T,
    T: Future<Output = TO>,
    TO: ToResult<Vec<TOS>, E>,
    E: Debug,
{
    let mut all_types = Vec::new();
    let mut page = 1;
    loop {
        let types = get(page).await;
        let mut types = types.into_result().unwrap();
        all_types.append(&mut types);
        if types.is_empty() {
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
    fn into_result(self) -> Result<GetUniverseTypesTypeIdOk, GetUniverseTypesTypeIdSuccess> {
        if let GetUniverseTypesTypeIdSuccess::Status200(ok) = self {
            Ok(ok)
        } else {
            Err(self)
        }
    }
}

impl ToResult<GetCharactersCharacterIdSearchOk, GetCharactersCharacterIdSearchSuccess>
    for GetCharactersCharacterIdSearchSuccess
{
    fn into_result(
        self,
    ) -> Result<GetCharactersCharacterIdSearchOk, GetCharactersCharacterIdSearchSuccess> {
        if let GetCharactersCharacterIdSearchSuccess::Status200(ok) = self {
            Ok(ok)
        } else {
            Err(self)
        }
    }
}
impl ToResult<GetSearchOk, GetSearchSuccess> for GetSearchSuccess {
    fn into_result(self) -> Result<GetSearchOk, GetSearchSuccess> {
        if let GetSearchSuccess::Status200(ok) = self {
            Ok(ok)
        } else {
            Err(self)
        }
    }
}
impl ToResult<GetUniverseStructuresStructureIdOk, GetUniverseStructuresStructureIdSuccess>
    for GetUniverseStructuresStructureIdSuccess
{
    fn into_result(
        self,
    ) -> Result<GetUniverseStructuresStructureIdOk, GetUniverseStructuresStructureIdSuccess> {
        if let GetUniverseStructuresStructureIdSuccess::Status200(ok) = self {
            Ok(ok)
        } else {
            Err(self)
        }
    }
}
impl ToResult<GetUniverseStationsStationIdOk, GetUniverseStationsStationIdSuccess>
    for GetUniverseStationsStationIdSuccess
{
    fn into_result(
        self,
    ) -> Result<GetUniverseStationsStationIdOk, GetUniverseStationsStationIdSuccess> {
        if let GetUniverseStationsStationIdSuccess::Status200(ok) = self {
            Ok(ok)
        } else {
            Err(self)
        }
    }
}
impl ToResult<Vec<GetMarketsStructuresStructureId200Ok>, GetMarketsStructuresStructureIdSuccess>
    for GetMarketsStructuresStructureIdSuccess
{
    fn into_result(
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
impl ToResult<Vec<GetMarketsRegionIdOrders200Ok>, Option<GetMarketsRegionIdOrdersSuccess>>
    for Option<GetMarketsRegionIdOrdersSuccess>
{
    fn into_result(
        self,
    ) -> Result<Vec<GetMarketsRegionIdOrders200Ok>, Option<GetMarketsRegionIdOrdersSuccess>> {
        match self {
            Some(GetMarketsRegionIdOrdersSuccess::Status200(ok)) => Ok(ok),
            Some(_) => Err(self),
            None => Ok(Vec::new()),
        }
    }
}
impl ToResult<GetKillmailsKillmailIdKillmailHashOk, GetKillmailsKillmailIdKillmailHashSuccess>
    for GetKillmailsKillmailIdKillmailHashSuccess
{
    fn into_result(
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
impl ToResult<Vec<i32>, GetRouteOriginDestinationSuccess> for GetRouteOriginDestinationSuccess {
    fn into_result(self) -> Result<Vec<i32>, GetRouteOriginDestinationSuccess> {
        if let GetRouteOriginDestinationSuccess::Status200(ok) = self {
            Ok(ok)
        } else {
            Err(self)
        }
    }
}
