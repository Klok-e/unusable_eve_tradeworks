use itertools::Itertools;
use ordered_float::NotNan;
use term_table::{row::Row, table_cell::TableCell};

use crate::{
    config::Config, item_type::SystemMarketsItemData, order_ext::OrderIterExt,
    zkb::killmails::ItemFrequencies,
};

use super::help::{calculate_item_averages, prepare_sell_sell, PairCalculatedDataSellSellCommon};

pub fn get_good_items_sell_sell(
    pairs: Vec<SystemMarketsItemData>,
    config: &Config,
    disable_filters: bool,
    zkb_items: ItemFrequencies,
) -> Vec<PairCalculatedDataSellSell> {
    pairs
        .into_iter()
        .filter_map(|x| {
            let item_lose_popularity = *zkb_items.items.get(&x.desc.type_id).unwrap_or(&0);
            let period_days = (zkb_items.period_seconds as f64) / 60. / 60. / 24.;
            let lost_per_day = item_lose_popularity as f64 / period_days;

            let src_mkt_orders = x.source.orders.clone();
            let src_volume_on_market = src_mkt_orders.iter().sell_order_volume();

            let dst_mkt_orders = x.destination.orders.clone();
            let dst_volume_on_market = dst_mkt_orders.iter().sell_order_volume();

            let src_avgs = calculate_item_averages(config, &x.source.history).or_else(|| {
                log::debug!(
                    "Item {} ({}) doesn't have any history in source.",
                    x.desc.name,
                    x.desc.type_id
                );
                None
            });
            let dst_avgs =
                calculate_item_averages(config, &x.destination.history).or_else(|| {
                    log::debug!(
                        "Item {} ({}) doesn't have any history in destination.",
                        x.desc.name,
                        x.desc.type_id
                    );
                    None
                })?;

            let common = prepare_sell_sell(
                config,
                x,
                src_volume_on_market,
                src_avgs,
                dst_volume_on_market,
                dst_avgs,
                lost_per_day,
            );

            Some(PairCalculatedDataSellSell { common })
        })
        .filter(|x| disable_filters || x.margin > config.common.margin_cutoff)
        .filter(|x| {
            disable_filters
                || x.src_avgs.map(|x| x.volume).unwrap_or(0f64)
                    > config.common.sell_sell.min_src_volume
                    && (x.dst_avgs.volume > config.common.sell_sell.min_dst_volume
                        || x.common.lost_per_day
                            > config
                                .common
                                .sell_sell
                                .sell_sell_zkb
                                .min_dst_zkb_lost_volume)
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
        .sorted_unstable_by_key(|x| NotNan::new(-x.rough_profit).unwrap())
        .take(config.common.items_take)
        .collect::<Vec<_>>()
}

pub fn make_table_sell_sell<'b>(
    good_items: &[PairCalculatedDataSellSell],
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
        TableCell::new("rcmnd vlm"),
        TableCell::new("fld fr dy"),
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
            TableCell::new(format!("{:.2}", it.lost_per_day)),
            TableCell::new(format!("{:.2}", it.rough_profit)),
            TableCell::new(format!("{}", it.recommend_buy)),
            TableCell::new(
                it.filled_for_days
                    .map_or("N/A".to_string(), |x| format!("{:.2}", x)),
            ),
        ])
    }))
    .collect::<Vec<_>>();
    rows
}

pub struct PairCalculatedDataSellSell {
    pub common: PairCalculatedDataSellSellCommon,
}

impl std::ops::Deref for PairCalculatedDataSellSell {
    type Target = PairCalculatedDataSellSellCommon;

    fn deref(&self) -> &Self::Target {
        &self.common
    }
}
