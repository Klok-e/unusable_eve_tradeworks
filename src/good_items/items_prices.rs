use std::collections::HashMap;
use std::result::Result::Ok;

use anyhow::anyhow;
use chrono::Duration;
use itertools::Itertools;
use rust_eveonline_esi::models::GetCharactersCharacterIdWalletTransactions200Ok;

use crate::{
    cached_data::CachedStuff,
    config::CommonConfig,
    consts::CACHE_ALL_TYPE_DESC,
    good_items::{help::calculate_item_averages, sell_sell::calculate_sell_price},
    helper_ext::HashMapJoin,
    item_type::{ItemOrders, MarketData, TypeDescription},
    load_create::{load_or_create_history, load_or_create_orders},
    requests::{
        item_history::ItemHistoryEsiService, service::EsiRequestsService,
        transactions::WalletEsiService,
    },
    Station,
};

pub struct ItemsPricesService<'a> {
    pub wallet_esi_service: &'a WalletEsiService<'a>,
    pub cache: &'a mut CachedStuff,
    pub esi_requests: &'a EsiRequestsService<'a>,
    pub esi_history: &'a ItemHistoryEsiService<'a>,
    pub config: &'a CommonConfig,
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
                previous.ok_or(anyhow!(
                    "Previous value for all type descriptions wasn't found"
                ))
            })
            .await?;

        let item_history = load_or_create_history(
            self.cache,
            station,
            Duration::hours(self.config.item_history_timeout_hours),
            self.esi_history,
            &all_type_descriptions.iter().map(|x| *x.0).collect_vec(),
        )
        .await?;

        let item_orders = load_or_create_orders(
            self.cache,
            Duration::hours(self.config.item_history_timeout_hours),
            self.esi_requests,
            station,
        )
        .await?;

        let name_to_item_desc = all_type_descriptions
            .into_iter()
            .filter_map(|(_, desc)| {
                let desc = desc?;
                Some((desc.name.clone(), desc))
            })
            .collect::<HashMap<_, _>>();

        let item_order_history = item_history.outer_join(item_orders);

        let transactions = self.download_or_load_transactions(character_id).await?;

        let item_prices = items
            .into_iter()
            .map(|item_input| {
                let item = name_to_item_desc
                    .get(&item_input.name)
                    .ok_or_else(|| anyhow!("Item {} not found in cache", item_input.name))?;

                let (history, orders) = item_order_history.get(&item.type_id).ok_or_else(|| {
                    anyhow!(
                        "Item orders or history for {} not found in cache",
                        item_input.name
                    )
                })?;

                let market_data = MarketData::new(
                    orders
                        .as_ref()
                        .unwrap_or(&ItemOrders {
                            id: item.type_id,
                            orders: Vec::new(),
                        })
                        .clone(),
                    history
                        .as_ref()
                        .ok_or_else(|| anyhow!("History not found for item {}", item_input.name))?
                        .clone(),
                );
                let average_history = calculate_item_averages(self.config, &market_data.history);

                let buy_price = transactions
                    .get_bought_price_for_item(item.type_id, item_input.amount)
                    .unwrap_or(0.);

                log::debug!("Item {} buy price: {}", item.name, buy_price);

                let sell_price =
                    calculate_sell_price(average_history, &market_data, self.config, buy_price);

                Ok(ItemSellPrice {
                    price: sell_price,
                    item_id: item.type_id,
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        Ok(item_prices)
    }

    pub async fn download_or_load_transactions(
        &mut self,
        character_id: i32,
    ) -> anyhow::Result<CharacterTransactions> {
        let transactions = self
            .cache
            .load_or_create_async(
                format!("wallet-history-{character_id}.rmp"),
                vec![],
                Some(Duration::seconds(10 * 60)),
                |mut previous| {
                    let item_hist = &mut self.wallet_esi_service;
                    async move {
                        let mut new_transactions =
                            item_hist.get_transactions_history(character_id).await?;

                        if let Some(ref mut previous) = previous {
                            new_transactions.append(previous)
                        }

                        new_transactions.sort_unstable_by_key(|x| -x.transaction_id);
                        new_transactions.dedup_by_key(|x| x.transaction_id);

                        Ok(new_transactions)
                    }
                },
            )
            .await?;

        Ok(CharacterTransactions { transactions })
    }
}

pub struct CharacterTransactions {
    transactions: Vec<GetCharactersCharacterIdWalletTransactions200Ok>,
}

impl CharacterTransactions {
    pub fn get_bought_price_for_item(&self, type_id: i32, amount: i32) -> Option<f64> {
        let mut total_price = 0.0;
        let mut total_quantity = 0;

        for transaction in self
            .transactions
            .iter()
            .filter(|x| x.type_id == type_id && x.is_buy)
        {
            total_price += transaction.unit_price * transaction.quantity as f64;
            total_quantity += transaction.quantity;

            if total_quantity >= amount {
                break;
            }
        }

        if total_quantity >= amount {
            Some(total_price / amount as f64)
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct ItemInput {
    pub name: String,
    pub amount: i32,
}

pub struct ItemSellPrice {
    pub price: f64,
    pub item_id: i32,
}
