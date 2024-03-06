use anyhow::anyhow;
use good_lp::{IntoAffineExpression, SolverModel};
use itertools::Itertools;
use ordered_float::NotNan;

use crate::{
    config::CommonConfig,
    item_type::{ItemHistoryDay, ItemTypeAveraged, Order},
    requests::service::to_not_nan,
    stat::{AverageStat, MedianStat},
};

#[derive(Debug, Clone, Copy)]
pub struct ItemProfitData {
    pub single_item_volume_m3: f64,
    pub expenses: f64,
    pub sell_price: f64,
    pub max_item_amount: i64,
}

#[derive(Debug)]
pub struct ProcessedItemProfitData<T> {
    pub profit_data: ItemProfitData,
    pub volume_m3: i64,
    pub recommend_buy: i64,
    pub rough_profit: f64,
    pub item: T,
}

pub struct ProfitableItemsSummary<T> {
    pub items: Vec<ProcessedItemProfitData<T>>,
    pub sum_profit: f64,
    pub total_volume: i32,
}

pub trait DataVecExt<T> {
    fn take_maximizing_profit(
        self,
        max_cargo: i32,
        max_number_of_items: i32,
    ) -> Result<ProfitableItemsSummary<T>, anyhow::Error>;
}

impl<T> DataVecExt<T> for Vec<T>
where
    T: Into<ItemProfitData> + Clone,
{
    fn take_maximizing_profit(
        self,
        max_cargo: i32,
        max_number_of_items: i32,
    ) -> Result<ProfitableItemsSummary<T>, anyhow::Error> {
        use good_lp::{default_solver, variable, Expression, ProblemVariables, Solution, Variable};
        let mut vars = ProblemVariables::new();

        let mut binary_var_refs = Vec::new();
        for _ in &self {
            let binary_var_def = variable().binary();
            binary_var_refs.push(vars.add(binary_var_def));
        }

        let mut var_refs = Vec::new();
        for item in &self {
            let item: ItemProfitData = (*item).clone().into();
            let var_def = variable().integer().min(0).max(item.max_item_amount as i32);
            var_refs.push(vars.add(var_def));
        }

        let goal = var_refs
            .iter()
            .zip(self.iter())
            .map(|(&var, item): (&Variable, &T)| -> Expression {
                let item: ItemProfitData = (item.clone()).into();
                (item.sell_price - item.expenses) * var
            })
            .sum::<Expression>();

        let space_constraint = var_refs
            .iter()
            .zip(self.iter())
            .map(|(&var, item): (&Variable, &T)| -> Expression {
                let item: ItemProfitData = (item.clone()).into();
                item.single_item_volume_m3 * var
            })
            .sum::<Expression>()
            .leq(max_cargo);

        let mut solution = vars.maximise(&goal).using(default_solver);
        solution.set_parameter("log", "0");

        // link binary variables to variables
        for ((&var, &binary_var), item) in
            var_refs.iter().zip(binary_var_refs.iter()).zip(self.iter())
        {
            let item: ItemProfitData = (*item).clone().into();
            let max_buy_constraint = var
                .into_expression()
                .leq(binary_var * (item.max_item_amount as f64));
            solution = solution.with(max_buy_constraint);
        }
        let max_items_constraint = binary_var_refs
            .iter()
            .sum::<Expression>()
            .leq(max_number_of_items as f64);

        let solution = solution
            .with(max_items_constraint)
            .with(space_constraint)
            .solve()?;

        let recommended_items = var_refs
            .into_iter()
            .zip(self)
            .map(|(var, item): (Variable, T)| -> ProcessedItemProfitData<_> {
                let optimal = solution.value(var);
                let recommend_buy = optimal as i64;

                let item_converted: ItemProfitData = (item.clone()).into();

                let volume = (recommend_buy as f64 * item_converted.single_item_volume_m3) as i64;
                ProcessedItemProfitData {
                    profit_data: item_converted,
                    rough_profit: (item_converted.sell_price - item_converted.expenses)
                        * recommend_buy as f64,
                    recommend_buy,
                    volume_m3: volume,
                    item,
                }
            })
            .filter(|x: &ProcessedItemProfitData<_>| x.recommend_buy > 0)
            .sorted_unstable_by_key(|x| NotNan::new(-x.rough_profit).unwrap())
            .collect::<Vec<_>>();

        let volume = recommended_items
            .iter()
            .map(|x| x.profit_data.single_item_volume_m3 * x.recommend_buy as f64)
            .sum::<f64>() as i32;
        Ok(ProfitableItemsSummary {
            items: recommended_items,
            sum_profit: solution.eval(&goal),
            total_volume: volume,
        })
    }
}

