mod cached_data;
mod consts;
mod error;
mod item_type;
mod paged_all;
mod stat;
use std::collections::{HashMap, HashSet};

use chrono::{NaiveDate, Utc};
use consts::DATE_FMT;
use error::Result;

use futures::{stream, StreamExt};
use itertools::Itertools;
use rust_eveonline_esi::{
    apis::{
        configuration::Configuration,
        market_api::{self, GetMarketsRegionIdHistoryParams, GetMarketsRegionIdTypesParams},
        search_api::{get_search, GetSearchParams, GetSearchSuccess},
        universe_api::{
            self, GetUniverseConstellationsConstellationIdParams,
            GetUniverseConstellationsConstellationIdSuccess, GetUniverseSystemsSystemIdParams,
            GetUniverseSystemsSystemIdSuccess, GetUniverseTypesTypeIdParams,
        },
    },
    models::GetMarketsRegionIdHistory200Ok,
};
use stat::{AverageStat, MedianStat};

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

    // all jita history
    let jita_history = history(
        &config,
        &all_types,
        the_forge,
        "cache/all_types_history.txt",
    )
    .await;

    let t0dt = find_region_id_system(&config, "t0dt-t").await?;

    // all t0dt history
    let t0dt_history = history(&config, &all_types, t0dt, "cache/all_types_t0dt.txt").await;

    // t0dt_history.iter().find(|x| x.id == 58848).map(|x| dbg!(x));

    // turn history into n day average
    let jita_types_average = averages(jita_history)
        .into_iter()
        .map(|x| (x.id, x.market_data))
        .collect::<HashMap<_, _>>();

    let types_average = averages(t0dt_history)
        .into_iter()
        .map(|x| (x.id, x.market_data))
        .collect::<HashMap<_, _>>();

    // pair
    let pairs = jita_types_average
        .into_iter()
        .map(|(k, v)| SystemMarketsItem {
            id: k,
            source: v,
            destination: types_average[&k].clone(),
        })
        .collect::<Vec<_>>();

    // find items such that
    let good_items = pairs
        .into_iter()
        .map(|x| {
            let margin = x.destination.average / x.source.average;
            (x, margin)
        })
        .filter(|x| x.0.destination.volume > 1.)
        .sorted_by(|x, y| y.1.partial_cmp(&x.1).unwrap())
        .take(30)
        .collect::<Vec<_>>();

    let items = stream::iter(good_items.iter())
        .map(|it| {
            let config = &config;
            let it = it.clone();
            async move {
                (
                    it.clone(),
                    get_item_name(config, it.0.id)
                        .await
                        .unwrap_or("???".to_string()),
                )
            }
        })
        .buffer_unordered(16)
        .collect::<Vec<_>>()
        .await;

    let format = items
        .iter()
        .map(|it| {
            format!(
                "{}; jita: {:.2}; t0dt: {:.2}; margin: {:.2}; volume dest: {:.2}; id: {}",
                it.1,
                it.0 .0.source.average,
                it.0 .0.destination.average,
                it.0 .1,
                it.0 .0.destination.volume,
                it.0 .0.id
            )
        })
        .collect::<Vec<_>>();
    println!("Maybe good items:\n{}", format.join("\n"));
    println!();

    let format = items
        .iter()
        .map(|it| format!("{}", it.1))
        .collect::<Vec<_>>();
    println!("Item names only:\n{}", format.join("\n"));

    Ok(())
}

#[derive(Debug, Clone)]
pub struct SystemMarketsItem {
    pub id: i32,
    pub source: MarketData,
    pub destination: MarketData,
}

async fn get_item_name(config: &Configuration, id: i32) -> Option<String> {
    {
        let result = universe_api::get_universe_types_type_id(
            config,
            GetUniverseTypesTypeIdParams {
                type_id: id,
                accept_language: None,
                datasource: None,
                if_none_match: None,
                language: None,
            },
        )
        .await;
        match result {
            Ok(t) => Some(t),
            Err(e) => {
                println!("error: {}, typeid: {}", e, id);
                None
            }
        }
    }
    .map(|x| x.entity.unwrap().into_result().unwrap().name)
}

async fn history(
    config: &Configuration,
    item_types: &Vec<i32>,
    region: i32,
    cache_name: &str,
) -> Vec<ItemType> {
    let mut data = CachedData::load_or_create_async(cache_name, || async {
        let hists = stream::iter(item_types)
            .map(|&item_type| {
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
    .data;

    // fill blanks
    for item in data.iter_mut() {
        let history = std::mem::replace(&mut item.history, Vec::new());
        let avg = history.iter().map(|x| x.average).median().unwrap_or(1.);
        let high = history.iter().map(|x| x.highest).median().unwrap_or(1.);
        let low = history.iter().map(|x| x.lowest).median().unwrap_or(1.);
        let order = history.iter().map(|x| x.order_count).median().unwrap_or(0);
        let vol = history.iter().map(|x| x.volume).median().unwrap_or(0);

        // take earliest date
        let mut dates = history
            .into_iter()
            .map(|x| {
                let date = NaiveDate::parse_from_str(x.date.as_str(), DATE_FMT).unwrap();
                (date, x)
            })
            .collect::<HashMap<_, _>>();
        let current_date = Utc::now().naive_utc().date();
        let min = match dates.keys().min() {
            Some(s) => s,
            None => {
                item.history.push(GetMarketsRegionIdHistory200Ok {
                    average: avg,
                    date: current_date.format(DATE_FMT).to_string(),
                    highest: high,
                    lowest: low,
                    order_count: order,
                    volume: vol,
                });
                continue;
            }
        };

        for date in min.iter_days() {
            if dates.contains_key(&date) {
                continue;
            }

            dates.insert(
                date,
                GetMarketsRegionIdHistory200Ok {
                    average: avg,
                    date: date.format(DATE_FMT).to_string(),
                    highest: high,
                    lowest: low,
                    order_count: 0,
                    volume: 0,
                },
            );

            if date == current_date {
                break;
            }
        }
        let new_history = dates.into_iter().sorted_by_key(|x| x.0);
        for it in new_history {
            item.history.push(it.1);
        }
    }

    data
}

fn averages(history: Vec<ItemType>) -> Vec<ItemTypeAveraged> {
    history
        .into_iter()
        .map(|tp| {
            let lastndays = tp.history.into_iter().rev().take(30).collect::<Vec<_>>();
            ItemTypeAveraged {
                id: tp.id,
                market_data: MarketData {
                    average: lastndays.iter().map(|x| x.average).median().unwrap(),
                    highest: lastndays.iter().map(|x| x.highest).median().unwrap(),
                    lowest: lastndays.iter().map(|x| x.lowest).median().unwrap(),
                    order_count: lastndays
                        .iter()
                        .map(|x| x.order_count as f64)
                        .median()
                        .unwrap(),
                    volume: lastndays.iter().map(|x| x.volume as f64).median().unwrap(),
                },
            }
        })
        .collect::<Vec<_>>()
}
