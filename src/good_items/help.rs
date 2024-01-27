use good_lp::SolverModel;
use itertools::Itertools;
use ordered_float::NotNan;

use crate::{
    config::Config,
    item_type::{ItemHistoryDay, ItemTypeAveraged, Order, SystemMarketsItemData},
    requests::service::to_not_nan,
    stat::{AverageStat, MedianStat},
};

#[derive(Debug, Clone, Copy)]
pub struct PairCalculatedDataSellBuyFinal {
    pub single_item_desc_volume: f64,
    pub expenses: f64,
    pub sell_price: f64,
    pub max_profitable_buy: i64,
}

#[derive(Debug)]
pub struct PairCalculatedDataSellBuyFinal2<T> {
    pub calcs: PairCalculatedDataSellBuyFinal,
    pub m3_volume: i64,
    pub recommend_buy: i64,
    pub rough_profit: f64,
    pub item: T,
}

pub struct ProcessedSellBuyItems<T> {
    pub items: Vec<PairCalculatedDataSellBuyFinal2<T>>,
    pub sum_profit: f64,
    pub sum_volume: i32,
}

pub trait DataVecExt<T> {
    fn take_maximizing_profit(
        self,
        max_cargo: i32,
    ) -> Result<ProcessedSellBuyItems<T>, anyhow::Error>;
}

impl<T> DataVecExt<T> for Vec<T>
where
    T: Into<PairCalculatedDataSellBuyFinal> + Clone,
{
    fn take_maximizing_profit(
        self,
        max_cargo: i32,
    ) -> Result<ProcessedSellBuyItems<T>, anyhow::Error> {
        use good_lp::{default_solver, variable, Expression, ProblemVariables, Solution, Variable};
        let mut vars = ProblemVariables::new();
        let mut var_refs = Vec::new();
        for item in &self {
            let item: PairCalculatedDataSellBuyFinal = (*item).clone().into();
            let var_def = variable()
                .integer()
                .min(0)
                .max(item.max_profitable_buy as i32);
            var_refs.push(vars.add(var_def));
        }

        let goal = var_refs
            .iter()
            .zip(self.iter())
            .map(|(&var, item): (&Variable, &T)| -> Expression {
                let item: PairCalculatedDataSellBuyFinal = (item.clone()).into();
                (item.sell_price - item.expenses) * var
            })
            .sum::<Expression>();

        let space_constraint = var_refs
            .iter()
            .zip(self.iter())
            .map(|(&var, item): (&Variable, &T)| -> Expression {
                let item: PairCalculatedDataSellBuyFinal = (item.clone()).into();
                item.single_item_desc_volume * var
            })
            .sum::<Expression>()
            .leq(max_cargo);

        let mut solution = vars.maximise(&goal).using(default_solver);
        solution.set_parameter("log", "0");
        let solution = solution.with(space_constraint).solve()?;

        let recommended_items = var_refs
            .into_iter()
            .zip(self)
            .map(
                |(var, item): (Variable, T)| -> PairCalculatedDataSellBuyFinal2<_> {
                    let optimal = solution.value(var);
                    let recommend_buy = optimal as i64;

                    let item_converted: PairCalculatedDataSellBuyFinal = (item.clone()).into();

                    let volume =
                        (recommend_buy as f64 * item_converted.single_item_desc_volume) as i64;
                    PairCalculatedDataSellBuyFinal2 {
                        calcs: item_converted,
                        rough_profit: (item_converted.sell_price - item_converted.expenses)
                            * recommend_buy as f64,
                        recommend_buy,
                        m3_volume: volume,
                        item,
                    }
                },
            )
            .filter(|x: &PairCalculatedDataSellBuyFinal2<_>| x.calcs.max_profitable_buy > 0)
            .sorted_unstable_by_key(|x| NotNan::new(-x.rough_profit).unwrap())
            .collect::<Vec<_>>();

        let volume = recommended_items
            .iter()
            .map(|x| x.calcs.single_item_desc_volume * x.calcs.max_profitable_buy as f64)
            .sum::<f64>() as i32;
        Ok(ProcessedSellBuyItems {
            items: recommended_items,
            sum_profit: solution.eval(&goal),
            sum_volume: volume,
        })
    }
}

