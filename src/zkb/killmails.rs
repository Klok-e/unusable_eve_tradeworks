use std::collections::HashMap;

use futures::{stream, StreamExt};
use serde::{Deserialize, Serialize};

use crate::{
    consts::BUFFER_UNORDERED,
    requests::service::{EsiRequestsService, Killmail},
};

use super::zkb_requests::{ZkbRequestsService, ZkillEntity};

pub struct KillmailService<'a> {
    zkb: &'a ZkbRequestsService<'a>,
    esi: &'a EsiRequestsService<'a>,
}

impl<'a> KillmailService<'a> {
    pub fn new(zkb: &'a ZkbRequestsService<'a>, esi: &'a EsiRequestsService<'a>) -> Self {
        Self { zkb, esi }
    }

    pub async fn get_kill_item_frequencies(
        &self,
        entity: &ZkillEntity,
        pages: u32,
    ) -> crate::requests::error::Result<ItemFrequencies> {
        let kms = self.zkb.get_killmails(entity, pages).await.unwrap();
        let frequencies = kms.into_iter().map(|km| {
            self.esi
                .get_killmail_items_frequency(km.killmail_id, km.zkb.hash)
        });

        let km_freqs: Vec<Killmail> = stream::iter(frequencies)
            .map(|x| async { x.await })
            .buffer_unordered(BUFFER_UNORDERED)
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<crate::requests::error::Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect();

        let most_recent_time = km_freqs.iter().map(|x| x.time).max().unwrap();
        let oldest_time = km_freqs.iter().map(|x| x.time).min().unwrap();
        Ok(ItemFrequencies {
            items: km_freqs.iter().fold(HashMap::new(), |mut acc, x| {
                for (k, v) in x.items.iter() {
                    *acc.entry(*k).or_insert(0) += v;
                }
                acc
            }),
            period_seconds: (most_recent_time - oldest_time).num_seconds(),
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ItemFrequencies {
    pub items: HashMap<i32, i64>,
    pub period_seconds: i64,
}
