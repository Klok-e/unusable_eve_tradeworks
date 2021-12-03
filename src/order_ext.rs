use std::ops::Deref;

use crate::item_type::Order;

pub trait OrderIterExt<'a, It>
where
    It: Deref<Target = Order>,
{
    fn sell_order_volume(self) -> i32;
}

impl<'a, T, It> OrderIterExt<'a, It> for T
where
    T: Iterator<Item = It>,
    It: Deref<Target = Order> + Copy,
{
    fn sell_order_volume(self) -> i32 {
        let market_volume: i32 = self
            .filter(|x| !x.is_buy_order)
            .map(|x| x.volume_remain)
            .sum();
        market_volume
    }
}
