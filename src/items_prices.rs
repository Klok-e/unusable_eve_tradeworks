use std::collections::HashMap;

use anyhow::{anyhow, Ok};
use chrono::Duration;
use itertools::Itertools;
use rust_eveonline_esi::models::GetCharactersCharacterIdWalletTransactions200Ok;

use crate::{
    cached_data::CachedStuff,
    config::Config,
    consts::CACHE_ALL_TYPE_DESC,
    item_type::TypeDescription,
    load_create::{load_or_create_history, load_or_create_orders},
    requests::{
        item_history::ItemHistoryEsiService, service::EsiRequestsService,
        transactions::WalletEsiService,
    },
    Station,
};

pub struct ItemsPricesService<'a> {
    pub item_history: &'a WalletEsiService<'a>,
    pub cache: &'a mut CachedStuff,
    pub esi_requests: &'a EsiRequestsService<'a>,
    pub esi_history: &'a ItemHistoryEsiService<'a>,
    pub config: &'a Config,
}

impl<'a> ItemsPricesService<'a> {
    pub async fn get_prices_for_items(
        &mut self,
        character_id: i32,
        items: Vec<ItemInput>,
        station: Station,
    ) -> anyhow::Result<Vec<ItemSellPrice>> {
        let station = self
            .esi_requests
            .find_region_id_station(station, character_id)
            .await
            .unwrap();

        let all_type_descriptions: HashMap<i32, Option<TypeDescription>> = self
            .cache
            .load_or_create_async(CACHE_ALL_TYPE_DESC, vec![], None, |previous| async {
                Ok(previous.ok_or(anyhow!(
                    "Previous value for all type descriptions wasn't found"
                ))?)
            })
            .await?;

        let item_history = load_or_create_history(
            self.cache,
            station,
            Duration::hours(self.config.common.item_history_timeout_hours),
            self.esi_history,
            &all_type_descriptions.iter().map(|x| *x.0).collect_vec(),
        )
        .await?;

        let item_orders = load_or_create_orders(
            self.cache,
            Duration::hours(self.config.common.item_history_timeout_hours),
            self.esi_requests,
            station,
        )
        .await?;

        let transactions = self.download_or_load_transactions(character_id).await?;

        Ok(vec![])
    }

    pub async fn download_or_load_transactions(
        &mut self,
        character_id: i32,
    ) -> anyhow::Result<Vec<GetCharactersCharacterIdWalletTransactions200Ok>> {
        let transactions = self
            .cache
            .load_or_create_async(
                format!("wallet-history-{character_id}.rmp"),
                vec![],
                Some(Duration::seconds(10 * 60)),
                |mut previous| {
                    let item_hist = &mut self.item_history;
                    async move {
                        let mut new_transactions =
                            item_hist.get_transactions_history(character_id).await?;

                        if let Some(ref mut previous) = previous {
                            new_transactions.append(previous)
                        }

                        new_transactions.sort_unstable_by_key(|x| x.transaction_id);
                        new_transactions.dedup_by_key(|x| x.transaction_id);

                        Ok(new_transactions)
                    }
                },
            )
            .await?;

        Ok(transactions)
    }
}

pub struct ItemInput {
    pub name: String,
    pub amount: i32,
}

pub struct ItemSellPrice {
    pub price: f64,
    pub item_id: i32,
}
