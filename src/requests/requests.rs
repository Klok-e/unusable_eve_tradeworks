use std::collections::HashMap;

use crate::{
    consts::DATE_FMT,
    item_type::MarketsRegionHistory,
    requests::{paged_all::get_all_pages_simple, retry::IntoCcpError},
    Station, StationIdData,
};
use crate::{
    consts::{self, BUFFER_UNORDERED},
    item_type::ItemType,
    requests::paged_all::{get_all_pages, OnlyOk},
    requests::retry,
    StationId,
};
use chrono::{Duration, NaiveDate, NaiveDateTime, Utc};

use super::error::Result;
use crate::item_type::Order;
use crate::stat::MedianStat;

use futures::{stream, StreamExt};
use itertools::Itertools;
use ordered_float::NotNan;
use rust_eveonline_esi::{
    apis::{
        configuration::Configuration,
        killmails_api::{
            self, GetKillmailsKillmailIdKillmailHashError, GetKillmailsKillmailIdKillmailHashParams,
        },
        market_api::{
            self, GetMarketsRegionIdHistoryError, GetMarketsRegionIdHistoryParams,
            GetMarketsRegionIdOrdersError, GetMarketsRegionIdOrdersParams,
            GetMarketsRegionIdOrdersSuccess, GetMarketsStructuresStructureIdParams,
        },
        routes_api::{self, GetRouteOriginDestinationError, GetRouteOriginDestinationParams},
        search_api::{self, get_search, GetCharactersCharacterIdSearchParams, GetSearchParams},
        universe_api::{
            self, GetUniverseConstellationsConstellationIdParams,
            GetUniverseConstellationsConstellationIdSuccess, GetUniverseStationsStationIdParams,
            GetUniverseStructuresStructureIdParams, GetUniverseSystemsSystemIdParams,
            GetUniverseSystemsSystemIdSuccess, GetUniverseTypesTypeIdParams,
        },
        Error, ResponseContent,
    },
    models::{
        get_markets_region_id_orders_200_ok, GetKillmailsKillmailIdKillmailHashItem,
        GetKillmailsKillmailIdKillmailHashItemsItem, GetMarketsRegionIdOrders200Ok,
        GetUniverseTypesTypeIdOk,
    },
};

use tokio::sync::{Mutex, RwLock};

pub struct EsiRequestsService<'a> {
    pub config: &'a Configuration,
}
impl<'a> EsiRequestsService<'a> {
    pub fn new(config: &'a Configuration) -> Self {
        Self { config }
    }

