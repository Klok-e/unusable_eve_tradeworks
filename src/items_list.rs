use std::collections::{HashMap, HashSet};

use chrono::Duration;
use futures::{stream, StreamExt};

use rust_eveonline_esi::apis::configuration::Configuration;

use term_table::row::Row;
use tokio::join;

use crate::{
    cached_data::CachedStuff,
    config::Config,
    consts::{BUFFER_UNORDERED, CACHE_ALL_TYPES, CACHE_ALL_TYPE_DESC, CACHE_ALL_TYPE_PRICES},
    datadump_service::DatadumpService,
    error,
    good_items::{
        sell_buy::{get_good_items_sell_buy, make_table_sell_buy},
        sell_sell::{get_good_items_sell_sell, make_table_sell_sell},
    },
    helper_ext::HashMapJoin,
    item_type::{
        ItemHistory, ItemOrders, MarketData, SystemMarketsItem, SystemMarketsItemData,
        TypeDescription,
    },
    load_create::{
        create_load_all_types, create_load_item_descriptions, create_load_prices,
        load_or_create_history, load_or_create_orders,
    },
    requests::{
        item_history::ItemHistoryEsiService,
        service::{EsiRequestsService, Killmail},
    },
    zkb::{
        killmails::{ItemFrequencies, KillmailService},
        zkb_requests::ZkbRequestsService,
    },
};

pub async fn compute_sell_sell<'a>(
    mut pairs: Vec<SystemMarketsItemData>,
    config: &Config,
    disable_filters: bool,
    simple_list: &mut Vec<SimpleDisplay>,
    name_len: usize,
    cache: CachedStuff,
    force_no_refresh: bool,
    esi_requests: EsiRequestsService<'a>,
    esi_config: &Configuration,
    data_service: DatadumpService,
) -> anyhow::Result<Vec<Row<'a>>> {
    retain_item_groups(config, data_service, &mut pairs)?;

    let kms =
        get_zkb_frequencies(config, cache, force_no_refresh, esi_requests, esi_config).await?;

    let good_items = get_good_items_sell_sell(pairs, config, disable_filters, kms)?;
    *simple_list = good_items
        .items
        .iter()
        .map(|x| SimpleDisplay {
            name: x.item.market.desc.name.clone(),
            recommend_buy: x.recommend_buy,
            sell_price: x.item.dest_min_sell_price,
        })
        .collect();
    Ok(make_table_sell_sell(&good_items, name_len))
}

fn retain_item_groups(
    config: &Config,
    data_service: DatadumpService,
    pairs: &mut Vec<SystemMarketsItemData>,
) -> Result<(), anyhow::Error> {
    let exclude_group_ids = config
        .common
        .sell_sell
        .exclude_groups
        .as_ref()
        .map(|exclude_groups| data_service.get_group_ids_for_groups(exclude_groups))
        .transpose()?;
    log::debug!("Sell sell exclude groups {exclude_group_ids:?}");
    let include_group_ids = config
        .common
        .sell_sell
        .include_groups
        .as_ref()
        .map(|include_groups| data_service.get_group_ids_for_groups(include_groups))
        .transpose()?;
    log::debug!("Sell sell include groups {include_group_ids:?}");
    pairs.retain_mut(|pair| {
        let x = include_group_ids
            .as_ref()
            .map(|include_group_ids| {
                pair.desc
                    .market_group_id
                    .map(|market_group_id| include_group_ids.contains(&market_group_id))
                    .unwrap_or(false)
            })
            .unwrap_or(true)
            && exclude_group_ids
                .as_ref()
                .map(|exclude_group_ids| {
                    pair.desc
                        .market_group_id
                        .map(|market_group_id| !exclude_group_ids.contains(&market_group_id))
                        .unwrap_or(true)
                })
                .unwrap_or(true);
        log::debug!(
            "Item {}, market group id {:?}, excluded: {}",
            pair.desc.name,
            pair.desc.market_group_id,
            !x
        );
        x
    });
    Ok(())
}

async fn get_zkb_frequencies(
    config: &Config,
    mut cache: CachedStuff,
    force_no_refresh: bool,
    esi_requests: EsiRequestsService<'_>,
    esi_config: &Configuration,
) -> Result<ItemFrequencies, anyhow::Error> {
    let cache_zkb_entity = format!(
        "zkb_losses.{}.{}.rmp",
        config.common.zkill_entity.tp.zkill_filter_string(),
        config.common.zkill_entity.id
    );

    let esi_requests = &esi_requests;
    let client = &esi_config.client;
    let config = &config;

    let zkb = ZkbRequestsService::new(client);
    let km_service = KillmailService::new(&zkb, esi_requests);

    let killmails = cache
        .load_or_create_async(
            cache_zkb_entity,
            vec![],
            if force_no_refresh {
                None
            } else {
                Some(Duration::hours(24))
            },
            |previous: Option<Vec<Killmail>>| {
                let km_service = &km_service;

                async move {
                    let mut kills = Vec::new();
                    for page in 1..=config.common.sell_sell.sell_sell_zkb.zkb_download_pages {
                        let mut kills_page = km_service
                            .get_killmails(&config.common.zkill_entity, page)
                            .await?;

                        let kills_page_ids_hashset =
                            kills_page.iter().map(|x| x.km_id).collect::<HashSet<_>>();
                        match previous {
                            Some(ref previous)
                                if previous
                                    .iter()
                                    .any(|x| kills_page_ids_hashset.contains(&x.km_id)) =>
                            {
                                log::info!("Killmail already in cache, download stopped");
                                break;
                            }
                            _ => (),
                        }

                        kills.append(&mut kills_page);
                    }

                    kills.append(&mut previous.unwrap_or_default());

                    kills.sort_unstable_by_key(|x| x.km_id);
                    kills.dedup_by_key(|x| x.km_id);

                    Ok(kills)
                }
            },
        )
        .await?;

    let frequencies = km_service.get_item_frequencies(killmails);
    Ok(frequencies)
}

