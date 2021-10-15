use itertools::Itertools;
use ordered_float::NotNan;
use term_table::{row::Row, table_cell::TableCell};

use crate::{
    config::Config,
    item_type::{ItemTypeAveraged, SystemMarketsItemData},
    order_ext::OrderIterExt,
    zkb::killmails::ItemFrequencies,
};

use super::help::{averages, total_buy_from_sell_order_price};

pub fn get_good_items_sell_sell_zkb(
    pairs: Vec<SystemMarketsItemData>,
    zkb_items: ItemFrequencies,
    config: &Config,
) -> Vec<PairCalculatedDataSellSellZkb> {
    pairs
        .into_iter()
        .map(|x| {
            let item_lose_popularity = *zkb_items.items.get(&x.desc.type_id).unwrap_or(&0);
            let period_days = (zkb_items.period_seconds as f64) / 60. / 60. / 24.;
            let lost_per_day = item_lose_popularity as f64 / period_days;

            let src_mkt_orders = x.source.orders.iter().only_substantial_orders();
            let src_mkt_volume = src_mkt_orders.iter().copied().sell_order_volume();

            let dst_mkt_orders = x.destination.orders.iter().only_substantial_orders();
            let dst_mkt_volume: i32 = dst_mkt_orders.iter().copied().sell_order_volume();

            let src_avgs = averages(config, &x.source.history);
            let dst_avgs = averages(config, &x.destination.history);

            let recommend_buy_vol = (lost_per_day * config.rcmnd_fill_days)
                .max(1.)
                .min(src_mkt_volume as f64)
                .floor() as i32;

            let src_sell_order_price = (!x.source.orders.iter().any(|x| !x.is_buy_order))
                .then_some(src_avgs.highest.unwrap())
                .unwrap_or_else(|| {
                    total_buy_from_sell_order_price(x.source.orders.as_slice(), recommend_buy_vol)
                        / recommend_buy_vol as f64
                });

            let buy_price = src_sell_order_price * (1. + config.broker_fee);
            let expenses = buy_price
                + x.desc.volume.unwrap() as f64 * config.freight_cost_iskm3
                + buy_price * config.freight_cost_collateral_percent;

            // average can be none only if there's no history in dest
            // in this case we make history
            let dest_sell_price = dst_avgs.average.unwrap_or(expenses * 1.2);

            let sell_price = dest_sell_price * (1. - config.broker_fee - config.sales_tax);

            let margin = (sell_price - expenses) / expenses;

            let rough_profit = (sell_price - expenses) * recommend_buy_vol as f64;

            let filled_for_days =
                (lost_per_day > 0.).then(|| 1. / lost_per_day * dst_mkt_volume as f64);

            PairCalculatedDataSellSellZkb {
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
                lost_per_day,
            }
        })
        .filter(|x| x.margin > config.margin_cutoff)
        .filter(|x| {
            x.src_avgs.volume > config.min_src_volume
                && x.lost_per_day > config.min_dst_zkb_lost_volume
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

pub fn make_table_sell_sell_zkb<'a, 'b>(
    good_items: &'a [PairCalculatedDataSellSellZkb],
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
        TableCell::new("rcmnd vlm"),
        TableCell::new("fld fr dy"),
        TableCell::new("lost pr dy"),
    ]))
    .chain(good_items.iter().map(|it| {
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
            TableCell::new(format!("{:.2}", it.src_avgs.volume)),
            TableCell::new(format!("{:.2}", it.dst_avgs.volume)),
            TableCell::new(format!("{:.2}", it.market_src_volume)),
            TableCell::new(format!("{:.2}", it.market_dest_volume)),
            TableCell::new(format!("{:.2}", it.rough_profit)),
            TableCell::new(format!("{}", it.recommend_buy)),
            TableCell::new(
                it.filled_for_days
                    .map_or("N/A".to_string(), |x| format!("{:.2}", x)),
            ),
            TableCell::new(format!("{:.2}", it.lost_per_day)),
        ])
    }))
    .collect::<Vec<_>>();
    rows
}

pub struct PairCalculatedDataSellSellZkb {
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
    pub lost_per_day: f64,
}
