use std::collections::HashMap;

use crate::{
    consts::BUFFER_UNORDERED,
    error::Result,
    item_type::ItemType,
    paged_all::{get_all_pages, ToResult},
    retry, StationId,
};
use crate::{consts::DATE_FMT, Station, StationIdData};
use chrono::{Duration, NaiveDate, Utc};

use crate::item_type::Order;
use crate::stat::MedianStat;
use futures::{stream, StreamExt};
use itertools::Itertools;
use ordered_float::NotNan;
use rust_eveonline_esi::{
    apis::{
        configuration::Configuration,
        market_api::{
            self, GetMarketsRegionIdHistoryError, GetMarketsRegionIdHistoryParams,
            GetMarketsRegionIdOrdersError, GetMarketsRegionIdOrdersParams,
            GetMarketsStructuresStructureIdParams,
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

use tokio::sync::Mutex;

pub async fn find_region_id_station(
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
pub async fn get_item_stuff(config: &Configuration, id: i32) -> Option<GetUniverseTypesTypeIdOk> {
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

pub async fn get_orders_station(config: &Configuration, station: StationIdData) -> Vec<Order> {
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

pub async fn history(
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
                .buffer_unordered(BUFFER_UNORDERED);
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
        let avg = history
            .iter()
            .map(|x| x.average)
            .map(to_not_nan)
            .median()
            .unwrap_or_else(|| to_not_nan(1.));
        let high = history
            .iter()
            .map(|x| x.highest)
            .map(to_not_nan)
            .median()
            .unwrap_or_else(|| to_not_nan(1.));
        let low = history
            .iter()
            .map(|x| x.lowest)
            .map(to_not_nan)
            .median()
            .unwrap_or_else(|| to_not_nan(1.));

        // take earliest date
        let mut dates = history
            .into_iter()
            .map(|x| {
                let date = NaiveDate::parse_from_str(x.date.as_str(), DATE_FMT).unwrap();
                (date, x)
            })
            .collect::<HashMap<_, _>>();
        let current_date = Utc::now().naive_utc().date();
        let past_date = current_date - Duration::days(360);

        for date in past_date.iter_days() {
            if dates.contains_key(&date) {
                continue;
            }

            dates.insert(
                date,
                GetMarketsRegionIdHistory200Ok {
                    average: *avg,
                    date: date.format(DATE_FMT).to_string(),
                    highest: *high,
                    lowest: *low,
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

pub fn to_not_nan(x: f64) -> NotNan<f64> {
    NotNan::new(x).unwrap()
}