pub fn best_buy_volume_from_sell_to_sell(
    x: &[Order],
    recommend_buy_vol: i64,
    sell_price: f64,
    buy_broker_fee: f64,
    sell_broker_fee: f64,
    sell_tax: f64,
) -> (f64, i64) {
    let mut recommend_bought_volume = 0;
    let mut max_price = 0.;
    for order in x
        .iter()
        .filter(|x| !x.is_buy_order)
        .sorted_by_key(|x| NotNan::new(x.price).unwrap())
    {
        if max_price == 0. {
            max_price = order.price;
        }
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

pub fn averages(config: &Config, history: &[ItemHistoryDay]) -> Option<ItemTypeAveraged> {
    let last_n_days = history
        .iter()
        .rev()
        .take(config.common.days_average)
        .collect::<Vec<_>>();

    let avg_price = last_n_days
        .iter()
        .filter_map(|x| x.average)
        .map(to_not_nan)
        .median()
        .map(|x| *x);

    let mut avg_volume = *last_n_days
        .iter()
        .map(|x| x.volume as f64)
        .map(to_not_nan)
        .median()?;
    if avg_volume <= 1. {
        avg_volume = *last_n_days
            .iter()
            .map(|x| x.volume as f64)
            .map(to_not_nan)
            .average()?;
    }

    match (avg_price, avg_volume) {
        (Some(p), v) => Some(ItemTypeAveraged {
            average: p,
            volume: v,
        }),
        _ => None,
    }
}

pub fn weighted_price(config: &Config, history: &[ItemHistoryDay]) -> f64 {
    let last_n_days = history
        .iter()
        .rev()
        .take(config.common.days_average)
        .collect::<Vec<_>>();

    let sum_volume = last_n_days.iter().map(|x| x.volume).sum::<i64>() as f64;

    last_n_days
        .iter()
        .map(|x| x.average.unwrap() * x.volume as f64)
        .sum::<f64>()
        / sum_volume
}

pub fn match_buy_orders_profit(
    orders: impl Iterator<Item = Order>,
    mut quantity: i64,
    price_expense: f64,
    sales_tax: f64,
) -> (f64, i64) {
    let mut sum_sell_to_buy_price = 0.;
    let mut recommend_sold_volume = 0;
    'outer: for buy_order in orders
        .filter(|x| x.is_buy_order)
        .sorted_by_key(|x| NotNan::new(-x.price).unwrap())
    {
        let mut buy_order_fulfilled = buy_order.volume_remain;
        while buy_order_fulfilled > 0 {
            let sold_volume = buy_order_fulfilled.min(quantity);
            buy_order_fulfilled -= sold_volume;

            let expenses = price_expense * sold_volume as f64;

            let sell_sum_price = sold_volume as f64 * buy_order.price * (1. - sales_tax);

            if expenses >= sell_sum_price {
                break;
            }

            quantity -= sold_volume;
            sum_sell_to_buy_price += buy_order.price * sold_volume as f64;
            recommend_sold_volume += sold_volume;

            if quantity == 0 {
                break 'outer;
            }
        }
    }

    (sum_sell_to_buy_price, recommend_sold_volume)
}

pub fn match_buy_from_sell_orders<'a>(
    orders: impl Iterator<Item = &'a Order>,
    mut quantity: i64,
) -> (f64, i64) {
    let mut max_price = 0.;
    let mut recommend_bought_volume = 0;
    'outer: for sell_order in orders
        .filter(|x| !x.is_buy_order)
        .sorted_by_key(|x| NotNan::new(x.price).unwrap())
    {
        let mut sell_order_fulfilled = sell_order.volume_remain;
        while sell_order_fulfilled > 0 {
            let bought_volume = sell_order_fulfilled.min(quantity);
            sell_order_fulfilled -= bought_volume;

            quantity -= bought_volume;
            max_price = sell_order.price.max(max_price);
            recommend_bought_volume += bought_volume;

            if quantity == 0 {
                break 'outer;
            }
        }
    }

    (max_price, recommend_bought_volume)
}

pub struct PairCalculatedDataSellSellCommon {
    pub market: SystemMarketsItemData,
    pub margin: f64,
    pub rough_profit: f64,
    pub market_dest_volume: i64,
    pub recommend_buy: i64,
    pub expenses: f64,
    pub sell_price: f64,
    pub filled_for_days: Option<f64>,
    pub src_buy_price: f64,
    pub dest_min_sell_price: f64,
    pub src_avgs: Option<ItemTypeAveraged>,
    pub dst_avgs: ItemTypeAveraged,
    pub market_src_volume: i64,
    pub lost_per_day: f64,
}

pub fn prepare_sell_sell(
    config: &Config,
    market_data: SystemMarketsItemData,
    src_volume_on_market: i64,
    src_avgs: Option<ItemTypeAveraged>,
    dst_volume_on_market: i64,
    dst_avgs: ItemTypeAveraged,
    lost_per_day: f64,
) -> PairCalculatedDataSellSellCommon {
    let dst_lowest_sell_order = market_data
        .destination
        .orders
        .iter()
        .filter(|x| !x.is_buy_order)
        .map(|x| to_not_nan(x.price))
        .min()
        .map(|x| *x);
    let dst_weighted_price = weighted_price(config, &market_data.destination.history);
    let dest_sell_price =
        dst_lowest_sell_order.map_or(dst_weighted_price, |x| x.min(dst_weighted_price));

    let expected_item_volume_per_day = dst_avgs.volume.max(lost_per_day);

    let max_buy_vol = (expected_item_volume_per_day * config.common.sell_sell.rcmnd_fill_days)
        .max(1.)
        .min(src_volume_on_market as f64)
        .floor() as i64;
    let (buy_from_src_price, buy_from_src_volume) = best_buy_volume_from_sell_to_sell(
        market_data.source.orders.as_slice(),
        max_buy_vol,
        dest_sell_price,
        config.route.source.broker_fee,
        config.route.destination.broker_fee,
        config.common.sales_tax,
    );
    let buy_price = buy_from_src_price * (1. + config.route.source.broker_fee);
    let expenses = buy_price
        + market_data.desc.volume as f64 * config.common.sell_sell.freight_cost_iskm3
        + buy_price * config.common.sell_sell.freight_cost_collateral_percent;
    let sell_price_with_taxes =
        dest_sell_price * (1. - config.route.destination.broker_fee - config.common.sales_tax);
    let margin = (sell_price_with_taxes - expenses) / expenses;
    let rough_profit = (sell_price_with_taxes - expenses) * buy_from_src_volume as f64;
    let filled_for_days =
        (dst_avgs.volume > 0.).then_some(1. / dst_avgs.volume * dst_volume_on_market as f64);
    PairCalculatedDataSellSellCommon {
        market: market_data,
        margin,
        rough_profit,
        market_dest_volume: dst_volume_on_market,
        recommend_buy: buy_from_src_volume,
        expenses,
        sell_price: sell_price_with_taxes,
        filled_for_days,
        src_buy_price: buy_from_src_price,
        dest_min_sell_price: dest_sell_price,
        market_src_volume: src_volume_on_market,
        src_avgs,
        dst_avgs,
        lost_per_day,
    }
}
