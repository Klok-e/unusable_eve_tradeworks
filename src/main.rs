mod cached_data;
mod error;
mod item_type;
mod paged_all;
use error::Result;

use rust_eveonline_esi::apis::{
    configuration::Configuration,
    market_api::{
        self, GetMarketsGroupsParams, GetMarketsGroupsSuccess, GetMarketsRegionIdHistoryParams,
        GetMarketsRegionIdHistorySuccess, GetMarketsRegionIdTypesParams,
        GetMarketsRegionIdTypesSuccess,
    },
    search_api::{self, get_search, GetSearchParams, GetSearchSuccess},
    universe_api::{
        self, GetUniverseConstellationsConstellationIdParams,
        GetUniverseConstellationsConstellationIdSuccess, GetUniverseSystemsSystemIdParams,
        GetUniverseSystemsSystemIdSuccess,
    },
};

use crate::{
    cached_data::CachedData,
    item_type::ItemType,
    paged_all::{get_all_pages, ToResult},
};

#[tokio::main]
async fn main() -> Result<()> {
    run().await
}

async fn run() -> Result<()> {
    let config = Configuration::new();

    // find jita id
    let search_res = get_search(
        &config,
        GetSearchParams {
            categories: vec!["solar_system".to_string()],
            search: "jita".to_string(),
            accept_language: None,
            datasource: None,
            if_none_match: None,
            language: None,
            strict: None,
        },
    )
    .await?
    .entity
    .unwrap();
    let jita;
    if let GetSearchSuccess::Status200(search_res) = search_res {
        jita = search_res.solar_system.unwrap()[0];
    } else {
        panic!();
    }

    // get jita constellation
    let constellation;
    if let GetUniverseSystemsSystemIdSuccess::Status200(jita_const) =
        universe_api::get_universe_systems_system_id(
            &config,
            GetUniverseSystemsSystemIdParams {
                system_id: jita,
                accept_language: None,
                datasource: None,
                if_none_match: None,
                language: None,
            },
        )
        .await
        .unwrap()
        .entity
        .unwrap()
    {
        constellation = jita_const.constellation_id;
    } else {
        panic!();
    }

    // get jita constellation
    let the_forge;
    if let GetUniverseConstellationsConstellationIdSuccess::Status200(ok) =
        universe_api::get_universe_constellations_constellation_id(
            &config,
            GetUniverseConstellationsConstellationIdParams {
                constellation_id: constellation,
                accept_language: None,
                datasource: None,
                if_none_match: None,
                language: None,
            },
        )
        .await
        .unwrap()
        .entity
        .unwrap()
    {
        the_forge = ok.region_id;
    } else {
        panic!();
    }

    // all item type ids
    let all_types = CachedData::load_or_create_async("all_types", || {
        get_all_pages(
            |page| {
                let config = &config;
                async move {
                    market_api::get_markets_region_id_types(
                        config,
                        GetMarketsRegionIdTypesParams {
                            region_id: the_forge,
                            datasource: None,
                            if_none_match: None,
                            page: Some(page),
                        },
                    )
                    .await
                    .unwrap()
                    .entity
                    .unwrap()
                }
            },
            1000,
        )
    })
    .await
    .data;

    // all history
    let data = CachedData::load_or_create_async("all_types_history", || async {
        let mut all_histories = Vec::new();
        for item_type in all_types {
            let hist_for_type = market_api::get_markets_region_id_history(
                &config,
                GetMarketsRegionIdHistoryParams {
                    region_id: the_forge,
                    type_id: item_type,
                    datasource: None,
                    if_none_match: None,
                },
            )
            .await
            .unwrap()
            .entity
            .unwrap();
            let hist_for_type = hist_for_type.into_result().unwrap();
            all_histories.push(ItemType {
                id: item_type,
                history: hist_for_type,
            });
        }
        all_histories
    })
    .await
    .data;

    Ok(())
}
