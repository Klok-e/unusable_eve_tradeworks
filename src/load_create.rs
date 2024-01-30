use std::collections::HashMap;

use chrono::Duration;

use crate::{
    cached_data::CachedStuff,
    consts::CACHE_ALL_TYPES,
    item_type::{ItemHistory, ItemOrders},
    requests::{item_history::ItemHistoryEsiService, service::EsiRequestsService},
    StationIdData,
};

pub async fn load_or_create_history(
    cache: &mut CachedStuff,
    region: StationIdData,
    duration: Duration,
    esi_history: &ItemHistoryEsiService<'_>,
    all_types: &Vec<i32>,
) -> anyhow::Result<HashMap<i32, ItemHistory>> {
    let item_history = cache
        .load_or_create_async(
            format!("{}-history.rmp", region.region_id),
            vec![CACHE_ALL_TYPES],
            Some(duration),
            |_| async {
                Ok(esi_history
                    .all_item_history(all_types, region.region_id)
                    .await?)
            },
        )
        .await?
        .into_iter()
        .map(|x| (x.id, x))
        .collect::<HashMap<_, _>>();
    Ok(item_history)
}

pub async fn load_or_create_orders(
    cache: &mut CachedStuff,
    duration: Duration,
    esi_requests: &EsiRequestsService<'_>,
    source_region: StationIdData,
) -> anyhow::Result<HashMap<i32, ItemOrders>> {
    let source_item_orders = cache
        .load_or_create_async(
            format!(
                "{}-{}-orders.rmp",
                source_region.station_id.id, source_region.region_id
            ),
            vec![CACHE_ALL_TYPES],
            Some(duration),
            |_| async { Ok(esi_requests.all_item_orders(source_region).await?) },
        )
        .await?
        .into_iter()
        .map(|x| (x.id, x))
        .collect::<HashMap<_, _>>();
    Ok(source_item_orders)
}
