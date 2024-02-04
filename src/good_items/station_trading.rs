use std::result::Result::Ok;

use anyhow::anyhow;
use chrono::Duration;
use itertools::Itertools;

use term_table::{row::Row, table_cell::TableCell};

use crate::{
    cached_data::CachedStuff,
    config::CommonConfig,
    good_items::{help::calculate_item_averages, sell_sell::calculate_sell_price},
    helper_ext::HashMapJoin,
    item_type::{ItemOrders, ItemTypeAveraged, MarketData, TypeDescription},
    load_create::{
        create_load_all_types, create_load_item_descriptions, load_or_create_history,
        load_or_create_orders,
    },
    order_ext::OrderIterExt,
    requests::{
        item_history::ItemHistoryEsiService,
        service::{to_not_nan, EsiRequestsService},
    },
    Station,
};

use super::help::{calculate_weighted_price, outbid_price};

pub struct StationTradingService<'a> {
    pub cache: &'a mut CachedStuff,
    pub esi_requests: &'a EsiRequestsService<'a>,
    pub esi_history: &'a ItemHistoryEsiService<'a>,
    pub config: &'a CommonConfig,
}

impl<'a> StationTradingService<'a> {
    pub async fn get_prices_for_items(
        &mut self,
        station_config: Station,
        character_id: i32,
        debug_item_id: Option<i32>,
    ) -> anyhow::Result<StationTradeData> {
        let station = self
            .esi_requests
            .find_region_id_station(&station_config, character_id)
            .await?;

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
            Duration::seconds((self.config.refresh_timeout_hours * 60. * 60.) as i64),
            self.esi_requests,
            station,
        )
        .await?;

        let mut item_order_history = item_history.outer_join(item_orders);

        let mut disable_filters = false;
        if let Some(v) = debug_item_id {
            item_order_history.retain(|&k, _| k == v);
            disable_filters = true;
        }

        let item_data = item_order_history
            .into_iter()
            .map(|(type_id, (history, orders))| {
                let desc = if let Some(desc) = all_type_descriptions
                    .get(&type_id)
                    .and_then(|x| x.as_ref().cloned())
                    .clone()
                {
                    desc
                } else {
                    log::debug!("Couldn't find description for item {type_id} in cache");
                    return Ok(None);
                };

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

                let buy_price = if let Ok(buy_price) =
                    calculate_buy_price(average_history, &market_data, self.config)
                {
                    buy_price
                } else {
                    log::debug!("No calculate_buy_price for item {}", desc.name);
                    return Ok(None);
                };
                log::debug!("Item {} buy price: {}", type_id, buy_price);

                let sell_price =
                    calculate_sell_price(average_history, &market_data, self.config, buy_price);

                let buy_price_with_taxes = buy_price * (1. + station_config.broker_fee);
                let sell_price_with_taxes =
                    sell_price * (1. - station_config.broker_fee - self.config.sales_tax);

                let margin = (sell_price_with_taxes - buy_price_with_taxes) / buy_price_with_taxes;

                let expected_item_volume_per_day = average_history
                    .map(|x| x.volume)
                    .expect("No average history");

                let mut max_buy_vol = (expected_item_volume_per_day
                    * self.config.station_trade.daily_volume_pct)
                    .floor() as i64;

                // limit investment
                if (buy_price_with_taxes * max_buy_vol as f64) > self.config.max_investment_per_item
                {
                    max_buy_vol =
                        (self.config.max_investment_per_item / buy_price_with_taxes).floor() as i64;
                }

                let rough_profit =
                    (sell_price_with_taxes - buy_price_with_taxes) * max_buy_vol as f64;

                let src_volume_on_market = market_data.orders.iter().sell_order_volume();

                Ok(Some(PairCalculatedDataStationTrade {
                    desc,
                    market: market_data,
                    margin,
                    rough_profit,
                    recommend_buy: max_buy_vol,
                    expenses: buy_price_with_taxes * max_buy_vol as f64,
                    gain_per_item: sell_price_with_taxes,
                    buy_price,
                    sell_price,
                    historical_average: average_history,
                    market_volume: src_volume_on_market,
                }))
            })
            .collect::<anyhow::Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .filter(|x| {
                disable_filters
                    || x.margin > self.config.margin_cutoff
                        && x.historical_average.map(|x| x.volume).unwrap_or(0f64)
                            >= self.config.station_trade.min_item_volume
                        && self
                            .config
                            .min_profit
                            .map_or(true, |min_prft| x.rough_profit > min_prft)
            })
            .sorted_by_key(|x| to_not_nan(-x.rough_profit))
            .take(self.config.items_take)
            .collect_vec();

