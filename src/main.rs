mod auth;
mod cached_data;
mod consts;
mod error;
mod item_type;
mod paged_all;
mod stat;
use std::collections::HashMap;

use chrono::{NaiveDate, Utc};
use consts::DATE_FMT;
use error::Result;

use futures::{stream, StreamExt};
use item_type::Order;
use itertools::Itertools;
use rust_eveonline_esi::{
    apis::{
        configuration::Configuration,
        market_api::{
            self, GetMarketsRegionIdHistoryParams, GetMarketsRegionIdOrdersParams,
            GetMarketsRegionIdTypesParams, GetMarketsStructuresStructureIdParams,
        },
        search_api::{
            self, get_search, GetCharactersCharacterIdSearchParams, GetSearchParams,
            GetSearchSuccess,
        },
        universe_api::{
            self, GetUniverseConstellationsConstellationIdParams,
            GetUniverseConstellationsConstellationIdSuccess, GetUniverseStationsStationIdParams,
            GetUniverseStructuresStructureIdParams, GetUniverseSystemsSystemIdParams,
            GetUniverseSystemsSystemIdSuccess, GetUniverseTypesTypeIdParams,
        },
    },
    models::{GetMarketsRegionIdHistory200Ok, GetUniverseTypesTypeIdOk},
};
use stat::{AverageStat, MedianStat};
use tokio::sync::Mutex;

use crate::{
    auth::Auth,
    cached_data::CachedData,
    consts::RETRIES,
    item_type::{ItemType, ItemTypeAveraged, MarketData, SystemMarketsItem, SystemMarketsItemData},
    paged_all::{get_all_pages, ToResult},
};
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterInfo {
    #[serde(rename = "CharacterID")]
    pub character_id: i32,
    #[serde(rename = "CharacterName")]
    pub character_name: String,
    #[serde(rename = "ExpiresOn")]
    pub expires_on: String,
    #[serde(rename = "Scopes")]
    pub scopes: String,
    #[serde(rename = "TokenType")]
    pub token_type: String,
    #[serde(rename = "CharacterOwnerHash")]
    pub character_owner_hash: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    run().await
}

