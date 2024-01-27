use good_lp::SolverModel;
use itertools::Itertools;
use num_format::{Locale, ToFormattedString};
use ordered_float::NotNan;
use term_table::{row::Row, table_cell::TableCell};

use crate::{
    config::Config,
    item_type::{ItemTypeAveraged, SystemMarketsItemData},
    order_ext::OrderIterExt,
};

use super::help::{self, calculate_item_averages, DataVecExt};
pub fn get_good_items_sell_buy(
    pairs: Vec<SystemMarketsItemData>,
    config: &Config,
    disable_filters: bool,
) -> Result<help::ProfitableItemsSummary<PairCalculatedDataSellBuy>, anyhow::Error> {
    pairs
        .into_iter()
        .filter_map(|x| calculate_pairs(x, config))
        .filter(|x| disable_filters || x.margin > config.common.margin_cutoff)
        .collect::<Vec<_>>()
        .take_maximizing_profit(config.common.sell_buy.cargo_capacity)
}

fn calculate_pairs(x: SystemMarketsItemData, config: &Config) -> Option<PairCalculatedDataSellBuy> {
    let src_mkt_orders = x.source.orders.clone();
    let src_mkt_volume = src_mkt_orders.iter().sell_order_volume();

    let dst_mkt_orders = x.destination.orders.clone();
    let dst_mkt_volume = dst_mkt_orders.iter().sell_order_volume();

    let src_avgs = calculate_item_averages(config, &x.source.history);
    let dst_avgs = calculate_item_averages(config, &x.destination.history);

    let (max_profitable_buy_volume, dest_sell_price, max_buy_price, avg_buy_price) =
        calculate_prices_volumes(&x, config)?;

    // multibuy can only buy at a fixed price, so all buys from multiple sell orders
    // with different prices have you paid the same price for all of them
    let expenses = max_buy_price;
    let buy_with_broker_fee = expenses * (1. + config.route.source.broker_fee);
    let fin_sell_price = dest_sell_price * (1. - config.common.sales_tax);

    let margin = (fin_sell_price - buy_with_broker_fee) / buy_with_broker_fee;

    // also calculate avg buy price
    let best_expenses = avg_buy_price;
    let buy_with_broker_fee = best_expenses * (1. + config.route.source.broker_fee);
    let fin_sell_price = dest_sell_price * (1. - config.common.sales_tax);

    Some(PairCalculatedDataSellBuy {
        market: x,
        margin,
        market_dest_volume: dst_mkt_volume,
        max_profitable_buy_volume,
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
) -> Option<(i64, f64, f64, f64)> {
    let mut source_sell_orders = x
        .source
        .orders
        .iter()
        .filter(|&x| !x.is_buy_order)
        .cloned()
        .sorted_by_key(|x| NotNan::new(x.price).unwrap());
    let mut curr_src_sell_order = source_sell_orders.next()?;
    let mut max_profitable_buy_volume = 0;
    let mut sum_sell_price = 0.;
    let mut max_buy_price = 0.;
    let mut sum_buy_price = 0.;
    'outer: for buy_order in x
        .destination
        .orders
        .iter()
        .filter(|x| x.is_buy_order)
        .sorted_by_key(|x| NotNan::new(-x.price).unwrap())
    {
        let mut buy_order_fulfilled = buy_order.volume_remain;
        while buy_order_fulfilled > 0 {
            let bought_volume = buy_order_fulfilled.min(curr_src_sell_order.volume_remain);
            buy_order_fulfilled -= bought_volume;

            let expenses = (curr_src_sell_order.price * (1. + config.route.source.broker_fee))
                * bought_volume as f64;

            let sell_price =
                bought_volume as f64 * buy_order.price * (1. - config.common.sales_tax);

            if expenses >= sell_price {
                break;
            }
            sum_buy_price += curr_src_sell_order.price * bought_volume as f64;
            curr_src_sell_order.volume_remain -= bought_volume;
            max_buy_price = curr_src_sell_order.price.max(max_buy_price);
            sum_sell_price += buy_order.price * bought_volume as f64;
            max_profitable_buy_volume += bought_volume;

            if curr_src_sell_order.volume_remain == 0 {
                curr_src_sell_order = if let Some(x) = source_sell_orders.next() {
                    x
                } else {
                    break 'outer;
                }
            }
        }
    }
    Some((
        max_profitable_buy_volume,
        sum_sell_price / max_profitable_buy_volume as f64,
        max_buy_price,
        sum_buy_price / max_profitable_buy_volume as f64,
    ))
}

pub fn make_table_sell_buy<'b>(
    good_items: &help::ProfitableItemsSummary<PairCalculatedDataSellBuy>,
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
        TableCell::new("vlm"),
    ]))
    .chain(good_items.items.iter().map(|it| {
        let item = &it.item;
        let short_name =
            item.market.desc.name[..(name_length.min(item.market.desc.name.len()))].to_owned();
        Row::new(vec![
            TableCell::new(format!("{}", item.market.desc.type_id)),
            TableCell::new(short_name),
            TableCell::new(format!("{:.2}", item.src_buy_price)),
            TableCell::new(format!("{:.2}", item.dest_min_sell_price)),
            TableCell::new(format!("{:.2}", item.expenses)),
            TableCell::new(format!("{:.2}", item.sell_price)),
            TableCell::new(format!("{:.2}", item.margin)),
            TableCell::new(format!(
                "{:.2}",
                item.src_avgs.map(|x| x.volume).unwrap_or(0f64)
            )),
            TableCell::new(format!(
                "{:.2}",
                item.dst_avgs.map(|x| x.volume).unwrap_or(0f64)
            )),
            TableCell::new(format!("{:.2}", item.market_src_volume)),
            TableCell::new(format!("{:.2}", item.market_dest_volume)),
            TableCell::new(format!("{:.2}", it.rough_profit)),
            TableCell::new(format!("{}", it.recommend_buy)),
            TableCell::new(format!("{}", it.volume_m3)),
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
        TableCell::new_with_col_span(good_items.total_volume.to_formatted_string(&Locale::fr), 13),
    ])))
    .collect::<Vec<_>>();
    rows
}

#[derive(Debug, Clone)]
pub struct PairCalculatedDataSellBuy {
    pub market: SystemMarketsItemData,
    margin: f64,
    market_dest_volume: i64,
    max_profitable_buy_volume: i64,
    expenses: f64,
    sell_price: f64,
    src_buy_price: f64,
    pub dest_min_sell_price: f64,
    src_avgs: Option<ItemTypeAveraged>,
    dst_avgs: Option<ItemTypeAveraged>,
    market_src_volume: i64,
}

impl From<PairCalculatedDataSellBuy> for help::ItemProfitData {
    fn from(value: PairCalculatedDataSellBuy) -> Self {
        help::ItemProfitData {
            single_item_volume_m3: value.market.desc.volume as f64,
            expenses: value.expenses,
            sell_price: value.sell_price,
            max_profitable_buy: value.max_profitable_buy_volume,
        }
    }
}
