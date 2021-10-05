use std::ops::Deref;

use itertools::Itertools;

use crate::item_type::Order;

pub trait OrderIterExt<'a, It>
where
    It: Deref<Target = Order>,
{
    fn only_substantial_orders(self) -> Vec<It>;
    fn sell_order_volume(self) -> i32;
}

impl<'a, T, It> OrderIterExt<'a, It> for T
where
    T: Iterator<Item = It>,
    It: Deref<Target = Order> + Copy,
{
    fn only_substantial_orders(self) -> Vec<It> {
        let orders = self.collect::<Vec<_>>();
        let src_market_volume = orders.iter().copied().sell_order_volume();
        let src_perc_sell_orders = orders
            .into_iter()
            .group_by(|x| {
                let mut num = (x.price * 100.).round() as i64;
                // get first 2 figits
                while num > 99 {
                    num = num / 10;
                }
                num
            })
            .into_iter()
            .map(|(k, v)| (k, v.collect::<Vec<It>>()))
            .filter(|(_, v)| v.iter().copied().sell_order_volume() as f64 / src_market_volume as f64 > 0.05)
            .map(|(_, v)| v.into_iter())
            .flatten()
            .collect_vec();
        src_perc_sell_orders
    }

    fn sell_order_volume(self) -> i32 {
        let market_volume: i32 = self
            .filter(|x| !x.is_buy_order)
            .map(|x| x.volume_remain)
            .sum();
        market_volume
    }
}