async fn run() -> Result<()> {
    let auth = Auth::load_or_request_token().await;

    let mut config = Configuration::new();
    config.oauth_access_token = Some(auth.access_token.clone());
    let character_id = {
        let client = reqwest::Client::new();
        client
            .get("https://login.eveonline.com/oauth/verify")
            .header("Authorization", format!("Bearer {}", auth.access_token))
            .send()
            .await
            .unwrap()
            .json::<CharacterInfo>()
            .await
            .unwrap()
            .character_id
    };

    let the_forge = find_region_id_station(
        &config,
        Station {
            is_citadel: false,
            name: "Jita IV - Moon 4 - Caldari Navy Assembly Plant",
        },
        character_id,
    )
    .await?;

    // all item type ids
    let all_types = CachedData::load_or_create_async("cache/all_types.txt", || {
        get_all_pages(
            |page| {
                let config = &config;
                async move {
                    market_api::get_markets_region_id_types(
                        config,
                        GetMarketsRegionIdTypesParams {
                            region_id: the_forge.region_id,
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

    let t0dt = find_region_id_station(
        &config,
        Station {
            is_citadel: true,
            name: "T0DT-T - The Firstest Imperial Palace",
        },
        character_id,
    )
    .await?;

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
        });

    let pairs: Vec<SystemMarketsItemData> = CachedData::load_or_create_async("cache/pairs", || {
        let config = &config;
        async move {
            stream::iter(pairs)
                .map(|it| {
                    let it = it.clone();
                    async move {
                        Some(SystemMarketsItemData {
                            desc: match get_item_stuff(config, it.id).await {
                                Some(x) => x.into(),
                                None => return None,
                            },
                            source: it.source,
                            destination: it.destination,
                        })
                    }
                })
                .buffer_unordered(16)
                .collect::<Vec<Option<SystemMarketsItemData>>>()
                .await
                .into_iter()
                .flatten()
                .collect()
        }
    })
    .await
    .data;

    let sales_tax = 0.05;
    let broker_fee = 0.02;
    let freight_cost_iskm3 = 1500.;

    // find items such that
    let good_items = pairs
        .into_iter()
        .map(|x| {
            let buy_price = x.source.average * (1. + broker_fee);
            let expenses = buy_price + x.desc.volume.unwrap() as f64 * freight_cost_iskm3;

            let sell_price = x.destination.average * (1. - broker_fee - sales_tax);

            let margin = sell_price / expenses;
            (x, margin)
        })
        .filter(|x| x.0.destination.volume > 1.)
        .sorted_by(|x, y| y.1.partial_cmp(&x.1).unwrap())
        .take(30)
        .collect::<Vec<_>>();

    let format = good_items
        .iter()
        .map(|it| {
            let sell_volume:i32 = it.0.destination.orders.iter().map(|it|it.volume_remain).sum();
            format!(
                "{}; jita: {:.2}; t0dt: {:.2}; margin: {:.2}; volume dest: {:.2}; current volume: {}; id: {}",
                it.0.desc.name,
                it.0.source.highest,
                it.0.destination.lowest,
                it.1,
                it.0.destination.volume,
                sell_volume,
                it.0.desc.type_id
            )
        })
        .collect::<Vec<_>>();
    println!("Maybe good items:\n{}", format.join("\n"));
    println!();

    let format = good_items
        .iter()
        .map(|it| format!("{}", it.0.desc.name))
        .collect::<Vec<_>>();
    println!("Item names only:\n{}", format.join("\n"));

    Ok(())
}

#[derive(Clone, Copy)]
pub struct StationIdData {
    pub station_id: StationId,
    pub system_id: i32,
    pub region_id: i32,
}

pub struct Station<'a> {
    pub is_citadel: bool,
    pub name: &'a str,
}
#[derive(Clone, Copy)]
pub struct StationId {
    pub is_citadel: bool,
    pub id: i64,
}

async fn find_region_id_station(
    config: &Configuration,
    station: Station<'_>,
    character_id: i32,
) -> Result<StationIdData> {
    // find system id
    let station_id = if station.is_citadel {
        search_api::get_characters_character_id_search(
            config,
            GetCharactersCharacterIdSearchParams {
                categories: vec!["structure".to_string()],
                character_id: character_id,
                search: station.name.to_string(),
                accept_language: None,
                datasource: None,
                if_none_match: None,
                language: None,
                strict: None,
                token: None,
            },
        )
        .await?
        .entity
        .unwrap()
        .into_result()
        .unwrap()
        .structure
        .unwrap()
        .into_iter()
        .next()
        .unwrap()
    } else {
        get_search(
            &config,
            GetSearchParams {
                categories: vec!["station".to_string()],
                search: station.name.to_string(),
                accept_language: None,
                datasource: None,
                if_none_match: None,
                language: None,
                strict: None,
            },
        )
        .await?
        .entity
        .unwrap()
        .into_result()
        .unwrap()
        .station
        .unwrap()
        .into_iter()
        .next()
        .unwrap() as i64
    };
    let system_id = if station.is_citadel {
        universe_api::get_universe_structures_structure_id(
            config,
            GetUniverseStructuresStructureIdParams {
                structure_id: station_id,
                datasource: None,
                if_none_match: None,
                token: None,
            },
        )
        .await
        .unwrap()
        .entity
        .unwrap()
        .into_result()
        .unwrap()
        .solar_system_id
    } else {
        universe_api::get_universe_stations_station_id(
            config,
            GetUniverseStationsStationIdParams {
                station_id: station_id as i32,
                datasource: None,
                if_none_match: None,
            },
        )
        .await
        .unwrap()
        .entity
        .unwrap()
        .into_result()
        .unwrap()
        .system_id
    };

    // get system constellation
    let constellation;
    if let GetUniverseSystemsSystemIdSuccess::Status200(jita_const) =
        universe_api::get_universe_systems_system_id(
            &config,
            GetUniverseSystemsSystemIdParams {
                system_id: system_id,
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
    Ok(StationIdData {
        station_id: StationId {
            is_citadel: station.is_citadel,
            id: station_id,
        },
        system_id: system_id,
        region_id: region,
    })
}
async fn get_item_stuff(config: &Configuration, id: i32) -> Option<GetUniverseTypesTypeIdOk> {
    {
        let mut retry = 0;
        loop {
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
            break match result {
                Ok(t) => Some(t),
                Err(e) => {
                    retry += 1;
                    if retry < RETRIES {
                        println!("error: {}, typeid: {}; retry: {}", e, id, retry);
                        continue;
                    }
                    println!(
                        "error: {}, typeid: {}; retry: {}. No retries left",
                        e, id, retry
                    );
                    None
                }
            };
        }
    }
    .map(|x| x.entity.unwrap().into_result().unwrap())
}

async fn get_orders_station(config: &Configuration, station: StationIdData) -> Vec<Order> {
    if station.station_id.is_citadel {
        get_all_pages(
            |page| async move {
                market_api::get_markets_structures_structure_id(
                    config,
                    GetMarketsStructuresStructureIdParams {
                        structure_id: station.station_id.id,
                        datasource: None,
                        if_none_match: None,
                        page: Some(page),
                        token: None,
                    },
                )
                .await
                .unwrap()
                .entity
                .unwrap()
            },
            1000,
        )
        .await
        .into_iter()
        .map(|it| Order {
            duration: it.duration,
            is_buy_order: it.is_buy_order,
            issued: it.issued,
            location_id: it.location_id,
            min_volume: it.min_volume,
            order_id: it.order_id,
            price: it.price,
            type_id: it.type_id,
            volume_remain: it.volume_remain,
            volume_total: it.volume_total,
        })
        .collect::<Vec<_>>()
    } else {
        get_all_pages(
            |page| async move {
                market_api::get_markets_region_id_orders(
                    config,
                    GetMarketsRegionIdOrdersParams {
                        order_type: "all".to_string(),
                        region_id: station.region_id,
                        datasource: None,
                        if_none_match: None,
                        page: Some(page),
                        type_id: None,
                    },
                )
                .await
                .unwrap()
                .entity
                .unwrap()
            },
            1000,
        )
        .await
        .into_iter()
        .filter(|it| it.system_id == station.system_id)
        .map(|it| Order {
            duration: it.duration,
            is_buy_order: it.is_buy_order,
            issued: it.issued,
            location_id: it.location_id,
            min_volume: it.min_volume,
            order_id: it.order_id,
            price: it.price,
            type_id: it.type_id,
            volume_remain: it.volume_remain,
            volume_total: it.volume_total,
        })
        .collect::<Vec<_>>()
    }
}

async fn history(
    config: &Configuration,
    item_types: &Vec<i32>,
    station: StationIdData,
    cache_name: &str,
) -> Vec<ItemType> {
    let mut data = CachedData::load_or_create_async(cache_name, || async {
        let station_orders = get_orders_station(config, station).await;
        let station_orders = Mutex::new(
            station_orders
                .into_iter()
                .group_by(|x| x.type_id)
                .into_iter()
                .map(|(k, v)| (k, v.collect::<Vec<_>>()))
                .collect::<HashMap<_, _>>(),
        );

        let hists = stream::iter(item_types)
            .map(|&item_type| {
                let config = &config;
                let station_orders = &station_orders;
                async move {
                    let mut retries = 0;
                    loop {
                        // println!("get type {}", item_type);
                        let hist_for_type = {
                            let region_hist_result = market_api::get_markets_region_id_history(
                                config,
                                GetMarketsRegionIdHistoryParams {
                                    region_id: station.region_id,
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
                                    if retries > RETRIES {
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
                            orders: std::mem::replace(
                                station_orders.lock().await.get_mut(&item_type).unwrap(),
                                Vec::new(),
                            ),
                        });
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
                    orders: tp.orders,
                },
            }
        })
        .collect::<Vec<_>>()
}