pub fn calculate_optimal_buy_volume(
    orders: &[Order],
    recommend_buy_vol: i64,
    sell_price: f64,
    buy_broker_fee: f64,
    sell_broker_fee: f64,
    sell_tax: f64,
    max_investment: f64,
) -> (f64, i64) {
    let mut current_bought_volume = 0;
    let mut max_price = 0.;
    for order in orders
        .iter()
        .filter(|x| !x.is_buy_order)
        .sorted_by_key(|x| NotNan::new(x.price).unwrap())
    {
        if max_price == 0. {
            max_price = order.price;
        }

        let mut current_buy = order
            .volume_remain
            .min(recommend_buy_vol - current_bought_volume);

        // limit investment
        if max_price * (current_bought_volume + current_buy) as f64 > max_investment {
            current_buy = ((max_investment - max_price * current_bought_volume as f64) / max_price)
                .floor() as i64;
            if current_buy == 0 {
                break;
            }
        }

        let profit =
            sell_price * (1. - sell_broker_fee - sell_tax) - order.price * (1. + buy_broker_fee);
        if profit <= 0. {
            break;
        }

        current_bought_volume += current_buy;
        max_price = order.price.max(max_price);
        if recommend_buy_vol <= current_bought_volume {
            break;
        }
    }
    (max_price, current_bought_volume)
}

pub fn calculate_item_averages(
    config: &CommonConfig,
    history: &[ItemHistoryDay],
) -> Option<ItemTypeAveraged> {
    let last_n_days = history
        .iter()
        .rev()
        .take(config.days_average)
        .collect::<Vec<_>>();

    let avg_price = last_n_days
        .iter()
        .filter_map(|x| x.average)
        .map(to_not_nan)
        .median()
        .map(|x| *x)?;

    let avg_low_price = last_n_days
        .iter()
        .filter_map(|x| x.lowest)
        .map(to_not_nan)
        .median()
        .map(|x| *x)?;

    let avg_high_price = last_n_days
        .iter()
        .filter_map(|x| x.highest)
        .map(to_not_nan)
        .median()
        .map(|x| *x)?;

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

    Some(ItemTypeAveraged {
        average: avg_price,
        low_average: avg_low_price,
        high_average: avg_high_price,
        volume: avg_volume,
    })
}

