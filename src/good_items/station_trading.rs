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
    load_create::{
        create_load_all_types, create_load_item_descriptions, load_or_create_history,
        load_or_create_orders,
    },
    requests::{
        item_history::ItemHistoryEsiService, service::EsiRequestsService,
        transactions::WalletEsiService,
    },
    Station, StationIdData,
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
        station: StationIdData,
    ) -> anyhow::Result<Vec<ItemSellPrice>> {
        let all_types =
            create_load_all_types(self.cache, self.esi_requests, station, station).await?;
        let all_type_descriptions =
            create_load_item_descriptions(self.cache, &all_types, self.esi_requests).await?;

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

        let item_order_history = item_history.outer_join(item_orders);

        let item_prices = item_order_history
            .into_iter()
            .map(|(type_id, (history, orders))| {
                let market_data = MarketData::new(
                    orders
                        .as_ref()
                        .unwrap_or(&ItemOrders {
                            id: type_id,
                            orders: Vec::new(),
                        })
                        .clone(),
                    history
                        .as_ref()
                        .ok_or_else(|| anyhow!("History not found for item {}", type_id))?
                        .clone(),
                );
                let average_history = calculate_item_averages(self.config, &market_data.history);

                let buy_price = 0.;

                log::debug!("Item {} buy price: {}", type_id, buy_price);

                let sell_price =
                    calculate_sell_price(average_history, &market_data, self.config, buy_price);

                Ok(ItemSellPrice {
                    price: sell_price,
                    item_id: type_id,
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        Ok(item_prices)
    }
}

pub struct ItemSellPrice {
    pub price: f64,
    pub item_id: i32,
}
