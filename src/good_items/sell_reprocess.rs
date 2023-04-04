use std::collections::HashMap;

use itertools::Itertools;
use num_format::{Locale, ToFormattedString};
use ordered_float::NotNan;
use term_table::{row::Row, table_cell::TableCell};

use crate::{
    config::Config,
    datadump_service::{DatadumpService, ReprocessItemInfo},
    item_type::{ItemTypeAveraged, SystemMarketsItemData},
    order_ext::OrderIterExt,
};

use super::help::{averages, match_buy_orders_profit};

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
        .iter()
        .filter_map(|x| process_item_pair(datadump, x, config, &items_map))
        .filter(|x| disable_filters || x.best_margin > config.common.margin_cutoff)
        .filter(|x| {
            disable_filters
                || config
                    .common
                    .min_profit
                    .map_or(true, |min_prft| x.rough_profit > min_prft)
        })
        .sorted_unstable_by_key(|x| NotNan::new(-x.rough_profit).unwrap())
        .map(|item| PairCalculatedDataSellReprocessFinal {
            market: item.market,
            margin: item.margin,
            rough_profit: item.rough_profit,
            market_dest_volume: item.market_dest_volume,
            recommend_buy: item.recommend_buy,
            expenses: item.expenses,
            sell_price: item.sell_price,
            src_buy_price: item.src_buy_price,
            dest_min_sell_price: item.dest_min_sell_price,
            src_avgs: item.src_avgs,
            dst_avgs: item.dst_avgs,
            market_src_volume: item.market_src_volume,
            volume: 0,
        })
        .take(config.common.items_take)
        .collect::<Vec<_>>();

    let volume = recommended_items
        .iter()
        .map(|x| x.market.desc.volume as f64 * x.recommend_buy as f64)
        .sum::<f64>() as i32;
    let profit = recommended_items
        .iter()
        .map(|x| x.rough_profit)
        .sum::<f64>();
    Ok(ProcessedSellBuyItems {
        items: recommended_items,
        sum_profit: profit,
        sum_volume: volume,
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
    let dst_mkt_volume: i32 = dst_mkt_orders.iter().sell_order_volume();

    let src_avgs = averages(config, &x.source.history);
    let dst_avgs = averages(config, &x.destination.history);

    let (recommend_buy_vol, dest_sell_price, max_buy_price, avg_buy_price) =
        calculate_prices_volumes(x, config, &reprocess, items_map)?;

    // multibuy can only buy at a fixed price, so all buys from multiple sell orders
    // with different prices have you paid the same price for all of them
    let expenses = max_buy_price;
    let buy_with_broker_fee = expenses * (1. + config.route.source.broker_fee);
    let fin_sell_price = dest_sell_price * (1. - config.common.sales_tax);

    let margin = (fin_sell_price - buy_with_broker_fee) / buy_with_broker_fee;

    let rough_profit = (fin_sell_price - buy_with_broker_fee) * recommend_buy_vol as f64;

    // also calculate avg buy price
    let best_expenses = avg_buy_price;
    let buy_with_broker_fee = best_expenses * (1. + config.route.source.broker_fee);
    let fin_sell_price = dest_sell_price * (1. - config.common.sales_tax);

    let best_margin = (fin_sell_price - buy_with_broker_fee) / buy_with_broker_fee;

    Some(PairCalculatedDataSellReprocess {
        market: x.clone(),
        margin,
        best_margin,
        rough_profit,
        market_dest_volume: dst_mkt_volume,
        recommend_buy: recommend_buy_vol,
        expenses: buy_with_broker_fee,
        sell_price: fin_sell_price,
        src_buy_price: expenses,
        dest_min_sell_price: dest_sell_price,
        market_src_volume: src_mkt_volume,
        src_avgs,
        dst_avgs,
    })
}

fn calculate_prices_volumes(
    x: &SystemMarketsItemData,
    config: &Config,
    reprocess: &ReprocessItemInfo,
    items_map: &HashMap<i32, &SystemMarketsItemData>,
) -> Option<(i32, f64, f64, f64)> {
    let source_sell_orders = x
        .source
        .orders
        .iter()
        .cloned()
        .filter(|x| !x.is_buy_order)
        .sorted_by_key(|x| NotNan::new(x.price).unwrap());
    let mut recommend_bought_volume = 0;
    let mut sum_sell_price = 0.;
    let mut max_buy_price: f64 = 0.;
    let mut sum_buy_price = 0.;
    'outer: for sell_order in source_sell_orders {
        for _ in 0..sell_order.volume_remain {
            let mut item_reproc_sell = 0.;
            for reprocessed_item in &reprocess.reprocessed_into {
                // some items can't be sold but can be retrieved by reprocessing
                // example: Mangled Sansha Data Analyzer
                if !items_map.contains_key(&reprocessed_item.item_id) {
                    return None;
                }
                let reprocessed_item_buy_orders = items_map[&reprocessed_item.item_id]
                    .destination
                    .orders
                    .iter()
                    .cloned()
                    .filter(|x| x.is_buy_order)
                    .sorted_by_key(|x| NotNan::new(-x.price).unwrap());
                let (sum_received, matched) = match_buy_orders_profit(
                    reprocessed_item_buy_orders,
                    (reprocessed_item.quantity as f64 * config.common.sell_reprocess.repro_portion)
                        as i32,
                    0.,
                    0.,
                );
                if matched == 0 {
                    break;
                }

                item_reproc_sell += sum_received;
            }

            let expenses = max_buy_price.max(sell_order.price)
                * (recommend_bought_volume + 1) as f64
                * (1. + config.route.source.broker_fee);
            let profit = item_reproc_sell
                * (1. - config.common.sales_tax)
                * (1. - config.route.destination.broker_fee);
            if expenses >= profit {
                break 'outer;
            }

            recommend_bought_volume += 1;
            max_buy_price = max_buy_price.max(sell_order.price);
            sum_sell_price += item_reproc_sell;
            sum_buy_price += max_buy_price * recommend_bought_volume as f64;
        }
    }
    if recommend_bought_volume == 0 {
        return None;
    }

    Some((
        recommend_bought_volume,
        sum_sell_price / recommend_bought_volume as f64,
        max_buy_price,
        sum_buy_price / recommend_bought_volume as f64,
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
        TableCell::new("sell prc"),
        TableCell::new("margin"),
        TableCell::new("vlm src"),
        TableCell::new("vlm dst"),
        TableCell::new("mkt src"),
        TableCell::new("mkt dst"),
        TableCell::new("rough prft"),
        TableCell::new("rcmnd"),
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
            TableCell::new(format!("{:.2}", it.sell_price)),
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
        ])
    }))
    .chain(std::iter::once(Row::new(vec![
        TableCell::new("total profit"),
        TableCell::new_with_col_span(
            (good_items.sum_profit.round() as i64).to_formatted_string(&Locale::fr),
            12,
        ),
    ])))
    .chain(std::iter::once(Row::new(vec![
        TableCell::new("total volume"),
        TableCell::new_with_col_span(good_items.sum_volume.to_formatted_string(&Locale::fr), 12),
    ])))
    .collect::<Vec<_>>();
    rows
}

#[derive(Debug, Clone)]
struct PairCalculatedDataSellReprocess {
    market: SystemMarketsItemData,
    margin: f64,
    rough_profit: f64,
    market_dest_volume: i32,
    recommend_buy: i32,
    expenses: f64,
    sell_price: f64,
    src_buy_price: f64,
    dest_min_sell_price: f64,
    src_avgs: Option<ItemTypeAveraged>,
    dst_avgs: Option<ItemTypeAveraged>,
    market_src_volume: i32,
    best_margin: f64,
}

#[derive(Debug, Clone)]
pub struct PairCalculatedDataSellReprocessFinal {
    pub market: SystemMarketsItemData,
    margin: f64,
    rough_profit: f64,
    market_dest_volume: i32,
    pub recommend_buy: i32,
    expenses: f64,
    sell_price: f64,
    src_buy_price: f64,
    pub dest_min_sell_price: f64,
    src_avgs: Option<ItemTypeAveraged>,
    dst_avgs: Option<ItemTypeAveraged>,
    market_src_volume: i32,
    volume: i32,
}

pub struct ProcessedSellBuyItems {
    pub items: Vec<PairCalculatedDataSellReprocessFinal>,
    pub sum_profit: f64,
    pub sum_volume: i32,
}
