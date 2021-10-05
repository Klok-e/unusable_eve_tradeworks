use itertools::Itertools;
use ordered_float::NotNan;

use crate::{PairCalculatedData, averages, config::Config, item_type::SystemMarketsItemData, order_ext::OrderIterExt};

pub fn get_good_items(pairs: Vec<SystemMarketsItemData>, config: &Config) -> Vec<PairCalculatedData> {
    let good_items = pairs
        .into_iter()
        .map(|x| {
            let src_mkt_orders = x.source.orders.iter().only_substantial_orders();
            let src_mkt_volume = src_mkt_orders.iter().copied().sell_order_volume();

            let dst_mkt_orders = x.destination.orders.iter().only_substantial_orders();
            let dst_mkt_volume: i32 = dst_mkt_orders.iter().copied().sell_order_volume();

            let src_avgs = averages(&config, &x.source.history);
            let dst_avgs = averages(&config, &x.destination.history);

            let recommend_buy_vol = (dst_avgs.volume * config.rcmnd_fill_days)
                .max(1.)
                .min(src_mkt_volume as f64)
                .floor() as i32;

            let src_sell_order_price = (!x.source.orders.iter().any(|x| !x.is_buy_order))
                .then_some(src_avgs.highest)
                .unwrap_or_else(|| {
                    let mut recommend_bought_volume = 0;
                    let mut max_price = 0.;
                    for order in x
                        .source
                        .orders
                        .iter()
                        .filter(|x| !x.is_buy_order)
                        .sorted_by_key(|x| NotNan::new(x.price).unwrap())
                    {
                        recommend_bought_volume += order.volume_remain.min(recommend_buy_vol);
                        max_price = order.price;
                        if recommend_buy_vol <= recommend_bought_volume {
                            break;
                        }
                    }
                    max_price
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
        .collect::<Vec<_>>();
    good_items
}