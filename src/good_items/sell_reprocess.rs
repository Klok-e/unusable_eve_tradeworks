use itertools::Itertools;
use num_format::{Locale, ToFormattedString};
use ordered_float::NotNan;
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use std::collections::HashMap;
use term_table::{row::Row, table_cell::TableCell};

use crate::{
    config::Config,
    datadump_service::{DatadumpService, ReprocessItemInfo},
    item_type::{ItemTypeAveraged, SystemMarketsItemData},
    order_ext::OrderIterExt,
};

use super::help::{averages, match_buy_from_sell_orders, match_buy_orders_profit};

pub fn get_good_items_sell_reprocess(
    pairs: Vec<SystemMarketsItemData>,
    orig_pairs: Vec<SystemMarketsItemData>,
    config: &Config,
    disable_filters: bool,
    datadump: &DatadumpService,
) -> Result<ProcessedSellBuyItems, anyhow::Error> {
    let items_map: HashMap<i32, &SystemMarketsItemData> = orig_pairs
        .iter()
        .map(|x| (x.desc.type_id, x))
        .collect::<HashMap<_, _>>();
    let recommended_items = pairs
        .par_iter()
        .filter_map(|x| process_item_pair(datadump, x, config, &items_map))
        .filter(|x| disable_filters || x.margin > config.common.margin_cutoff)
        .filter(|x| {
            disable_filters
                || config
                    .common
                    .min_profit
                    .map_or(true, |min_prft| x.rough_profit > min_prft)
        })
        .collect::<Vec<_>>()
        .into_iter()
        .sorted_unstable_by_key(|x| NotNan::new(-x.rough_profit).unwrap())
        .map(|item| PairCalculatedDataSellReprocessFinal {
            market: item.market,
            margin: item.margin,
            rough_profit: item.rough_profit,
            market_dest_volume: item.market_dest_volume,
            recommend_buy: item.recommend_buy,
            expenses: item.expenses,
            profit: item.profit,
            src_buy_price: item.src_buy_price,
            dest_min_sell_price: item.dest_min_sell_price,
            src_avgs: item.src_avgs,
            dst_avgs: item.dst_avgs,
            market_src_volume: item.market_src_volume,
            portion_size: item.portion_size,
            reprocess_volume: item.reprocess_volume,
        })
        .take(config.common.items_take)
        .collect::<Vec<_>>();

    let volume = recommended_items
        .iter()
        .map(|x| x.market.desc.volume as f64 * x.recommend_buy as f64)
        .sum::<f64>() as i32;
    let reprocess_volume = recommended_items
        .iter()
        .map(|x| x.reprocess_volume)
        .sum::<f64>() as i32;
    let profit = recommended_items
        .iter()
        .map(|x| x.rough_profit)
        .sum::<f64>();
    Ok(ProcessedSellBuyItems {
        items: recommended_items,
        sum_profit: profit,
        sum_volume: volume,
        reprocess_volume,
    })
}

fn process_item_pair(
    datadump: &DatadumpService,
    x: &SystemMarketsItemData,
    config: &Config,
    items_map: &HashMap<i32, &SystemMarketsItemData>,
) -> Option<PairCalculatedDataSellReprocess> {
    let reprocess = datadump.get_reprocess_items(x.desc.type_id).unwrap();
    if reprocess.reprocessed_into.is_empty() {
        return None;
    }

    let src_mkt_orders = x.source.orders.clone();
    let src_mkt_volume = src_mkt_orders.iter().sell_order_volume();

    let dst_mkt_orders = x.destination.orders.clone();
    let dst_mkt_volume = dst_mkt_orders.iter().sell_order_volume();

    let src_avgs = averages(config, &x.source.history);
    let dst_avgs = averages(config, &x.destination.history);

    let (
        recommend_buy_vol,
        dest_sell_price,
        max_buy_price,
        _avg_buy_price,
        expenses_total,
        profit_total,
        result_volume,
    ) = calculate_prices_volumes(x, config, &reprocess, items_map)?;

    // multibuy can only buy at a fixed price, so all buys from multiple sell orders
    // with different prices have you paid the same price for all of them
    let margin = (profit_total - expenses_total) / expenses_total;

    let rough_profit = profit_total - expenses_total;

    Some(PairCalculatedDataSellReprocess {
        market: x.clone(),
        margin,
        rough_profit,
        market_dest_volume: dst_mkt_volume,
        recommend_buy: recommend_buy_vol,
        expenses: expenses_total,
        profit: profit_total,
        src_buy_price: max_buy_price,
        dest_min_sell_price: dest_sell_price,
        market_src_volume: src_mkt_volume,
        src_avgs,
        dst_avgs,
        portion_size: x.desc.portion_size.unwrap(),
        reprocess_volume: result_volume,
    })
}

