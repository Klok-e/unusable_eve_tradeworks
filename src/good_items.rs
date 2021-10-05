use itertools::Itertools;
use ordered_float::NotNan;

use crate::{
    config::Config,
    item_type::{ItemHistoryDay, ItemTypeAveraged, Order, SystemMarketsItemData},
    order_ext::OrderIterExt,
    requests::to_not_nan,
    stat::AverageStat,
    PairCalculatedData,
};

pub fn get_good_items_sell_sell(
    pairs: Vec<SystemMarketsItemData>,
    config: &Config,
) -> Vec<PairCalculatedData> {
    pairs
        .into_iter()
        .map(|x| {
            let src_mkt_orders = x.source.orders.iter().only_substantial_orders();
            let src_mkt_volume = src_mkt_orders.iter().copied().sell_order_volume();

            let dst_mkt_orders = x.destination.orders.iter().only_substantial_orders();
            let dst_mkt_volume: i32 = dst_mkt_orders.iter().copied().sell_order_volume();

            let src_avgs = averages(config, &x.source.history);
            let dst_avgs = averages(config, &x.destination.history);

            let recommend_buy_vol = (dst_avgs.volume * config.rcmnd_fill_days)
                .max(1.)
                .min(src_mkt_volume as f64)
                .floor() as i32;

            let src_sell_order_price = (!x.source.orders.iter().any(|x| !x.is_buy_order))
                .then_some(src_avgs.highest)
                .unwrap_or_else(|| {
                    total_buy_from_sell_order_price(x.source.orders.as_slice(), recommend_buy_vol)
                        / recommend_buy_vol as f64
                });

            let buy_price = src_sell_order_price * (1. + config.broker_fee);
            let expenses = buy_price
                + x.desc.volume.unwrap() as f64 * config.freight_cost_iskm3
                + src_sell_order_price * config.freight_cost_collateral_percent;

            let dest_sell_price = dst_avgs.average;

            let sell_price = dest_sell_price * (1. - config.broker_fee - config.sales_tax);

            let margin = (sell_price - expenses) / expenses;

            let rough_profit = (sell_price - expenses) * recommend_buy_vol as f64;

            let filled_for_days =
                (dst_avgs.volume > 0.).then(|| 1. / dst_avgs.volume * dst_mkt_volume as f64);

            PairCalculatedData {
                market: x,
                margin,
                rough_profit,
                market_dest_volume: dst_mkt_volume,
                recommend_buy: recommend_buy_vol,
                expenses,
                sell_price,
                filled_for_days,
                src_buy_price: src_sell_order_price,
                dest_min_sell_price: dest_sell_price,
                market_src_volume: src_mkt_volume,
                src_avgs,
                dst_avgs,
            }
        })
        .filter(|x| x.margin > config.margin_cutoff)
        .filter(|x| {
            x.src_avgs.volume > config.min_src_volume && x.dst_avgs.volume > config.min_dst_volume
        })
        .filter(|x| {
            if let Some(filled_for_days) = x.filled_for_days {
                filled_for_days < config.max_filled_for_days_cutoff
            } else {
                true
            }
        })
        .sorted_unstable_by_key(|x| NotNan::new(-x.rough_profit).unwrap())
        .take(config.items_take)
        .collect::<Vec<_>>()
}