pub fn compute_sell_buy<'a>(
    pairs: Vec<SystemMarketsItemData>,
    config: &Config,
    disable_filters: bool,
    simple_list: &mut Vec<SimpleDisplay>,
    name_len: usize,
) -> anyhow::Result<Vec<Row<'a>>> {
    let good_items = get_good_items_sell_buy(pairs, config, disable_filters)?;
    *simple_list = good_items
        .items
        .iter()
        .map(|x| SimpleDisplay {
            name: x.item.market.desc.name.clone(),
            recommend_buy: x.recommend_buy,
            sell_price: x.item.dest_min_sell_price,
        })
        .collect();
    Ok(make_table_sell_buy(&good_items, name_len))
}

pub async fn compute_pairs<'a>(
    config: &Config,
    esi_requests: &EsiRequestsService<'a>,
    esi_history: &ItemHistoryEsiService<'a>,
    character_id: i32,
    cache: &mut CachedStuff,
    data_service: &DatadumpService,
) -> anyhow::Result<Vec<SystemMarketsItemData>> {
    let source_region = esi_requests
        .find_region_id_station(config.route.source.clone(), character_id)
        .await
        .unwrap();
    let dest_region = esi_requests
        .find_region_id_station(config.route.destination.clone(), character_id)
        .await
        .unwrap();
    let all_types = create_load_all_types(cache, esi_requests, source_region, dest_region).await?;
    let all_type_descriptions =
        create_load_item_descriptions(cache, &all_types, esi_requests).await?;
    let all_type_prices = create_load_prices(cache, esi_requests).await?;

    let source_item_history = load_or_create_history(
        cache,
        source_region,
        Duration::hours(config.common.item_history_timeout_hours),
        esi_history,
        &all_types,
    )
    .await?;
    let dest_item_history = load_or_create_history(
        cache,
        dest_region,
        Duration::hours(config.common.item_history_timeout_hours),
        esi_history,
        &all_types,
    )
    .await?;

    let source_item_orders = load_or_create_orders(
        cache,
        Duration::seconds((config.common.refresh_timeout_hours * 60. * 60.) as i64),
        esi_requests,
        source_region,
    )
    .await?;

    let dest_item_orders = load_or_create_orders(
        cache,
        Duration::seconds((config.common.refresh_timeout_hours * 60. * 60.) as i64),
        esi_requests,
        dest_region,
    )
    .await?;

    let source_items = source_item_orders.outer_join(source_item_history);
    let dest_items = dest_item_orders.outer_join(dest_item_history);
    let pairs = source_items
        .inner_join(dest_items)
        .into_iter()
        .flat_map(|(k, (source, dest))| {
            let source = match make_item_orders_history_empty_if_none(source) {
                Some(source) => source,
                None => {
                    return None;
                }
            };
            let dest = match make_item_orders_history_empty_if_none(dest) {
                Some(dest) => dest,
                None => {
                    return None;
                }
            };
            Some(SystemMarketsItem {
                id: k,
                source: MarketData::new(source.0, source.1),
                destination: MarketData::new(dest.0, dest.1),
            })
        });
    let group_ids = config
        .common
        .include_groups
        .as_ref()
        .map(|x| -> anyhow::Result<Vec<i32>> {
            let groups = x
                .iter()
                .map(|name| {
                    let children = data_service.get_all_group_id_with_root_name(name.as_str())?;

                    Ok(children)
                })
                .collect::<anyhow::Result<Vec<_>>>()?
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();
            Ok(groups)
        })
        .transpose()?;
    Ok(pairs
        .filter_map(|it| {
            let req_res = all_type_descriptions
                .get(&it.id)
                .unwrap_or_else(|| {
                    let it_id = it.id;
                    log::warn!("Couldn't find item {it_id} in all_type_descriptions map");
                    &None
                })
                .clone();
            let req_res = match req_res {
                Some(x) => x,
                None => return None,
            };

            // include only specific groups
            if let Some(ids) = &group_ids {
                if !req_res
                    .market_group_id
                    .map(|x| ids.contains(&x))
                    .unwrap_or(false)
                {
                    return None;
                }
            }

            Some(SystemMarketsItemData {
                desc: req_res,
                source: it.source,
                destination: it.destination,
                adjusted_price: all_type_prices.get(&it.id).copied(),
            })
        })
        .collect::<Vec<_>>())
}

pub struct SimpleDisplay {
    pub name: String,
    pub recommend_buy: i64,
    pub sell_price: f64,
}

fn make_item_orders_history_empty_if_none(
    items: (Option<ItemOrders>, Option<ItemHistory>),
) -> Option<(ItemOrders, ItemHistory)> {
    match items {
        (Some(order), Some(history)) => Some((order, history)),
        (Some(order), None) => {
            let new_history = ItemHistory {
                id: order.id,
                ..Default::default()
            };
            Some((order, new_history))
        }
        (None, Some(history)) => {
            let new_orders = ItemOrders {
                id: history.id,
                ..Default::default()
            };
            Some((new_orders, history))
        }
        _ => None,
    }
}
