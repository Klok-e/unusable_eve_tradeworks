#![feature(bool_to_option)]

mod auth;
mod cached_data;
mod config;
mod consts;
mod error;
mod item_type;
mod logger;
mod paged_all;
mod retry;
mod stat;
use std::collections::HashMap;

use chrono::{NaiveDate, Utc};
use consts::DATE_FMT;
use error::Result;

use futures::{stream, StreamExt};
use item_type::Order;
use itertools::Itertools;
use ordered_float::NotNan;
use rust_eveonline_esi::{
    apis::{
        configuration::Configuration,
        market_api::{
            self, GetMarketsRegionIdHistoryError, GetMarketsRegionIdHistoryParams,
            GetMarketsRegionIdOrdersError, GetMarketsRegionIdOrdersParams,
            GetMarketsRegionIdTypesParams, GetMarketsStructuresStructureIdParams,
        },
        search_api::{self, get_search, GetCharactersCharacterIdSearchParams, GetSearchParams},
        universe_api::{
            self, GetUniverseConstellationsConstellationIdParams,
            GetUniverseConstellationsConstellationIdSuccess, GetUniverseStationsStationIdParams,
            GetUniverseStructuresStructureIdParams, GetUniverseSystemsSystemIdParams,
            GetUniverseSystemsSystemIdSuccess, GetUniverseTypesTypeIdParams,
        },
        Error,
    },
    models::{
        GetMarketsRegionIdHistory200Ok, GetMarketsRegionIdOrders200Ok, GetUniverseTypesTypeIdOk,
    },
};
use stat::{AverageStat, MedianStat};
use term_table::{row::Row, table_cell::TableCell, TableBuilder};
use tokio::{join, sync::Mutex};

