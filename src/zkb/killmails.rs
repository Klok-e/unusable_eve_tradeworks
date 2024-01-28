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

    pub fn get_item_frequencies(&self, killmails: Vec<Killmail>) -> ItemFrequencies {
        let most_recent_time = killmails.iter().map(|x| x.time).max().unwrap();
        let oldest_time = killmails.iter().map(|x| x.time).min().unwrap();
        let period_seconds = (most_recent_time - oldest_time).num_seconds();

        log::info!(
            "{} killmails for {} days...",
            killmails.len(),
            period_seconds as f64 / 60. / 60. / 24.
        );

        ItemFrequencies {
            items: killmails.iter().fold(HashMap::new(), |mut acc, x| {
                for (k, v) in x.items.iter() {
                    *acc.entry(*k).or_insert(0) += v;
                }
                acc
            }),
            period_seconds,
        }
    }

    pub async fn get_killmails(
        &self,
        entity: &ZkillEntity,
        page: u32,
    ) -> Result<Vec<Killmail>, anyhow::Error> {
        let kms = self.zkb.get_kb_page(entity, page).await?;
        let frequencies = kms.into_iter().map(|km| {
            self.esi
                .get_killmail_items_frequency(km.killmail_id, km.zkb.hash)
        });
        let km_freqs: Vec<Killmail> = stream::iter(frequencies)
            .buffer_unordered(BUFFER_UNORDERED)
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<crate::requests::error::Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect();
        Ok(km_freqs)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ItemFrequencies {
    pub items: HashMap<i32, i64>,
    pub period_seconds: i64,
}
