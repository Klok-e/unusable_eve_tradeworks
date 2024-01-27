use std::collections::HashMap;

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
    requests::{item_history::ItemHistoryEsiService, service::EsiRequestsService},
    zkb::{
        killmails::{ItemFrequencies, KillmailService},
        zkb_requests::ZkbRequestsService,
    },
};

pub async fn compute_sell_sell<'a>(
    pairs: Vec<SystemMarketsItemData>,
    config: &Config,
    disable_filters: bool,
    simple_list: &mut Vec<SimpleDisplay>,
    name_len: usize,
    cache: CachedStuff,
    force_no_refresh: bool,
    esi_requests: EsiRequestsService<'a>,
    esi_config: &Configuration,
) -> anyhow::Result<Vec<Row<'a>>> {
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

async fn get_zkb_frequencies(
    config: &Config,
    mut cache: CachedStuff,
    force_no_refresh: bool,
    esi_requests: EsiRequestsService<'_>,
    esi_config: &Configuration,
) -> Result<ItemFrequencies, anyhow::Error> {
    let cache_zkb_entity = format!(
        "cache/zkb_losses.{}.{}.rmp",
        config.common.zkill_entity.tp.zkill_filter_string(),
        config.common.zkill_entity.id
    );
    let kms = cache
        .load_or_create_async(
            cache_zkb_entity,
            vec![],
            if force_no_refresh {
                None
            } else {
                Some(Duration::hours(24))
            },
            || {
                let esi_requests = &esi_requests;
                let client = &esi_config.client;
                let config = &config;
                async move {
                    let zkb = ZkbRequestsService::new(client);
                    let km_service = KillmailService::new(&zkb, esi_requests);
                    Ok(km_service
                        .get_kill_item_frequencies(
                            &config.common.zkill_entity,
                            config.common.sell_sell.sell_sell_zkb.zkb_download_pages,
                        )
                        .await?)
                }
            },
        )
        .await?;
    Ok(kms)
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
    let all_types = cache
        .load_or_create_json_async(CACHE_ALL_TYPES, vec![], Some(Duration::days(7)), || async {
            let all_types_src = esi_requests.get_all_item_types(source_region.region_id);
            let all_types_dest = esi_requests.get_all_item_types(dest_region.region_id);
            let (all_types_src, all_types_dest) = join!(all_types_src, all_types_dest);
            let (mut all_types, all_types_dest) = (all_types_src?, all_types_dest?);

            all_types.extend(all_types_dest);
            all_types.sort_unstable();
            all_types.dedup();
            Ok(all_types)
        })
        .await?;
    let all_type_descriptions: HashMap<i32, Option<TypeDescription>> = cache
        .load_or_create_async(
            CACHE_ALL_TYPE_DESC,
            vec![CACHE_ALL_TYPES],
            Some(Duration::days(7)),
            || async {
                let res = stream::iter(all_types.clone())
                    .map(|id| {
                        let esi_requests = &esi_requests;
                        async move {
                            let req_res = esi_requests.get_item_description(id).await?;

                            Ok((id, req_res.map(|x| x.into())))
                        }
                    })
                    .buffer_unordered(BUFFER_UNORDERED)
                    .collect::<Vec<error::Result<_>>>()
                    .await
                    .into_iter()
                    .collect::<error::Result<Vec<_>>>()?
                    .into_iter()
                    .collect();

                Ok(res)
            },
        )
        .await?;
    let all_type_prices: HashMap<i32, f64> = cache
        .load_or_create_async(
            CACHE_ALL_TYPE_PRICES,
            vec![CACHE_ALL_TYPES],
            Some(Duration::days(1)),
            || async {
                let prices = esi_requests.get_ajusted_prices().await?.unwrap();
                Ok(prices
                    .into_iter()
                    .map(|x| (x.type_id, x.adjusted_price.unwrap()))
                    .collect())
            },
        )
        .await?;

    let source_item_history = cache
        .load_or_create_async(
            format!("cache/{}-history.rmp", source_region.region_id),
            vec![CACHE_ALL_TYPES],
            Some(Duration::hours(config.common.item_history_timeout_hours)),
            || async {
                Ok(esi_history
                    .all_item_history(&all_types, source_region.region_id)
                    .await?)
            },
        )
        .await?
        .into_iter()
        .map(|x| (x.id, x))
        .collect::<HashMap<_, _>>();
    let dest_item_history = cache
        .load_or_create_async(
            format!("cache/{}-history.rmp", dest_region.region_id),
            vec![CACHE_ALL_TYPES],
            Some(Duration::hours(config.common.item_history_timeout_hours)),
            || async {
                Ok(esi_history
                    .all_item_history(&all_types, dest_region.region_id)
                    .await?)
            },
        )
        .await?
        .into_iter()
        .map(|x| (x.id, x))
        .collect::<HashMap<_, _>>();

    let source_item_orders = cache
        .load_or_create_async(
            format!("cache/{}-orders.rmp", config.route.source.name),
            vec![CACHE_ALL_TYPES],
            Some(Duration::seconds(
                (config.common.refresh_timeout_hours * 60. * 60.) as i64,
            )),
            || async { Ok(esi_requests.all_item_orders(source_region).await?) },
        )
        .await?
        .into_iter()
        .map(|x| (x.id, x))
        .collect::<HashMap<_, _>>();
    let dest_item_orders = cache
        .load_or_create_async(
            format!("cache/{}-orders.rmp", config.route.destination.name),
            vec![CACHE_ALL_TYPES],
            Some(Duration::seconds(
                (config.common.refresh_timeout_hours * 60. * 60.) as i64,
            )),
            || async { Ok(esi_requests.all_item_orders(dest_region).await?) },
        )
        .await?
        .into_iter()
        .map(|x| (x.id, x))
        .collect::<HashMap<_, _>>();

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
        .map(|x| {
            let groups = x
                .iter()
                .map(|name| {
                    let children = data_service.get_all_group_id_with_root_name(name.as_str())?;

                    Ok(children)
                })
                .collect::<error::Result<Vec<_>>>()?
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();
            error::Result::Ok(groups)
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