    pub async fn find_region_id_station(
        &self,
        station: Station,
        character_id: i32,
    ) -> Result<StationIdData> {
        // find system id
        let station_id = if station.is_citadel {
            search_api::get_characters_character_id_search(
                self.config,
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
            .into_ok()
            .unwrap()
            .structure
            .unwrap()[0]
        } else {
            get_search(
                self.config,
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
            .into_ok()
            .unwrap()
            .station
            .unwrap()
            .into_iter()
            .next()
            .unwrap() as i64
        };
        let system_id = if station.is_citadel {
            universe_api::get_universe_structures_structure_id(
                self.config,
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
            .into_ok()
            .unwrap()
            .solar_system_id
        } else {
            universe_api::get_universe_stations_station_id(
                self.config,
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
            .into_ok()
            .unwrap()
            .system_id
        };

        // get system constellation
        let constellation = if let GetUniverseSystemsSystemIdSuccess::Status200(jita_const) =
            universe_api::get_universe_systems_system_id(
                self.config,
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
            jita_const.constellation_id
        } else {
            panic!();
        };

        // get system region
        let region = if let GetUniverseConstellationsConstellationIdSuccess::Status200(ok) =
            universe_api::get_universe_constellations_constellation_id(
                self.config,
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
            ok.region_id
        } else {
            panic!();
        };
        Ok(StationIdData {
            station_id: StationId {
                is_citadel: station.is_citadel,
                id: station_id,
            },
            system_id,
            region_id: region,
        })
    }
    pub async fn get_item_stuff(&self, id: i32) -> Option<GetUniverseTypesTypeIdOk> {
        {
            retry::retry(|| async {
                universe_api::get_universe_types_type_id(
                    self.config,
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
        .map(|x| x.entity.unwrap().into_ok().unwrap())
    }

    pub async fn get_orders_station(&self, station: StationIdData) -> Result<Vec<Order>> {
        // download all orders
        log::info!("Downloading region orders...");
        let pages: Vec<GetMarketsRegionIdOrders200Ok> = get_all_pages_simple(|page| async move {
            let orders = market_api::get_markets_region_id_orders(
                self.config,
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
            .unwrap();

            Ok(orders.into_ok().unwrap())
        })
        .await?;
        log::info!("All region orders downloaded. Calculating distances...");

        let distance_cache = RwLock::new(HashMap::new());

        // calculate distance to all buy orders
        let pages: Vec<(GetMarketsRegionIdOrders200Ok, Option<usize>)> = stream::iter(pages)
            .map(|x| async {
                let dist_if_buy = if x.is_buy_order {
                    let map = distance_cache.read().await;
                    let dist = match map.get(&x.system_id) {
                        Some(&dist) => dist,
                        None => {
                            drop(map);
                            log::debug!(
                                "Distance between origin {} and dest {} not in cache, making request...",
                                station.system_id,
                                x.system_id
                            );
                            let dist =
                                retry::retry::<_, _, _, Error<GetRouteOriginDestinationError>>(
                                    || async {
                                        Ok(routes_api::get_route_origin_destination(
                                            self.config,
                                            GetRouteOriginDestinationParams {
                                                destination: x.system_id,
                                                origin: station.system_id,
                                                avoid: None,
                                                connections: None,
                                                datasource: None,
                                                flag: None,
                                                if_none_match: None,
                                            },
                                        )
                                        .await?
                                        .entity
                                        .unwrap())
                                    },
                                )
                                .await
                                .map(|x| Some(x.into_ok().unwrap().len()))
                                .unwrap_or_else(|| {
                                    log::warn!(
                                        "Couldn't calculate distance between origin {} and dest {}",
                                        station.system_id,
                                        x.system_id
                                    );
                                    None
                                });
                            log::debug!(
                                "Inserting distance between origin {} and dest {} into cache...",
                                station.system_id,
                                x.system_id
                            );
                            let mut map = distance_cache.write().await;
                            map.insert(x.system_id,dist);
                            dist
                        }
                    };

                    log::debug!(
                        "Distance between origin {} and dest {} is {:?}",
                        station.system_id,
                        x.system_id,
                        dist
                    );

                    dist
                } else {
                    None
                };

                (x, dist_if_buy)
            })
            .buffer_unordered(BUFFER_UNORDERED)
            .collect::<Vec<_>>()
            .await;
        log::info!("All distances calculated.");

        let mut orders_in_station = pages
            .into_iter()
            .filter(|it| {
                it.0.location_id == station.station_id.id
                    || (if let Some(dist) = it.1 {
                        it.0.is_buy_order
                            && (match it.0.range {
                                get_markets_region_id_orders_200_ok::Range::Station => 0,
                                get_markets_region_id_orders_200_ok::Range::Solarsystem => 0,
                                get_markets_region_id_orders_200_ok::Range::_1 => 1,
                                get_markets_region_id_orders_200_ok::Range::_2 => 2,
                                get_markets_region_id_orders_200_ok::Range::_3 => 3,
                                get_markets_region_id_orders_200_ok::Range::_4 => 4,
                                get_markets_region_id_orders_200_ok::Range::_5 => 5,
                                get_markets_region_id_orders_200_ok::Range::_10 => 10,
                                get_markets_region_id_orders_200_ok::Range::_20 => 20,
                                get_markets_region_id_orders_200_ok::Range::_30 => 30,
                                get_markets_region_id_orders_200_ok::Range::_40 => 40,
                                get_markets_region_id_orders_200_ok::Range::Region => 40,
                            }) >= dist - 1
                    } else {
                        false
                    })
            })
            .map(|it| Order {
                duration: it.0.duration,
                is_buy_order: it.0.is_buy_order,
                issued: it.0.issued,
                location_id: it.0.location_id,
                min_volume: it.0.min_volume,
                order_id: it.0.order_id,
                price: it.0.price,
                type_id: it.0.type_id,
                volume_remain: it.0.volume_remain,
                volume_total: it.0.volume_total,
            })
            .collect::<Vec<_>>();

        if station.station_id.is_citadel {
            log::info!("Loading citadel orders...");
            let mut orders_in_citadel = get_all_pages(|page| async move {
                market_api::get_markets_structures_structure_id(
                    self.config,
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
            })
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
            .collect::<Vec<_>>();
            log::info!("All citadel orders loaded.");

            orders_in_station.append(&mut orders_in_citadel);
        }

        // some orders can be both regional and placed in a citadel
        // so there may be duplicates
        orders_in_station.sort_unstable_by_key(|x| x.order_id);
        orders_in_station.dedup_by_key(|x| x.order_id);

        Ok(orders_in_station)
    }

    pub async fn history(
        &self,
        item_types: &[i32],
        station: StationIdData,
    ) -> Result<Vec<ItemType>> {
        let mut data = self.download_history(item_types, station).await?;

        // fill blanks
        for item in data.iter_mut() {
            let history = std::mem::take(&mut item.history);
            let avg = history
                .iter()
                .map(|x| x.average.unwrap())
                .map(to_not_nan)
                .median()
                .map(|x| *x);
            let high = history
                .iter()
                .map(|x| x.highest.unwrap())
                .map(to_not_nan)
                .median()
                .map(|x| *x);
            let low = history
                .iter()
                .map(|x| x.lowest.unwrap())
                .map(to_not_nan)
                .median()
                .map(|x| *x);

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
                    MarketsRegionHistory {
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

        Ok(data)
    }
    async fn get_item_type_history(
        &self,
        station: StationIdData,
        item_type: i32,
        station_orders: &Mutex<HashMap<i32, Vec<Order>>>,
    ) -> Option<ItemType> {
        let res: Option<ItemType> =
            retry::retry::<_, _, _, Error<GetMarketsRegionIdHistoryError>>(|| async {
                let hist_for_type = (|| async {
                    Ok(market_api::get_markets_region_id_history(
                        self.config,
                        GetMarketsRegionIdHistoryParams {
                            region_id: station.region_id,
                            type_id: item_type,
                            datasource: None,
                            if_none_match: None,
                        },
                    )
                    .await?
                    .entity
                    .unwrap()
                    .into_ok()
                    .unwrap())
                })()
                .await;

                // turn all 404 errors into empty vecs
                let hist_for_type = match hist_for_type {
                    Err(Error::ResponseError(e)) if e.status.as_u16() == 404 => Ok(Vec::new()),
                    e => e,
                }?;

                let mut dummy_empty_vec = Vec::new();
                Ok(ItemType {
                    id: item_type,
                    history: hist_for_type
                        .into_iter()
                        .map(|x| MarketsRegionHistory {
                            average: Some(x.average),
                            date: x.date,
                            highest: Some(x.highest),
                            lowest: Some(x.lowest),
                            order_count: x.order_count,
                            volume: x.volume,
                        })
                        .collect(),
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
        &self,
        item_types: &[i32],
        station: StationIdData,
    ) -> Result<Vec<ItemType>> {
        let station_orders = self.get_orders_station(station).await?;
        let station_orders =
            Mutex::new(station_orders.into_iter().into_group_map_by(|x| x.type_id));

        let hists = stream::iter(item_types)
            .map(|&item_type| {
                let station_orders = &station_orders;
                async move {
                    self.get_item_type_history(station, item_type, station_orders)
                        .await
                }
            })
            .buffer_unordered(BUFFER_UNORDERED);
        Ok(hists
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .flatten()
            .collect::<Vec<_>>())
    }

    pub async fn get_killmail_items_frequency(
        &self,
        killmail_id: i32,
        hash: String,
    ) -> Option<Killmail> {
        let km =
            retry::retry::<_, _, _, Error<GetKillmailsKillmailIdKillmailHashError>>(|| async {
                let res = killmails_api::get_killmails_killmail_id_killmail_hash(
                    self.config,
                    GetKillmailsKillmailIdKillmailHashParams {
                        killmail_hash: hash.clone(),
                        killmail_id,
                        datasource: None,
                        if_none_match: None,
                    },
                )
                .await?
                .entity
                .unwrap()
                .into_ok()
                .unwrap();
                Ok(res)
            })
            .await?;

        let km_items = km
            .victim
            .items
            .unwrap_or_default()
            .into_iter()
            .flat_map(|x| {
                std::iter::once(KillmailItem::from(x.clone())).chain(
                    x.items
                        .map(|x| x.into_iter().map(KillmailItem::from))
                        .unwrap_or_else(|| Vec::new().into_iter().map(KillmailItem::from)),
                )
            })
            .map(|item| {
                let qty = item.quantity_destroyed.unwrap_or(0) + item.quantity_dropped.unwrap_or(0);
                if qty < 1 {
                    log::warn!("Quantity is somehow less than one");
                }
                (item.item_type_id, qty)
            })
            .chain(std::iter::once((km.victim.ship_type_id, 1)));
        Some(Killmail {
            items: km_items
                .group_by(|x| x.0)
                .into_iter()
                .map(|(k, g)| (k, g.map(|x| x.1).sum()))
                .collect(),
            time: NaiveDateTime::parse_from_str(km.killmail_time.as_str(), consts::DATE_TIME_FMT)
                .unwrap(),
        })
    }
}

pub struct Killmail {
    pub items: HashMap<i32, i64>,
    pub time: NaiveDateTime,
}

pub fn to_not_nan(x: f64) -> NotNan<f64> {
    NotNan::new(x).unwrap()
}

#[derive(Debug)]
pub struct KillmailItem {
    /// Flag for the location of the item
    pub flag: i32,
    /// item_type_id integer
    pub item_type_id: i32,
    /// How many of the item were destroyed if any
    pub quantity_destroyed: Option<i64>,
    /// How many of the item were dropped if any
    pub quantity_dropped: Option<i64>,
    /// singleton integer
    pub singleton: i32,
}

impl From<GetKillmailsKillmailIdKillmailHashItem> for KillmailItem {
    fn from(x: GetKillmailsKillmailIdKillmailHashItem) -> Self {
        Self {
            flag: x.flag,
            item_type_id: x.item_type_id,
            quantity_destroyed: x.quantity_destroyed,
            quantity_dropped: x.quantity_dropped,
            singleton: x.singleton,
        }
    }
}

impl From<GetKillmailsKillmailIdKillmailHashItemsItem> for KillmailItem {
    fn from(x: GetKillmailsKillmailIdKillmailHashItemsItem) -> Self {
        Self {
            flag: x.flag,
            item_type_id: x.item_type_id,
            quantity_destroyed: x.quantity_destroyed,
            quantity_dropped: x.quantity_dropped,
            singleton: x.singleton,
        }
    }
}
