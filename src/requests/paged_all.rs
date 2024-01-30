use std::{fmt::Debug, future::Future};

use reqwest::StatusCode;
use rust_eveonline_esi::{
    apis::{
        killmails_api::GetKillmailsKillmailIdKillmailHashSuccess,
        market_api::{
            GetMarketsPricesSuccess, GetMarketsRegionIdHistorySuccess,
            GetMarketsRegionIdOrdersSuccess, GetMarketsRegionIdTypesSuccess,
            GetMarketsStructuresStructureIdSuccess,
        },
        routes_api::GetRouteOriginDestinationSuccess,
        search_api::GetCharactersCharacterIdSearchSuccess,
        universe_api::{
            GetUniverseStationsStationIdSuccess, GetUniverseStructuresStructureIdSuccess,
            GetUniverseTypesTypeIdSuccess,
        },
        wallet_api::GetCharactersCharacterIdWalletTransactionsSuccess,
    },
    models::{
        GetCharactersCharacterIdSearchOk, GetCharactersCharacterIdWalletTransactions200Ok,
        GetKillmailsKillmailIdKillmailHashOk, GetMarketsPrices200Ok,
        GetMarketsRegionIdHistory200Ok, GetMarketsRegionIdOrders200Ok,
        GetMarketsStructuresStructureId200Ok, GetUniverseStationsStationIdOk,
        GetUniverseStructuresStructureIdOk, GetUniverseTypesTypeIdOk,
    },
};

use crate::requests::retry::{self, Retry};

use super::error::EsiApiError;

pub async fn get_all_pages<Ret, F, T>(get: F) -> Result<Vec<T>, super::error::EsiApiError>
where
    F: Fn(i32) -> Ret,
    Ret: Future<Output = Result<Vec<T>, super::error::EsiApiError>>,
    T: Debug,
{
    let mut all_items = Vec::new();
    let mut page = 1;
    loop {
        let page_items = retry::retry_smart(|| async {
            match get(page).await {
                Ok(x) => Ok(Retry::Success(x)),

                // 404 means that page is empty
                Err(EsiApiError {
                    status: StatusCode::NOT_FOUND,
                    ..
                }) => Ok(Retry::Success(Vec::new())),

                // 403 Forbidden is sometimes thrown randomly??? retry in this case
                Err(EsiApiError {
                    status: StatusCode::FORBIDDEN,
                    ..
                }) => {
                    log::warn!("403 Forbidden when getting pages, retrying...");
                    Ok(Retry::Retry)
                }

                Err(e) => Err(e),
            }
        })
        .await?;
        let mut page_items = page_items.unwrap_or_else(|| {
            log::warn!("Max retry count exceeded and error wasn't resolved.");
            Vec::new()
        });
        if page_items.is_empty() {
            break;
        }
        all_items.append(&mut page_items);

        page += 1;
    }
    Ok(all_items)
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

impl OnlyOk<Vec<GetMarketsPrices200Ok>, GetMarketsPricesSuccess> for GetMarketsPricesSuccess {
    fn into_ok(self) -> Result<Vec<GetMarketsPrices200Ok>, GetMarketsPricesSuccess> {
        if let GetMarketsPricesSuccess::Status200(ok) = self {
            Ok(ok)
        } else {
            Err(self)
        }
    }
}

impl
    OnlyOk<
        Vec<GetCharactersCharacterIdWalletTransactions200Ok>,
        GetCharactersCharacterIdWalletTransactionsSuccess,
    > for GetCharactersCharacterIdWalletTransactionsSuccess
{
    fn into_ok(
        self,
    ) -> Result<
        Vec<GetCharactersCharacterIdWalletTransactions200Ok>,
        GetCharactersCharacterIdWalletTransactionsSuccess,
    > {
        if let GetCharactersCharacterIdWalletTransactionsSuccess::Status200(ok) = self {
            Ok(ok)
        } else {
            Err(self)
        }
    }
}
