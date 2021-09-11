mod cached_data;
mod error;
mod item_type;
mod paged_all;
use error::Result;

use futures::{stream, StreamExt};
use rust_eveonline_esi::apis::{
    configuration::Configuration,
    market_api::{self, GetMarketsRegionIdHistoryParams, GetMarketsRegionIdTypesParams},
    search_api::{get_search, GetSearchParams, GetSearchSuccess},
    universe_api::{
        self, GetUniverseConstellationsConstellationIdParams,
        GetUniverseConstellationsConstellationIdSuccess, GetUniverseSystemsSystemIdParams,
        GetUniverseSystemsSystemIdSuccess,
    },
};
use statrs::statistics::{Data, Median};

use crate::{
    cached_data::CachedData,
    item_type::{ItemType, ItemTypeAveraged, MarketData},
    paged_all::{get_all_pages, ToResult},
};

#[tokio::main]
async fn main() -> Result<()> {
    run().await
}

async fn find_region_id_system(config: &Configuration, sys_name: &str) -> Result<i32> {
    // find system id
    let search_res = get_search(
        &config,
        GetSearchParams {
            categories: vec!["solar_system".to_string()],
            search: sys_name.to_string(),
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

    // get system constellation
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

    // get system region
    let region;
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
        region = ok.region_id;
    } else {
        panic!();
    }
    Ok(region)
}

async fn run() -> Result<()> {
    let config = Configuration::new();

    let the_forge = find_region_id_system(&config, "jita").await?;

    // all item type ids
    let all_types = CachedData::load_or_create_async("cache/all_types.txt", || {
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
    let types_history: Vec<ItemType> =
        history(&config, all_types, the_forge, "cache/all_types_history.txt").await;

    // turn history into 7 day running average
    let _types_average = averages(types_history);

    Ok(())
}

async fn history(
    config: &Configuration,
    item_types: Vec<i32>,
    region: i32,
    cache_name: &str,
) -> Vec<ItemType> {
    CachedData::load_or_create_async(cache_name, || async {
        let hists = stream::iter(item_types)
            .map(|item_type| {
                let config = &config;
                async move {
                    {
                        let mut retries = 0;
                        loop {
                            println!("get type {}", item_type);
                            let hist_for_type = {
                                let region_hist_result = market_api::get_markets_region_id_history(
                                    config,
                                    GetMarketsRegionIdHistoryParams {
                                        region_id: region,
                                        type_id: item_type,
                                        datasource: None,
                                        if_none_match: None,
                                    },
                                )
                                .await;
                                // retry on error
                                match region_hist_result {
                                    Ok(t) => t,
                                    Err(e) => {
                                        retries += 1;
                                        if retries > 2 {
                                            break None;
                                        }
                                        println!("error {}. Retrying {} ...", e, retries);
                                        continue;
                                    }
                                }
                            }
                            .entity
                            .unwrap();
                            let hist_for_type = hist_for_type.into_result().unwrap();
                            break Some(ItemType {
                                id: item_type,
                                history: hist_for_type,
                            });
                        }
                    }
                }
            })
            .buffer_unordered(16);
        hists
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
    })
    .await
    .data
}

fn averages(history: Vec<ItemType>) -> Vec<ItemTypeAveraged> {
    history
        .into_iter()
        .map(|tp| {
            let lastndays = tp.history.into_iter().rev().take(7).collect::<Vec<_>>();
            ItemTypeAveraged {
                id: tp.id,
                market_data: MarketData {
                    average: Data::new(lastndays.iter().map(|x| x.average).collect::<Vec<_>>())
                        .median(),
                    highest: Data::new(lastndays.iter().map(|x| x.highest).collect::<Vec<_>>())
                        .median(),
                    lowest: Data::new(lastndays.iter().map(|x| x.lowest).collect::<Vec<_>>())
                        .median(),
                    order_count: Data::new(
                        lastndays
                            .iter()
                            .map(|x| x.order_count as f64)
                            .collect::<Vec<_>>(),
                    )
                    .median() as i64,
                    volume: Data::new(
                        lastndays
                            .iter()
                            .map(|x| x.volume as f64)
                            .collect::<Vec<_>>(),
                    )
                    .median() as i64,
                },
            }
        })
        .collect::<Vec<_>>()
}
