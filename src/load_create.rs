use std::collections::HashMap;

use chrono::Duration;
use futures::{stream, StreamExt};
use tokio::join;

use crate::{
    cached_data::CachedStuff,
    consts::{BUFFER_UNORDERED, CACHE_ALL_TYPES, CACHE_ALL_TYPE_DESC, CACHE_ALL_TYPE_PRICES},
    error,
    item_type::{ItemHistory, ItemOrders, TypeDescription},
    requests::{item_history::ItemHistoryEsiService, service::EsiRequestsService},
    StationIdData,
};

pub async fn load_or_create_history(
    cache: &mut CachedStuff,
    region: StationIdData,
    duration: Duration,
    esi_history: &ItemHistoryEsiService<'_>,
    all_types: &[i32],
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

pub async fn create_load_all_types(
    cache: &mut CachedStuff,
    esi_requests: &EsiRequestsService<'_>,
    source_region: crate::StationIdData,
    dest_region: crate::StationIdData,
) -> Result<Vec<i32>, anyhow::Error> {
    let all_types = cache
        .load_or_create_json_async(
            CACHE_ALL_TYPES,
            vec![],
            Some(Duration::days(7)),
            |_| async {
                let all_types_src = esi_requests.get_all_item_types(source_region.region_id);
                let all_types_dest = esi_requests.get_all_item_types(dest_region.region_id);
                let (all_types_src, all_types_dest) = join!(all_types_src, all_types_dest);
                let (mut all_types, all_types_dest) = (all_types_src?, all_types_dest?);

                all_types.extend(all_types_dest);
                all_types.sort_unstable();
                all_types.dedup();
                Ok(all_types)
            },
        )
        .await?;
    Ok(all_types)
}

pub async fn create_load_item_descriptions(
    cache: &mut CachedStuff,
    all_types: &Vec<i32>,
    esi_requests: &EsiRequestsService<'_>,
) -> Result<HashMap<i32, Option<TypeDescription>>, anyhow::Error> {
    let all_type_descriptions: HashMap<i32, Option<TypeDescription>> = cache
        .load_or_create_async(
            CACHE_ALL_TYPE_DESC,
            vec![CACHE_ALL_TYPES],
            Some(Duration::days(7)),
            |_| async {
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
    Ok(all_type_descriptions)
}

pub async fn create_load_prices(
    cache: &mut CachedStuff,
    esi_requests: &EsiRequestsService<'_>,
) -> Result<HashMap<i32, f64>, anyhow::Error> {
    let all_type_prices: HashMap<i32, f64> = cache
        .load_or_create_async(
            CACHE_ALL_TYPE_PRICES,
            vec![CACHE_ALL_TYPES],
            Some(Duration::days(1)),
            |_| async {
                let prices = esi_requests.get_ajusted_prices().await?.unwrap();
                Ok(prices
                    .into_iter()
                    .map(|x| (x.type_id, x.adjusted_price.unwrap()))
                    .collect())
            },
        )
        .await?;
    Ok(all_type_prices)
}