fn calculate_prices_volumes(
    x: &SystemMarketsItemData,
    config: &Config,
    reprocess: &ReprocessItemInfo,
    items_map: &HashMap<i32, &SystemMarketsItemData>,
) -> Option<(i64, f64, f64, f64, f64, f64, f64)> {
    let source_sell_orders = x
        .source
        .orders
        .iter()
        .cloned()
        .filter(|x| !x.is_buy_order)
        .sorted_by_key(|x| NotNan::new(x.price).unwrap())
        .collect::<Vec<_>>();
    let mut recommend_buy_volume = 0;
    let mut sum_sell_price = 0.;
    let mut sum_buy_price = 0.;
    let mut max_buy_price = 0.;
    let mut expenses = 0.;
    let mut profit = 0.;
    let mut volume = 0.;

    let min_reprocess_quantity = x.desc.portion_size.unwrap();

    loop {
        let total_buy_quantity = recommend_buy_volume + min_reprocess_quantity;
        let (buy_price, quantity) =
            match_buy_from_sell_orders(source_sell_orders.iter(), total_buy_quantity);
        if quantity != total_buy_quantity {
            break;
        }

        let mut item_reproc_sell = 0.;
        let mut item_reproc_sum_adjusted_price = 0.;
        let mut new_repro_volume = 0.;
        for reprocessed_item in &reprocess.reprocessed_into {
            // some items can't be sold but can be retrieved by reprocessing
            // example: Mangled Sansha Data Analyzer
            if !items_map.contains_key(&reprocessed_item.item_id) {
                return None;
            }
            let system_markets_item_data = &items_map[&reprocessed_item.item_id];
            let adjusted_price = system_markets_item_data.adjusted_price.unwrap_or(0.);
            let reprocessed_item_buy_orders = system_markets_item_data
                .destination
                .orders
                .iter()
                .cloned()
                .filter(|x| x.is_buy_order)
                .sorted_by_key(|x| NotNan::new(-x.price).unwrap());
            let (sum_received, matched) = match_buy_orders_profit(
                reprocessed_item_buy_orders,
                (total_buy_quantity as f64 / min_reprocess_quantity as f64
                    * reprocessed_item.quantity as f64
                    * config.common.sell_reprocess.repro_portion) as i64,
                0.,
                0.,
            );

            item_reproc_sell += sum_received;
            item_reproc_sum_adjusted_price += adjusted_price * matched as f64;
            new_repro_volume += system_markets_item_data.desc.volume as f64 * matched as f64;
        }

        let new_expenses =
            buy_price * (total_buy_quantity) as f64 * (1. + config.route.source.broker_fee)
                + (item_reproc_sum_adjusted_price * config.common.sell_reprocess.repro_tax);
        let new_profit = item_reproc_sell
            * (1. - config.common.sales_tax)
            * (1. - config.route.destination.broker_fee);

        if new_expenses >= new_profit {
            break;
        }

        expenses = new_expenses;
        profit = new_profit;
        volume = new_repro_volume;

        recommend_buy_volume += min_reprocess_quantity;
        sum_sell_price = item_reproc_sell;
        sum_buy_price = buy_price * recommend_buy_volume as f64;
        max_buy_price = buy_price;
    }

    if recommend_buy_volume == 0 {
        return None;
    }

    Some((
        recommend_buy_volume,
        sum_sell_price / recommend_buy_volume as f64,
        max_buy_price,
        sum_buy_price / recommend_buy_volume as f64,
        expenses,
        profit,
        volume,
    ))
}

