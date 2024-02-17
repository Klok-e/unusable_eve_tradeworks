use std::collections::HashMap;

use anyhow::anyhow;
use chrono::Duration;
use itertools::Itertools;

use term_table::{row::Row, table_cell::TableCell};

use crate::{
    cached_data::CachedStuff,
    config::CommonConfig,
    datadump_service::DatadumpService,
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

use super::help::outbid_price;

pub struct StationTradingService<'a> {
    pub cache: &'a mut CachedStuff,
    pub datadump: &'a DatadumpService,
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

        retain_item_groups(
            self.config,
            self.datadump,
            &mut item_order_history,
            &all_type_descriptions,
        )?;

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
                    match history.as_ref() {
                        Some(v) => v,
                        None => {
                            log::debug!("History not found for item {}", type_id);
                            return Ok(None);
                        }
                    }
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

                let Some(expected_item_volume_per_day) = average_history.map(|x| x.volume) else {
                    log::debug!("No average_history for item {}", desc.name);
                    return Ok(None);
                };

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

fn retain_item_groups<T>(
    config: &CommonConfig,
    data_service: &DatadumpService,
    items: &mut HashMap<i32, T>,
    descriptions: &HashMap<i32, Option<TypeDescription>>,
) -> Result<(), anyhow::Error> {
    let exclude_group_ids = config
        .station_trade
        .exclude_groups
        .as_ref()
        .map(|exclude_groups| data_service.get_group_ids_for_groups(exclude_groups))
        .transpose()?;
    log::debug!("Station trade exclude groups {exclude_group_ids:?}");
    let include_group_ids = config
        .station_trade
        .include_groups
        .as_ref()
        .map(|include_groups| data_service.get_group_ids_for_groups(include_groups))
        .transpose()?;
    log::debug!("Station trade include groups {include_group_ids:?}");
    items.retain(|k, _| {
        let desc = if let Some(desc) = descriptions
            .get(k)
            .and_then(|x| x.as_ref().cloned())
            .clone()
        {
            desc
        } else {
            log::debug!("Couldn't find description for item {k} in cache");
            return true;
        };

        let x = include_group_ids
            .as_ref()
            .map(|include_group_ids| {
                desc.market_group_id
                    .map(|market_group_id| include_group_ids.contains(&market_group_id))
                    .unwrap_or(false)
            })
            .unwrap_or(true)
            && exclude_group_ids
                .as_ref()
                .map(|exclude_group_ids| {
                    desc.market_group_id
                        .map(|market_group_id| !exclude_group_ids.contains(&market_group_id))
                        .unwrap_or(true)
                })
                .unwrap_or(true);
        log::debug!(
            "Item {}, market group id {:?}, excluded: {}",
            desc.name,
            desc.market_group_id,
            !x
        );
        x
    });
    Ok(())
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

    Ok(match (dst_highest_buy_order, dst_avgs) {
        (Some(dst_highest_buy_order), Some(dst_avgs))
            if dst_avgs.low_average > dst_highest_buy_order
                && ((dst_avgs.low_average - dst_highest_buy_order) / dst_highest_buy_order)
                    > config.ignore_difference_between_history_and_order_pct =>
        {
            dst_avgs.low_average
        }
        (Some(dst_highest_buy_order), _) => outbid_price(dst_highest_buy_order, true),
        (_, Some(dst_avgs)) => dst_avgs.low_average,
        (_, _) => return Err(anyhow!("No buy orders or market history")),
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
