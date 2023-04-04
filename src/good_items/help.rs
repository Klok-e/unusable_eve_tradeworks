use itertools::Itertools;
use ordered_float::NotNan;

use crate::{
    config::Config,
    item_type::{ItemHistoryDay, ItemTypeAveraged, Order, SystemMarketsItemData},
    requests::service::to_not_nan,
    stat::AverageStat,
};

pub fn best_buy_volume_from_sell_to_sell(
    x: &[Order],
    recommend_buy_vol: i32,
    sell_price: f64,
    buy_broker_fee: f64,
    sell_broker_fee: f64,
    sell_tax: f64,
) -> (f64, i32) {
    let mut recommend_bought_volume = 0;
    let mut max_price = 0.;
    for order in x
        .iter()
        .filter(|x| !x.is_buy_order)
        .sorted_by_key(|x| NotNan::new(x.price).unwrap())
    {
        if max_price == 0. {
            max_price = order.price;
        }
        let current_buy = order
            .volume_remain
            .min(recommend_buy_vol - recommend_bought_volume);

        let profit =
            sell_price * (1. - sell_broker_fee - sell_tax) - order.price * (1. + buy_broker_fee);
        if profit <= 0. {
            break;
        }

        recommend_bought_volume += current_buy;
        max_price = order.price.max(max_price);
        if recommend_buy_vol <= recommend_bought_volume {
            break;
        }
    }
    (max_price, recommend_bought_volume)
}

pub fn averages(config: &Config, history: &[ItemHistoryDay]) -> Option<ItemTypeAveraged> {
    let last_n_days = history
        .iter()
        .rev()
        .take(config.common.days_average)
        .collect::<Vec<_>>();

    let avg_price = last_n_days
        .iter()
        .filter_map(|x| x.average)
        .map(to_not_nan)
        .average()
        .map(|x| *x);
    let avg_volume = *last_n_days
        .iter()
        .map(|x| x.volume as f64)
        .map(to_not_nan)
        .average()
        .unwrap();
    match (avg_price, avg_volume) {
        (Some(p), v) => Some(ItemTypeAveraged {
            average: p,
            volume: v,
        }),
        _ => None,
    }
}

pub fn weighted_price(config: &Config, history: &[ItemHistoryDay]) -> f64 {
    let last_n_days = history
        .iter()
        .rev()
        .take(config.common.days_average)
        .collect::<Vec<_>>();

    let sum_volume = last_n_days.iter().map(|x| x.volume).sum::<i64>() as f64;

    last_n_days
        .iter()
        .map(|x| x.average.unwrap() * x.volume as f64)
        .sum::<f64>()
        / sum_volume
}

pub struct PairCalculatedDataSellSellCommon {
    pub market: SystemMarketsItemData,
    pub margin: f64,
    pub rough_profit: f64,
    pub market_dest_volume: i32,
    pub recommend_buy: i32,
    pub expenses: f64,
    pub sell_price: f64,
    pub filled_for_days: Option<f64>,
    pub src_buy_price: f64,
    pub dest_min_sell_price: f64,
    pub src_avgs: Option<ItemTypeAveraged>,
    pub dst_avgs: ItemTypeAveraged,
    pub market_src_volume: i32,
}

pub fn prepare_sell_sell(
    config: &Config,
    market_data: SystemMarketsItemData,
    volume_dest: f64,
    src_volume_on_market: i32,
    src_avgs: Option<ItemTypeAveraged>,
    dst_volume_on_market: i32,
    dst_avgs: ItemTypeAveraged,
) -> PairCalculatedDataSellSellCommon {
    let dst_lowest_sell_order = market_data
        .destination
        .orders
        .iter()
        .filter(|x| !x.is_buy_order)
        .map(|x| to_not_nan(x.price))
        .min()
        .map(|x| *x);
    let dst_weighted_price = weighted_price(config, &market_data.destination.history);
    let dest_sell_price =
        dst_lowest_sell_order.map_or(dst_weighted_price, |x| x.min(dst_weighted_price));
    let max_buy_vol = (volume_dest * config.common.sell_sell.rcmnd_fill_days)
        .max(1.)
        .min(src_volume_on_market as f64)
        .floor() as i32;
    let (buy_from_src_price, buy_from_src_volume) = best_buy_volume_from_sell_to_sell(
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
    let filled_for_days =
        (volume_dest > 0.).then_some(1. / volume_dest * dst_volume_on_market as f64);
    PairCalculatedDataSellSellCommon {
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
    }
}