pub fn get_good_items_sell_buy(
    pairs: Vec<SystemMarketsItemData>,
    config: &Config,
) -> Vec<PairCalculatedData> {
    pairs
        .into_iter()
        .map(|x| {
            let src_mkt_orders = x.source.orders.iter().only_substantial_orders();
            let src_mkt_volume = src_mkt_orders.iter().copied().sell_order_volume();

            let src_avgs = averages(config, &x.source.history);

            let (recommend_buy_vol, dest_sell_price) = {
                let mut recommend_bought_volume = 0;
                let mut sum_sell_price = 0.;
                for order in x
                    .destination
                    .orders
                    .iter()
                    .filter(|x| x.is_buy_order)
                    .sorted_by_key(|x| NotNan::new(-x.price).unwrap())
                {
                    let buy_price = src_avgs.highest * (1. + config.broker_fee);
                    let expenses = buy_price
                        + x.desc.volume.unwrap() as f64 * config.freight_cost_iskm3
                        + src_avgs.highest * config.freight_cost_collateral_percent;

                    let sell_price = order.price * (1. - config.broker_fee - config.sales_tax);

                    if expenses <= sell_price {
                        break;
                    }
                    sum_sell_price += sell_price;
                    recommend_bought_volume += order.volume_remain;
                }
                (
                    recommend_bought_volume,
                    sum_sell_price / x.destination.orders.len() as f64,
                )
            };

            let src_sell_order_price = (!x.source.orders.iter().any(|x| !x.is_buy_order))
                .then_some(src_avgs.highest)
                .unwrap_or_else(|| {
                    total_buy_from_sell_order_price(x.source.orders.as_slice(), recommend_buy_vol)
                        / recommend_buy_vol as f64
                });

            let buy_price = src_sell_order_price * (1. + config.broker_fee);
            let expenses = buy_price
                + x.desc.volume.unwrap() as f64 * config.freight_cost_iskm3
                + src_sell_order_price * config.freight_cost_collateral_percent;

            let sell_price = dest_sell_price * (1. - config.broker_fee - config.sales_tax);

            let margin = (sell_price - expenses) / expenses;

            let rough_profit = (sell_price - expenses) * recommend_buy_vol as f64;

            PairCalculatedData {
                market: x,
                margin,
                rough_profit,
                market_dest_volume: 0,
                recommend_buy: recommend_buy_vol,
                expenses,
                sell_price,
                filled_for_days: None,
                src_buy_price: src_sell_order_price,
                dest_min_sell_price: dest_sell_price,
                market_src_volume: src_mkt_volume,
                src_avgs,
                dst_avgs: Default::default(),
            }
        })
        .filter(|x| x.margin > config.margin_cutoff)
        .filter(|x| {
            x.src_avgs.volume > config.min_src_volume && x.dst_avgs.volume > config.min_dst_volume
        })
        .filter(|x| {
            if let Some(filled_for_days) = x.filled_for_days {
                filled_for_days < config.max_filled_for_days_cutoff
            } else {
                true
            }
        })
        .sorted_unstable_by_key(|x| NotNan::new(-x.rough_profit).unwrap())
        .take(config.items_take)
        .collect::<Vec<_>>()
}

fn total_buy_from_sell_order_price(x: &[Order], recommend_buy_vol: i32) -> f64 {
    let mut recommend_bought_volume = 0;
    let mut total_price = 0.;
    for order in x
        .iter()
        .filter(|x| !x.is_buy_order)
        .sorted_by_key(|x| NotNan::new(x.price).unwrap())
    {
        let current_buy = order.volume_remain.min(recommend_buy_vol);
        recommend_bought_volume += current_buy;
        total_price += order.price * current_buy as f64;
        if recommend_buy_vol <= recommend_bought_volume {
            break;
        }
    }
    total_price
}

pub fn averages(config: &Config, history: &[ItemHistoryDay]) -> ItemTypeAveraged {
    let lastndays = history
        .iter()
        .rev()
        .take(config.days_average)
        .collect::<Vec<_>>();
    ItemTypeAveraged {
        average: *lastndays
            .iter()
            .map(|x| x.average)
            .map(to_not_nan)
            .average()
            .unwrap(),
        highest: *lastndays
            .iter()
            .map(|x| x.highest)
            .map(to_not_nan)
            .average()
            .unwrap(),
        lowest: *lastndays
            .iter()
            .map(|x| x.lowest)
            .map(to_not_nan)
            .average()
            .unwrap(),
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
