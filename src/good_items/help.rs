use itertools::Itertools;
use ordered_float::NotNan;

use crate::{
    config::Config,
    item_type::{ItemHistoryDay, ItemTypeAveraged, Order},
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
