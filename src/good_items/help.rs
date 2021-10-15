use itertools::Itertools;
use ordered_float::NotNan;

use crate::{
    config::Config,
    item_type::{ItemHistoryDay, ItemTypeAveraged, Order},
    requests::to_not_nan,
    stat::AverageStat,
};

pub fn total_buy_from_sell_order_price(x: &[Order], recommend_buy_vol: i32) -> f64 {
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
        average: lastndays
            .iter()
            .map(|x| x.average)
            .flatten()
            .map(to_not_nan)
            .average()
            .map(|x| *x),
        highest: lastndays
            .iter()
            .map(|x| x.highest)
            .flatten()
            .map(to_not_nan)
            .average()
            .map(|x| *x),
        lowest: lastndays
            .iter()
            .map(|x| x.lowest)
            .flatten()
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
