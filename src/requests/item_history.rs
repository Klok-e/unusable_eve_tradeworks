use std::collections::HashMap;

use rust_eveonline_esi::apis::configuration::Configuration;

use crate::{consts::BUFFER_UNORDERED_SMALL, requests::paged_all::OnlyOk, requests::retry};
use crate::{
    consts::DATE_FMT,
    item_type::{ItemHistory, MarketsRegionHistory},
    requests::retry::Retry,
};
use chrono::{Duration, NaiveDate, Utc};
use reqwest::StatusCode;

use super::{
    error::{EsiApiError, Result},
    service::to_not_nan,
};

use crate::stat::MedianStat;

use futures::{stream, StreamExt};
use itertools::Itertools;

use rust_eveonline_esi::apis::market_api::{self, GetMarketsRegionIdHistoryParams};

pub struct ItemHistoryEsiService<'a> {
    pub config: &'a Configuration,
}
impl<'a> ItemHistoryEsiService<'a> {
    pub fn new(config: &'a Configuration) -> Self {
        Self { config }
    }

    pub async fn all_item_history(
        &self,
        item_types: &[i32],
        region_id: i32,
    ) -> Result<Vec<ItemHistory>> {
        let mut data = self.download_item_data(item_types, region_id).await?;

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
        region_id: i32,
        item_type: i32,
    ) -> Result<Option<ItemHistory>> {
        let res: Option<ItemHistory> = retry::retry_smart(|| async {
            let hist_for_type: Result<_> = async {
                Ok(market_api::get_markets_region_id_history(
                    self.config,
                    GetMarketsRegionIdHistoryParams {
                        region_id,
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
            }
            .await;

            // turn all 404 errors into empty vecs
            let hist_for_type = match hist_for_type {
                Ok(ok) => ok,
                Err(
                    api_err @ EsiApiError {
                        status: StatusCode::NOT_FOUND | StatusCode::BAD_REQUEST,
                        ..
                    },
                ) => {
                    log::debug!("Making empty hist_for_type: {api_err:?}");
                    Vec::new()
                }
                Err(e) => {
                    log::debug!(
                        "Region id: {region_id}; Item type: {item_type} Returning error: {e:?}"
                    );
                    return Err(e);
                }
            };

            let item = ItemHistory {
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
            };
            Ok(Retry::Success(item))
        })
        .await?;
        Ok(res)
    }

    async fn download_item_data(
        &self,
        item_types: &[i32],
        region_id: i32,
    ) -> Result<Vec<ItemHistory>> {
        let hists = stream::iter(item_types)
            .map(|&item_type| self.get_item_type_history(region_id, item_type))
            .buffer_unordered(BUFFER_UNORDERED_SMALL);
        Ok(hists
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>())
    }
}