use crate::{
    auth::Auth,
    cached_data::CachedData,
    config::{AuthConfig, Config},
    consts::ITEM_NAME_MAX_LENGTH,
    item_type::{ItemType, ItemTypeAveraged, MarketData, SystemMarketsItem, SystemMarketsItemData},
    paged_all::{get_all_pages, ToResult},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterInfo {
    pub scp: Vec<String>,
    pub jti: String,
    pub kid: String,
    pub sub: String,
    pub azp: String,
    pub tenant: String,
    pub tier: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    run().await
}

async fn run() -> Result<()> {
    logger::setup_logger()?;

    let config = Config::from_file_json("config.json")?;

    let program_config = AuthConfig::from_file("auth.json");
    let auth = Auth::load_or_request_token(&program_config).await;

    let mut esi_config = Configuration::new();
    esi_config.oauth_access_token = Some(auth.access_token.clone());

    // TODO: dangerous plese don't use in production
    let character_info =
        jsonwebtoken::dangerous_insecure_decode::<CharacterInfo>(auth.access_token.as_str())
            .unwrap()
            .claims;
    let character_id = character_info
        .sub
        .split(':')
        .nth(2)
        .unwrap()
        .parse()
        .unwrap();

    let pairs: Vec<SystemMarketsItemData> =
        CachedData::load_or_create_async("cache/path_data", || {
            let esi_config = &esi_config;
            let config = &config;
            async move {
                let source_region =
                    find_region_id_station(esi_config, config.source.clone(), character_id)
                        .await
                        .unwrap();

                // all item type ids
                let all_types = get_all_pages(
                    |page| {
                        let config = &esi_config;
                        async move {
                            market_api::get_markets_region_id_types(
                                config,
                                GetMarketsRegionIdTypesParams {
                                    region_id: source_region.region_id,
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
                .await;
                // let all_types = vec![16278];

                let dest_region =
                    find_region_id_station(esi_config, config.destination.clone(), character_id)
                        .await
                        .unwrap();

                // all jita history
                let source_history = history(esi_config, &all_types, source_region);

                // all t0dt history
                let dest_history = history(esi_config, &all_types, dest_region);

                let (source_history, dest_history) = join!(source_history, dest_history);

                // t0dt_history.iter().find(|x| x.id == 58848).map(|x| dbg!(x));

                // turn history into n day average
                let source_types_average = averages(config, source_history)
                    .into_iter()
                    .map(|x| (x.id, x.market_data))
                    .collect::<HashMap<_, _>>();

                let dest_types_average = averages(config, dest_history)
                    .into_iter()
                    .map(|x| (x.id, x.market_data))
                    .collect::<HashMap<_, _>>();

                // pair
                let pairs = source_types_average.into_iter().flat_map(|(k, v)| {
                    Some(SystemMarketsItem {
                        id: k,
                        source: v,
                        destination: match dest_types_average.get(&k) {
                            Some(x) => x.clone(),
                            None => {
                                log::warn!(
                                    "Destination history didn't have history for item: {}",
                                    k
                                );
                                return None;
                            }
                        },
                    })
                });

                stream::iter(pairs)
                    .map(|it| {
                        let it = it;
                        async move {
                            Some(SystemMarketsItemData {
                                desc: match get_item_stuff(esi_config, it.id).await {
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

    // find items such that
    let good_items = pairs
        .into_iter()
        .map(|x| {
            let sell_volume: i32 = x
                .destination
                .orders
                .iter()
                .filter(|x| !x.is_buy_order)
                .map(|x| x.volume_remain)
                .sum();

            let dest_sell_price = x
                .destination
                .orders
                .iter()
                .filter(|x| !x.is_buy_order)
                .min_by_key(|x| NotNan::new(x.price).unwrap())
                .map_or(x.destination.average, |x| x.price);

            let recommend_buy_vol = (x.destination.volume * config.rcmnd_fill_days)
                .max(1.)
                .floor() as i32;

            let src_sell_order_price = (!x.source.orders.iter().any(|x| !x.is_buy_order))
                .then_some(x.source.highest)
                .unwrap_or_else(|| {
                    let mut recommend_bought_volume = 0;
                    let mut max_price = 0.;
                    for order in x
                        .source
                        .orders
                        .iter()
                        .filter(|x| !x.is_buy_order)
                        .sorted_by_key(|x| NotNan::new(x.price).unwrap())
                    {
                        recommend_bought_volume += order.volume_remain.min(recommend_buy_vol);
                        max_price = order.price;
                        if recommend_buy_vol <= recommend_bought_volume {
                            break;
                        }
                    }
                    max_price
                });

            let buy_price = src_sell_order_price * (1. + config.broker_fee);
            let expenses = buy_price + x.desc.volume.unwrap() as f64 * config.freight_cost_iskm3;

            let sell_price = dest_sell_price * (1. - config.broker_fee - config.sales_tax);

            let margin = (sell_price - expenses) / expenses;

            let rough_profit = (sell_price - expenses) * recommend_buy_vol as f64;

            let filled_for_days =
                (x.destination.volume > 0.).then(|| 1. / x.destination.volume * sell_volume as f64);

            PairCalculatedData {
                market: x,
                margin,
                rough_profit,
                sell_volume,
                recommend_buy: recommend_buy_vol,
                expenses,
                sell_price,
                filled_for_days,
                src_buy_price: src_sell_order_price,
                dest_min_sell_price: dest_sell_price,
            }
        })
        .filter(|x| x.margin > config.margin_cutoff)
        .filter(|x| {
            x.market.source.volume > config.min_src_volume
                && x.market.destination.volume > config.min_dst_volume
        })
        .filter(|x| {
            if let Some(filled_for_days) = x.filled_for_days {
                filled_for_days < config.max_filled_for_days_cutoff
            } else {
                true
            }
        })
        .sorted_unstable_by_key(|x| NotNan::new(-x.rough_profit).unwrap())
        .take(config.items_take)
        .collect::<Vec<_>>();

    let rows = std::iter::once(Row::new(vec![
        TableCell::new("id"),
        TableCell::new("item name"),
        TableCell::new("src prc"),
        TableCell::new("dst prc"),
        TableCell::new("expenses"),
        TableCell::new("sell prc"),
        TableCell::new("margin"),
        TableCell::new("vlm src"),
        TableCell::new("vlm dst"),
        TableCell::new("on mkt"),
        TableCell::new("rough prft"),
        TableCell::new("rcmnd vlm"),
        TableCell::new("fld fr dy"),
    ]))
    .chain(good_items.iter().map(|it| {
        let short_name =
            it.market.desc.name[..(ITEM_NAME_MAX_LENGTH.min(it.market.desc.name.len()))].to_owned();
        Row::new(vec![
            TableCell::new(format!("{}", it.market.desc.type_id)),
            TableCell::new(short_name),
            TableCell::new(format!("{:.2}", it.src_buy_price)),
            TableCell::new(format!("{:.2}", it.dest_min_sell_price)),
            TableCell::new(format!("{:.2}", it.expenses)),
            TableCell::new(format!("{:.2}", it.sell_price)),
            TableCell::new(format!("{:.2}", it.margin)),
            TableCell::new(format!("{:.2}", it.market.source.volume)),
            TableCell::new(format!("{:.2}", it.market.destination.volume)),
            TableCell::new(format!("{:.2}", it.sell_volume)),
            TableCell::new(format!("{:.2}", it.rough_profit)),
            TableCell::new(format!("{}", it.recommend_buy)),
            TableCell::new(
                it.filled_for_days
                    .map_or("N/A".to_string(), |x| format!("{:.2}", x)),
            ),
        ])
    }))
    .collect::<Vec<_>>();
    let table = TableBuilder::new().rows(rows).build();
    println!("Maybe good items:\n{}", table.render());
    println!();

    let format = good_items
        .iter()
        .map(|it| format!("{} {}", it.market.desc.name, it.recommend_buy))
        .collect::<Vec<_>>();
    println!("Item names only:\n{}", format.join("\n"));

    Ok(())
}

pub struct PairCalculatedData {
    pub market: SystemMarketsItemData,
    pub margin: f64,
    pub rough_profit: f64,
    pub sell_volume: i32,
    pub recommend_buy: i32,
    pub expenses: f64,
    pub sell_price: f64,
    pub filled_for_days: Option<f64>,
    pub src_buy_price: f64,
    pub dest_min_sell_price: f64,
}

#[derive(Clone, Copy)]
pub struct StationIdData {
    pub station_id: StationId,
    pub system_id: i32,
    pub region_id: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Station {
    pub is_citadel: bool,
    pub name: String,
}
#[derive(Clone, Copy)]
pub struct StationId {
    pub is_citadel: bool,
    pub id: i64,
}

async fn find_region_id_station(
    config: &Configuration,
    station: Station,
    character_id: i32,
) -> Result<StationIdData> {
    // find system id
    let station_id = if station.is_citadel {
        search_api::get_characters_character_id_search(
            config,
            GetCharactersCharacterIdSearchParams {
                categories: vec!["structure".to_string()],
                character_id,
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
        .unwrap()[0]
    } else {
        get_search(
            config,
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
            config,
            GetUniverseSystemsSystemIdParams {
                system_id,
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
            config,
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
        system_id,
        region_id: region,
    })
}
async fn get_item_stuff(config: &Configuration, id: i32) -> Option<GetUniverseTypesTypeIdOk> {
    {
        retry::retry(|| async {
            universe_api::get_universe_types_type_id(
                config,
                GetUniverseTypesTypeIdParams {
                    type_id: id,
                    accept_language: None,
                    datasource: None,
                    if_none_match: None,
                    language: None,
                },
            )
            .await
        })
        .await
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
        let pages: Vec<GetMarketsRegionIdOrders200Ok> = get_all_pages(
            |page| async move {
                retry::retry::<_, _, _, Error<GetMarketsRegionIdOrdersError>>(|| async {
                    Ok(market_api::get_markets_region_id_orders(
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
                    .await?
                    .entity
                    .unwrap())
                })
                .await
            },
            1000,
        )
        .await;
        pages
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
    item_types: &[i32],
    station: StationIdData,
) -> Vec<ItemType> {
    async fn get_item_type_history(
        config: &Configuration,
        station: StationIdData,
        item_type: i32,
        station_orders: &Mutex<HashMap<i32, Vec<Order>>>,
    ) -> Option<ItemType> {
        let res: Option<ItemType> =
            retry::retry::<_, _, _, Error<GetMarketsRegionIdHistoryError>>(|| async {
                let hist_for_type = market_api::get_markets_region_id_history(
                    config,
                    GetMarketsRegionIdHistoryParams {
                        region_id: station.region_id,
                        type_id: item_type,
                        datasource: None,
                        if_none_match: None,
                    },
                )
                .await?
                .entity
                .unwrap();
                let hist_for_type = hist_for_type.into_result().unwrap();
                let mut dummy_empty_vec = Vec::new();
                Ok(ItemType {
                    id: item_type,
                    history: hist_for_type,
                    orders: std::mem::take(
                        station_orders
                            .lock()
                            .await
                            .get_mut(&item_type)
                            .unwrap_or(&mut dummy_empty_vec),
                    ),
                })
            })
            .await;
        res
    }

    async fn download_history(
        config: &Configuration,
        item_types: &[i32],
        station: StationIdData,
    ) -> Vec<ItemType> {
        log::info!("loading station orders");
        let station_orders = get_orders_station(config, station).await;
        log::info!("loading station orders finished");
        let station_orders =
            Mutex::new(station_orders.into_iter().into_group_map_by(|x| x.type_id));

        let hists =
            stream::iter(item_types)
                .map(|&item_type| {
                    let config = &config;
                    let station_orders = &station_orders;
                    async move {
                        get_item_type_history(config, station, item_type, station_orders).await
                    }
                })
                .buffer_unordered(16);
        hists
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
    }

    let mut data = download_history(config, item_types, station).await;

    // fill blanks
    for item in data.iter_mut() {
        let history = std::mem::take(&mut item.history);
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

fn averages(config: &Config, history: Vec<ItemType>) -> Vec<ItemTypeAveraged> {
    history
        .into_iter()
        .map(|tp| {
            let lastndays = tp
                .history
                .into_iter()
                .rev()
                .take(config.days_average)
                .collect::<Vec<_>>();
            ItemTypeAveraged {
                id: tp.id,
                market_data: MarketData {
                    average: lastndays.iter().map(|x| x.average).average().unwrap(),
                    highest: lastndays.iter().map(|x| x.highest).average().unwrap(),
                    lowest: lastndays.iter().map(|x| x.lowest).average().unwrap(),
                    order_count: lastndays
                        .iter()
                        .map(|x| x.order_count as f64)
                        .average()
                        .unwrap(),
                    volume: lastndays.iter().map(|x| x.volume as f64).average().unwrap(),
                    orders: tp.orders,
                },
            }
        })
        .collect::<Vec<_>>()
}
