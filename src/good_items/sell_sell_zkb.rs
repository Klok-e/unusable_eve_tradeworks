use itertools::Itertools;
use ordered_float::NotNan;
use term_table::{row::Row, table_cell::TableCell};

use crate::{
    config::Config, item_type::SystemMarketsItemData, order_ext::OrderIterExt,
    zkb::killmails::ItemFrequencies,
};

use super::help::{averages, prepare_sell_sell, PairCalculatedDataSellSellCommon};

pub fn get_good_items_sell_sell_zkb(
    pairs: Vec<SystemMarketsItemData>,
    zkb_items: ItemFrequencies,
    config: &Config,
) -> Vec<PairCalculatedDataSellSellZkb> {
    pairs
        .into_iter()
        .filter_map(|x| -> Option<_> {
            let item_lose_popularity = *zkb_items.items.get(&x.desc.type_id).unwrap_or(&0);
            let period_days = (zkb_items.period_seconds as f64) / 60. / 60. / 24.;
            let lost_per_day = item_lose_popularity as f64 / period_days;

            let src_mkt_orders = x.source.orders.clone();
            let src_volume_on_market = src_mkt_orders.iter().sell_order_volume();

            let dst_mkt_orders = x.destination.orders.clone();
            let dst_volume_on_market: i32 = dst_mkt_orders.iter().sell_order_volume();

            let src_avgs = averages(config, &x.source.history).or_else(|| {
                log::debug!(
                    "Item {} ({}) doesn't have any history in source.",
                    x.desc.name,
                    x.desc.type_id
                );
                None
            });
            let dst_avgs = averages(config, &x.destination.history).or_else(|| {
                log::debug!(
                    "Item {} ({}) doesn't have any history in destination.",
                    x.desc.name,
                    x.desc.type_id
                );
                None
            })?;

            let volume_dest = lost_per_day;

            let common = prepare_sell_sell(
                config,
                x,
                volume_dest,
                src_volume_on_market,
                src_avgs,
                dst_volume_on_market,
                dst_avgs,
            )?;
            Some(PairCalculatedDataSellSellZkb {
                common,
                lost_per_day,
            })
        })
        .filter(|x| x.margin > config.margin_cutoff)
        .filter(|x| {
            x.src_avgs.map(|x| x.volume).unwrap_or(0f64) > config.min_src_volume
                && x.lost_per_day > config.min_dst_zkb_lost_volume
                && config
                    .min_profit
                    .map_or(true, |min_prft| x.rough_profit > min_prft)
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
            TableCell::new(format!(
                "{:.2}",
                it.src_avgs.map(|x| x.volume).unwrap_or(0f64)
            )),
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
    pub common: PairCalculatedDataSellSellCommon,
    pub lost_per_day: f64,
}

impl std::ops::Deref for PairCalculatedDataSellSellZkb {
    type Target = PairCalculatedDataSellSellCommon;

    fn deref(&self) -> &Self::Target {
        &self.common
    }
}
