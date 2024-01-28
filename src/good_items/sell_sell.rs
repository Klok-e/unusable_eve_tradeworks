use num_format::{Locale, ToFormattedString};

use term_table::{row::Row, table_cell::TableCell};

use crate::{
    config::Config,
    item_type::{ItemTypeAveraged, SystemMarketsItemData},
    order_ext::OrderIterExt,
    requests::service::to_not_nan,
    zkb::killmails::ItemFrequencies,
};

use super::help::{self, calculate_item_averages, calculate_optimal_buy_volume};
use super::help::{calculate_weighted_price, DataVecExt};

pub fn get_good_items_sell_sell(
    pairs: Vec<SystemMarketsItemData>,
    config: &Config,
    disable_filters: bool,
    zkb_items: ItemFrequencies,
) -> Result<help::ProfitableItemsSummary<PairCalculatedDataSellSell>, anyhow::Error> {
    pairs
        .into_iter()
        .filter_map(|market_data| {
            let item_lose_popularity =
                *zkb_items.items.get(&market_data.desc.type_id).unwrap_or(&0);
            let period_days = (zkb_items.period_seconds as f64) / 60. / 60. / 24.;
            let lost_per_day = item_lose_popularity as f64 / period_days;

            let src_mkt_orders = market_data.source.orders.clone();
            let src_volume_on_market = src_mkt_orders.iter().sell_order_volume();

            let dst_mkt_orders = market_data.destination.orders.clone();
            let dst_volume_on_market = dst_mkt_orders.iter().sell_order_volume();

            let src_avgs = calculate_item_averages(config, &market_data.source.history).or(None);
            let dst_avgs =
                calculate_item_averages(config, &market_data.destination.history).or(None);

            let common = prepare_sell_sell(
                config,
                market_data,
                src_volume_on_market,
                src_avgs,
                dst_volume_on_market,
                dst_avgs,
                lost_per_day,
            )?;

            Some(common)
        })
        .filter(|x| disable_filters || x.margin > config.common.margin_cutoff)
        .filter(|x| {
            disable_filters
                || x.src_avgs.map(|x| x.volume).unwrap_or(0f64)
                    >= config.common.sell_sell.min_src_volume
                    && x.dst_avgs.map(|x| x.volume).unwrap_or(0f64)
                        >= config.common.sell_sell.min_dst_volume
                    && x.lost_per_day
                        >= config
                            .common
                            .sell_sell
                            .sell_sell_zkb
                            .min_dst_zkb_lost_volume
                    && config
                        .common
                        .min_profit
                        .map_or(true, |min_prft| x.rough_profit > min_prft)
        })
        .filter(|x| {
            disable_filters
                || if let Some(filled_for_days) = x.filled_for_days {
                    filled_for_days < config.common.sell_sell.max_filled_for_days_cutoff
                } else {
                    true
                }
        })
        .collect::<Vec<_>>()
        .take_maximizing_profit(config.common.cargo_capacity)
}

pub fn make_table_sell_sell<'b>(
    good_items: &help::ProfitableItemsSummary<PairCalculatedDataSellSell>,
    name_length: usize,
) -> Vec<Row<'b>> {
    let rows = std::iter::once(Row::new(vec![
        TableCell::new("id"),
        TableCell::new("itm nm"),
        TableCell::new("src p"),
        TableCell::new("dst p"),
        TableCell::new("expns"),
        TableCell::new("sll p"),
        TableCell::new("mrgn"),
        TableCell::new("vlm src"),
        TableCell::new("vlm dst"),
        TableCell::new("mkt src"),
        TableCell::new("mkt dst"),
        TableCell::new("lst"),
        TableCell::new("rgh prft"),
        TableCell::new("buy"),
        TableCell::new("fld"),
    ]))
    .chain(good_items.items.iter().map(|it| {
        let it = &it.item;
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
            TableCell::new(format!("{:.2}", it.lost_per_day)),
            TableCell::new(format!("{:.2}", it.rough_profit)),
            TableCell::new(format!("{}", it.recommend_buy)),
            TableCell::new(
                it.filled_for_days
                    .map_or("N/A".to_string(), |x| format!("{:.2}", x)),
            ),
        ])
    }))
    .chain(std::iter::once(Row::new(vec![
        TableCell::new("total profit"),
        TableCell::new_with_col_span(
            (good_items.sum_profit.round() as i64).to_formatted_string(&Locale::fr),
            14,
        ),
    ])))
    .chain(std::iter::once(Row::new(vec![
        TableCell::new("total volume"),
        TableCell::new_with_col_span(good_items.total_volume.to_formatted_string(&Locale::fr), 14),
    ])))
    .collect::<Vec<_>>();
    rows
}