pub fn make_table_sell_reprocess<'b>(
    good_items: &ProcessedSellBuyItems,
    name_length: usize,
) -> Vec<Row<'b>> {
    let rows = std::iter::once(Row::new(vec![
        TableCell::new("id"),
        TableCell::new("item name"),
        TableCell::new("src prc"),
        TableCell::new("dst prc"),
        TableCell::new("expenses"),
        TableCell::new("profit"),
        TableCell::new("margin"),
        TableCell::new("vlm src"),
        TableCell::new("vlm dst"),
        TableCell::new("mkt src"),
        TableCell::new("mkt dst"),
        TableCell::new("rough prft"),
        TableCell::new("rcmnd"),
        TableCell::new("prtn sz"),
    ]))
    .chain(good_items.items.iter().map(|it| {
        let short_name =
            it.market.desc.name[..(name_length.min(it.market.desc.name.len()))].to_owned();
        Row::new(vec![
            TableCell::new(format!("{}", it.market.desc.type_id)),
            TableCell::new(short_name),
            TableCell::new(format!("{:.2}", it.src_buy_price)),
            TableCell::new(format!("{:.2}", it.dest_min_sell_price)),
            TableCell::new(format!("{:.2}", it.expenses)),
            TableCell::new(format!("{:.2}", it.profit)),
            TableCell::new(format!("{:.2}", it.margin)),
            TableCell::new(format!(
                "{:.2}",
                it.src_avgs.map(|x| x.volume).unwrap_or(0f64)
            )),
            TableCell::new(format!(
                "{:.2}",
                it.dst_avgs.map(|x| x.volume).unwrap_or(0f64)
            )),
            TableCell::new(format!("{:.2}", it.market_src_volume)),
            TableCell::new(format!("{:.2}", it.market_dest_volume)),
            TableCell::new(format!("{:.2}", it.rough_profit)),
            TableCell::new(format!("{}", it.recommend_buy)),
            TableCell::new(format!("{}", it.portion_size)),
        ])
    }))
    .chain(std::iter::once(Row::new(vec![
        TableCell::new("total profit"),
        TableCell::new_with_col_span(
            (good_items.sum_profit.round() as i64).to_formatted_string(&Locale::fr),
            13,
        ),
    ])))
    .chain(std::iter::once(Row::new(vec![
        TableCell::new("total volume"),
        TableCell::new_with_col_span(good_items.sum_volume.to_formatted_string(&Locale::fr), 13),
    ])))
    .chain(std::iter::once(Row::new(vec![
        TableCell::new("reprocess volume"),
        TableCell::new_with_col_span(
            good_items.reprocess_volume.to_formatted_string(&Locale::fr),
            13,
        ),
    ])))
    .collect::<Vec<_>>();
    rows
}

#[derive(Debug, Clone)]
struct PairCalculatedDataSellReprocess {
    market: SystemMarketsItemData,
    margin: f64,
    rough_profit: f64,
    market_dest_volume: i64,
    recommend_buy: i64,
    expenses: f64,
    profit: f64,
    src_buy_price: f64,
    dest_min_sell_price: f64,
    src_avgs: Option<ItemTypeAveraged>,
    dst_avgs: Option<ItemTypeAveraged>,
    market_src_volume: i64,
    portion_size: i64,
    reprocess_volume: f64,
}

#[derive(Debug, Clone)]
pub struct PairCalculatedDataSellReprocessFinal {
    pub market: SystemMarketsItemData,
    margin: f64,
    rough_profit: f64,
    market_dest_volume: i64,
    pub recommend_buy: i64,
    expenses: f64,
    profit: f64,
    src_buy_price: f64,
    pub dest_min_sell_price: f64,
    src_avgs: Option<ItemTypeAveraged>,
    dst_avgs: Option<ItemTypeAveraged>,
    market_src_volume: i64,
    portion_size: i64,
    reprocess_volume: f64,
}

pub struct ProcessedSellBuyItems {
    pub items: Vec<PairCalculatedDataSellReprocessFinal>,
    pub sum_profit: f64,
    pub sum_volume: i32,
    pub reprocess_volume: i32,
}
