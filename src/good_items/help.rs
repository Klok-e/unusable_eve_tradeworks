use itertools::Itertools;
use ordered_float::NotNan;

use crate::{
    config::Config,
    item_type::{ItemHistoryDay, ItemTypeAveraged, Order, SystemMarketsItemData},
    requests::to_not_nan,
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

pub fn averages(config: &Config, history: &[ItemHistoryDay]) -> ItemTypeAveraged {
    let lastndays = history
        .iter()
        .rev()
        .take(config.days_average)
        .collect::<Vec<_>>();
    ItemTypeAveraged {
        average: lastndays
            .iter()
            .filter_map(|x| x.average)
            .map(to_not_nan)
            .average()
            .map(|x| *x),
        highest: lastndays
            .iter()
            .filter_map(|x| x.highest)
            .map(to_not_nan)
            .average()
            .map(|x| *x),
        lowest: lastndays
            .iter()
            .filter_map(|x| x.lowest)
            .map(to_not_nan)
            .average()
            .map(|x| *x),
        order_count: *lastndays
            .iter()
            .map(|x| x.order_count as f64)
            .map(to_not_nan)
            .average()
            .unwrap(),
        volume: *lastndays
            .iter()
            .map(|x| x.volume as f64)
            .map(to_not_nan)
            .average()
            .unwrap(),
    }
}

pub fn max_avg_price(config: &Config, history: &[ItemHistoryDay]) -> Option<f64> {
    let lastndays = history
        .iter()
        .rev()
        .take(config.days_average)
        .collect::<Vec<_>>();

    lastndays
        .iter()
        .filter_map(|x| x.average)
        .map(to_not_nan)
        .max()
        .map(|x| *x)
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
    pub src_avgs: ItemTypeAveraged,
    pub dst_avgs: ItemTypeAveraged,
    pub market_src_volume: i32,
}

pub fn prepare_sell_sell(
    dst_mkt_orders: Vec<crate::item_type::Order>,
    config: &Config,
    x: SystemMarketsItemData,
    volume_dest: f64,
    src_volume_on_market: i32,
    src_avgs: ItemTypeAveraged,
    dst_volume_on_market: i32,
    dst_avgs: ItemTypeAveraged,
) -> Option<PairCalculatedDataSellSellCommon> {
    let dst_lowest_sell_order = dst_mkt_orders
        .iter()
        .filter(|x| !x.is_buy_order)
        .map(|x| to_not_nan(x.price))
        .min()
        .map(|x| *x);
    let dst_max_price = max_avg_price(config, &x.destination.history);
    let dest_sell_price = dst_max_price.unwrap_or_else(|| {
        log::debug!(
            "Item {} ({}) doesn't have any history in destination.",
            x.desc.name,
            x.desc.type_id
        );
        src_avgs.average.unwrap_or(0.) * 1.3
    });
    let dest_sell_price = dst_lowest_sell_order.map_or(dest_sell_price, |x| x.min(dest_sell_price));
    let max_buy_vol = (volume_dest * config.rcmnd_fill_days)
        .max(1.)
        .min(src_volume_on_market as f64)
        .floor() as i32;
    let (buy_from_src_price, buy_from_src_volume) =
        (!x.source.orders.iter().any(|x| !x.is_buy_order))
            .then_some((
                src_avgs.highest.or_else(|| {
                    log::debug!(
                        "Item {} ({}) doesn't have any history in source.",
                        x.desc.name,
                        x.desc.type_id
                    );
                    None
                })?,
                max_buy_vol,
            ))
            .unwrap_or_else(|| {
                let (price, volume) = best_buy_volume_from_sell_to_sell(
                    x.source.orders.as_slice(),
                    max_buy_vol,
                    dest_sell_price,
                    config.broker_fee_source,
                    config.broker_fee_destination,
                    config.sales_tax,
                );
                (price, volume)
            });
    if buy_from_src_volume == 0 {
        return None;
    }
    let buy_price = buy_from_src_price * (1. + config.broker_fee_source);
    let expenses = buy_price
        + x.desc.volume.unwrap() as f64 * config.freight_cost_iskm3
        + buy_price * config.freight_cost_collateral_percent;
    let sell_price = dest_sell_price * (1. - config.broker_fee_destination - config.sales_tax);
    let margin = (sell_price - expenses) / expenses;
    let rough_profit = (sell_price - expenses) * buy_from_src_volume as f64;
    let filled_for_days =
        (volume_dest > 0.).then(|| 1. / volume_dest * dst_volume_on_market as f64);
    Some(PairCalculatedDataSellSellCommon {
        market: x,
        margin,
        rough_profit,
        market_dest_volume: dst_volume_on_market,
        recommend_buy: buy_from_src_volume,
        expenses,
        sell_price,
        filled_for_days,
        src_buy_price: buy_from_src_price,
        dest_min_sell_price: dest_sell_price,
        market_src_volume: src_volume_on_market,
        src_avgs,
        dst_avgs,
    })
}
