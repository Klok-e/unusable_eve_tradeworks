use std::ops::Deref;

use itertools::Itertools;
use ordered_float::NotNan;

use crate::{item_type::Order, requests::service::to_not_nan};

pub trait OrderIterExt<'a, It>
where
    It: Deref<Target = Order>,
{
    fn sell_order_volume(self) -> i64;
    fn sell_order_min_price(self) -> Option<f64>;
    fn get_lowest_sell_order_over_volume(self, volume: f64) -> Option<f64>;
    fn get_highest_buy_order_over_volume(self, volume: f64) -> Option<f64>;
}

impl<'a, T, It> OrderIterExt<'a, It> for T
where
    T: Iterator<Item = It>,
    It: Deref<Target = Order> + Copy,
{
    fn sell_order_volume(self) -> i64 {
        self.filter(|x| !x.is_buy_order)
            .map(|x| x.volume_remain)
            .sum()
    }

    fn sell_order_min_price(self) -> Option<f64> {
        self.filter(|x| !x.is_buy_order)
            .map(|x| to_not_nan(x.price))
            .min()
            .map(|x| *x)
    }

    fn get_lowest_sell_order_over_volume(self, volume: f64) -> Option<f64> {
        let mut accumulated_volume = 0_f64;

        self.filter(|x| !x.is_buy_order)
            .sorted_by_key(|x| NotNan::new(x.price).unwrap())
            .map(|x| (to_not_nan(x.price), x.volume_remain))
            .find_map(|(price, vol_remain)| {
                accumulated_volume += vol_remain as f64;
                if accumulated_volume >= volume {
                    Some(*price)
                } else {
                    None
                }
            })
    }

    fn get_highest_buy_order_over_volume(self, volume: f64) -> Option<f64> {
        let mut accumulated_volume = 0_f64;

        self.filter(|x| x.is_buy_order)
            .sorted_by_key(|x| NotNan::new(-x.price).unwrap())
            .map(|x| (to_not_nan(x.price), x.volume_remain))
            .find_map(|(price, vol_remain)| {
                accumulated_volume += vol_remain as f64;
                if accumulated_volume >= volume {
                    Some(*price)
                } else {
                    log::debug!("Order {price}, remain {vol_remain}, lower than min volume {volume}; skipping...");
                    None
                }
            })
    }
}