pub fn calculate_weighted_price(
    config: &CommonConfig,
    history: &[ItemHistoryDay],
) -> anyhow::Result<f64> {
    let last_n_days = history
        .iter()
        .rev()
        .take(config.days_average)
        .collect::<Vec<_>>();

    let sum = last_n_days.iter().map(|x| x.volume).sum::<i64>();
    if sum == 0 {
        return Err(anyhow!("Historical volume is zero"));
    }

    let sum_volume = to_not_nan(sum as f64);

    Ok(*(to_not_nan(
        last_n_days
            .iter()
            .map(
                |x| Ok(x.average.ok_or(anyhow!("Item history average is None"))? * x.volume as f64),
            )
            .collect::<anyhow::Result<Vec<_>>>()?
            .iter()
            .sum::<f64>(),
    ) / sum_volume))
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

pub fn outbid_price(price: f64, is_buy_order: bool) -> f64 {
    // Determine the scale of the price to find out the position of the fourth digit
    let scale = price.log10().floor() as i32 + 1;

    let a = 10f64.powi(-(4 - scale));
    let mut price = price / a;
    if is_buy_order {
        let floor = price.floor();
        price = if (price - floor).abs() > 0.99 {
            price.round()
        } else {
            floor
        } * a;
    } else {
        let ceil = price.ceil();
        price = if (price - ceil).abs() < 0.001 {
            price.round()
        } else {
            ceil
        } * a;
    }

    // Calculate the minimum increment based on the scale
    let increment = 10f64.powi(-(4 - scale));

    // Adjust the price based on whether it's a buy or sell order
    if is_buy_order {
        price + increment
    } else {
        price - increment
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_outbid_price_buy_order_small_scale() {
        let price = 0.0001; // Small scale price
        let expected = 0.0002; // Expected to increase by the minimum increment
        let result = outbid_price(price, true);
        assert!(
            (result - expected).abs() < 0.0001,
            "Failed to correctly outbid a small scale buy order: {result} != {expected}"
        );
    }

    #[test]
    fn test_outbid_price_buy_order_one_scale() {
        let price = 1.001; // Small scale price
        let expected = 1.002; // Expected to increase by the minimum increment
        let result = outbid_price(price, true);
        assert!(
            (result - expected).abs() < 0.0001,
            "Failed to correctly outbid a one scale buy order: {result} != {expected}"
        );
    }

    #[test]
    fn test_outbid_price_sell_order_one_scale() {
        let price = 1.001;
        let expected = 1.000;
        let result = outbid_price(price, false);
        assert!(
            (result - expected).abs() < 0.0001,
            "Failed to correctly outbid a one scale sell order: {result} != {expected}"
        );
    }

    #[test]
    fn test_outbid_price_sell_order_small_scale() {
        let price = 0.0002; // Small scale price
        let expected = 0.0001; // Expected to decrease by the minimum increment
        let result = outbid_price(price, false);
        assert!(
            (result - expected).abs() < 0.0001,
            "Failed to correctly outbid a small scale sell order: {result} != {expected}"
        );
    }

    #[test]
    fn test_outbid_price_buy_order_large_scale() {
        let price = 10000.0; // Large scale price
        let expected = 10010.0; // Expected to increase by the minimum increment at this scale
        let result = outbid_price(price, true);
        assert!(
            (result - expected).abs() < 0.0001,
            "Failed to correctly outbid a large scale buy order: {result} != {expected}"
        );
    }

    #[test]
    fn test_outbid_price_sell_order_large_scale() {
        let price = 10010.0; // Large scale price
        let expected = 10000.0; // Expected to decrease by the minimum increment at this scale
        let result = outbid_price(price, false);
        assert!(
            (result - expected).abs() < 0.0001,
            "Failed to correctly outbid a large scale sell order: {result} != {expected}"
        );
    }

    #[test]
    fn test_outbid_price_edge_case() {
        let price = 9999.0; // Edge case near a significant digit change
        let expected_buy = 10000.0; // Expected to increase and round up for a buy order
        let result_buy = outbid_price(price, true);
        assert!(
            (result_buy - expected_buy).abs() < 0.0001,
            "Failed at edge case for buy order: {result_buy} != {expected_buy}"
        );

        let expected_sell = 9998.0; // Expected to decrease and round down for a sell order
        let result_sell = outbid_price(price, false);
        assert!(
            (result_sell - expected_sell).abs() < 0.0001,
            "Failed at edge case for sell order: {result_sell} != {expected_sell}"
        );
    }

    #[test]
    fn test_outbid_price_significant_digits() {
        let price = 1234.5678; // Test price with more than 4 significant digits
        let expected_buy = 1235.0; // Expected to round to 4 significant digits for a buy order
        let result_buy = outbid_price(price, true);
        assert!(
            (result_buy - expected_buy).abs() < 0.0001,
            "Failed to maintain 4 significant digits for buy order: {result_buy} != {expected_buy}"
        );

        let expected_sell = 1234.0; // Expected to round to 4 significant digits for a sell order
        let result_sell = outbid_price(price, false);
        assert!(
            (result_sell - expected_sell).abs() < 0.0001,
            "Failed to maintain 4 significant digits for sell order: {result_sell} != {expected_sell}"
        );
    }
}