        Ok(StationTradeData { item_data })
    }
}

#[derive(Debug, Clone)]
pub struct PairCalculatedDataStationTrade {
    pub desc: TypeDescription,
    pub market: MarketData,
    pub margin: f64,
    pub rough_profit: f64,
    pub recommend_buy: i64,
    pub expenses: f64,
    pub gain_per_item: f64,
    pub buy_price: f64,
    pub sell_price: f64,
    pub historical_average: Option<ItemTypeAveraged>,
    pub market_volume: i64,
}

fn calculate_buy_price(
    dst_avgs: Option<ItemTypeAveraged>,
    dest_market: &MarketData,
    config: &CommonConfig,
) -> anyhow::Result<f64> {
    let dst_highest_buy_order = if let Some(dst_avgs) = dst_avgs {
        dest_market.orders.iter().get_highest_buy_order_over_volume(
            dst_avgs.volume * config.station_trade.dst_ignore_orders_under_volume_pct,
        )
    } else {
        dest_market.orders.iter().sell_order_min_price()
    };

    let weighted_price = calculate_weighted_price(config, &dest_market.history);
    let item_lowest_historical_avg = if let Ok(weighted_price) = weighted_price {
        weighted_price
    } else {
        return Err(anyhow!("No weighted_price"));
    };

    Ok(match dst_highest_buy_order {
        Some(dst_highest_buy_order) => outbid_price(dst_highest_buy_order, true),
        None => item_lowest_historical_avg,
    })
}

#[derive(Debug, Clone)]
pub struct StationTradeData {
    item_data: Vec<PairCalculatedDataStationTrade>,
}

impl StationTradeData {
    pub fn make_table_station_trade<'b>(&self, name_length: usize) -> Vec<Row<'b>> {
        let rows = std::iter::once(Row::new(vec![
            TableCell::new("id"),
            TableCell::new("itm nm"),
            TableCell::new("buy p"),
            TableCell::new("sell p"),
            TableCell::new("total expenses"),
            TableCell::new("gain per item"),
            TableCell::new("mrgn"),
            TableCell::new("vlm src"),
            TableCell::new("mkt src"),
            TableCell::new("rgh prft"),
            TableCell::new("buy"),
        ]))
        .chain(self.item_data.iter().map(|it| {
            let short_name = it.desc.name[..(name_length.min(it.desc.name.len()))].to_owned();
            Row::new(vec![
                TableCell::new(format!("{}", it.desc.type_id)),
                TableCell::new(short_name),
                TableCell::new(format!("{:.2}", it.buy_price)),
                TableCell::new(format!("{:.2}", it.sell_price)),
                TableCell::new(format!("{:.2}", it.expenses)),
                TableCell::new(format!("{:.2}", it.gain_per_item)),
                TableCell::new(format!("{:.2}", it.margin)),
                TableCell::new(format!(
                    "{:.2}",
                    it.historical_average.map(|x| x.volume).unwrap_or(0f64)
                )),
                TableCell::new(format!("{:.2}", it.market_volume)),
                TableCell::new(format!("{:.2}", it.rough_profit)),
                TableCell::new(format!("{}", it.recommend_buy)),
            ])
        }))
        .collect::<Vec<_>>();
        rows
    }

    pub fn get_buy_order_data(&self) -> impl Iterator<Item = BuyOrderData> + '_ {
        self.item_data.iter().map(|x| BuyOrderData {
            type_id: x.desc.type_id,
            item_price: x.buy_price,
            item_quantity: x.recommend_buy,
        })
    }
}

pub struct BuyOrderData {
    pub type_id: i32,
    pub item_price: f64,
    pub item_quantity: i64,
}