#[derive(Debug, Clone)]
pub struct PairCalculatedDataSellSell {
    pub market: SystemMarketsItemData,
    pub margin: f64,
    pub rough_profit: f64,
    pub market_dest_volume: i64,
    pub recommend_buy: i64,
    pub expenses: f64,
    pub sell_price: f64,
    pub filled_for_days: Option<f64>,
    pub src_buy_price: f64,
    pub dest_min_sell_price: f64,
    pub src_avgs: Option<ItemTypeAveraged>,
    pub dst_avgs: Option<ItemTypeAveraged>,
    pub market_src_volume: i64,
    pub lost_per_day: f64,
}

impl From<PairCalculatedDataSellSell> for help::ItemProfitData {
    fn from(value: PairCalculatedDataSellSell) -> Self {
        help::ItemProfitData {
            single_item_volume_m3: value.market.desc.volume as f64,
            expenses: value.expenses,
            sell_price: value.sell_price,
            max_profitable_buy_size: value.recommend_buy,
        }
    }
}

pub fn prepare_sell_sell(
    config: &Config,
    market_data: SystemMarketsItemData,
    src_volume_on_market: i64,
    src_avgs: Option<ItemTypeAveraged>,
    dst_volume_on_market: i64,
    dst_avgs: Option<ItemTypeAveraged>,
    lost_per_day: f64,
) -> Option<PairCalculatedDataSellSell> {
    let src_lowest_sell_order = market_data.source.orders.iter().sell_order_min_price()?;

    let dst_lowest_sell_order = if let Some(dst_avgs) = dst_avgs {
        market_data
            .destination
            .orders
            .iter()
            .get_lowest_sell_order_over_volume(
                dst_avgs.volume * config.common.sell_sell.dst_ignore_orders_under_volume_pct,
            )
    } else {
        market_data.destination.orders.iter().sell_order_min_price()
    };

    let mut sell_with_markup =
        src_lowest_sell_order * (1. + config.common.sell_sell.markup_if_no_orders_dest);

    let weighted_price = calculate_weighted_price(config, &market_data.destination.history);
    if let Ok(weighted_price) = weighted_price {
        sell_with_markup = sell_with_markup.max(weighted_price)
    }

    let dest_sell_price = match dst_lowest_sell_order {
        Some(dst_lowest_sell_order) => (dst_lowest_sell_order * 0.999).min(sell_with_markup),
        None => sell_with_markup,
    };

    let lost_per_day_scaled = lost_per_day
        * config
            .common
            .sell_sell
            .sell_sell_zkb
            .zkb_losses_volume_multiplier;

    let expected_item_volume_per_day = match dst_avgs {
        Some(x) => x.volume.max(lost_per_day_scaled),
        None => lost_per_day_scaled,
    };

    let max_buy_vol = (expected_item_volume_per_day * config.common.sell_sell.rcmnd_fill_days)
        .min(src_volume_on_market as f64)
        .floor() as i64;
    let (buy_from_src_price, buy_from_src_volume) = calculate_optimal_buy_volume(
        market_data.source.orders.as_slice(),
        max_buy_vol,
        dest_sell_price,
        config.route.source.broker_fee,
        config.route.destination.broker_fee,
        config.common.sales_tax,
    );
    let buy_price = buy_from_src_price * (1. + config.route.source.broker_fee);
    let expenses = buy_price
        + market_data.desc.volume as f64 * config.common.sell_sell.freight_cost_iskm3
        + buy_price * config.common.sell_sell.freight_cost_collateral_percent;
    let sell_price_with_taxes =
        dest_sell_price * (1. - config.route.destination.broker_fee - config.common.sales_tax);
    let margin = (sell_price_with_taxes - expenses) / expenses;
    let rough_profit = (sell_price_with_taxes - expenses) * buy_from_src_volume as f64;

    let filled_for_days = if dst_volume_on_market == 0 {
        Some(0.)
    } else if expected_item_volume_per_day > 0. {
        Some(*to_not_nan(
            1. / expected_item_volume_per_day * dst_volume_on_market as f64,
        ))
    } else {
        None
    };

    Some(PairCalculatedDataSellSell {
        market: market_data,
        margin,
        rough_profit,
        market_dest_volume: dst_volume_on_market,
        recommend_buy: buy_from_src_volume,
        expenses,
        sell_price: sell_price_with_taxes,
        filled_for_days,
        src_buy_price: buy_from_src_price,
        dest_min_sell_price: dest_sell_price,
        market_src_volume: src_volume_on_market,
        src_avgs,
        dst_avgs,
        lost_per_day,
    })
}
