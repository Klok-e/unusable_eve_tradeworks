use good_lp::SolverModel;
use itertools::Itertools;
use ordered_float::NotNan;
use term_table::{row::Row, table_cell::TableCell};

use crate::{
    config::Config,
    item_type::{ItemTypeAveraged, SystemMarketsItemData},
    order_ext::OrderIterExt,
};

use super::help::averages;
pub fn get_good_items_sell_buy(
    pairs: Vec<SystemMarketsItemData>,
    config: &Config,
    disable_filters: bool,
) -> ProcessedSellBuyItems {
    pairs
        .into_iter()
        .filter_map(|x| {
            let src_mkt_orders = x.source.orders.clone();
            let src_mkt_volume = src_mkt_orders.iter().sell_order_volume();

            let dst_mkt_orders = x.destination.orders.clone();
            let dst_mkt_volume: i32 = dst_mkt_orders.iter().sell_order_volume();

            let src_avgs = averages(config, &x.source.history);
            let dst_avgs = averages(config, &x.destination.history);

            let (recommend_buy_vol, dest_sell_price, max_buy_price, avg_buy_price) = {
                let mut source_sell_orders = x
                    .source
                    .orders
                    .iter()
                    .cloned()
                    .filter(|x| !x.is_buy_order)
                    .sorted_by_key(|x| NotNan::new(x.price).unwrap());

                let mut curr_src_sell_order = source_sell_orders.next()?;

                let mut recommend_bought_volume = 0;
                let mut sum_sell_price = 0.;
                let mut max_buy_price = 0.;
                let mut sum_buy_price = 0.;
                'outer: for buy_order in x
                    .destination
                    .orders
                    .iter()
                    .filter(|x| x.is_buy_order)
                    .sorted_by_key(|x| NotNan::new(-x.price).unwrap())
                {
                    let mut buy_order_fulfilled = buy_order.volume_remain;
                    while buy_order_fulfilled > 0 {
                        let bought_volume =
                            buy_order_fulfilled.min(curr_src_sell_order.volume_remain);
                        buy_order_fulfilled -= bought_volume;

                        let expenses = (curr_src_sell_order.price
                            * (1. + config.broker_fee_source))
                            * bought_volume as f64;

                        let sell_price =
                            bought_volume as f64 * buy_order.price * (1. - config.sales_tax);

                        if expenses >= sell_price {
                            break;
                        }
                        sum_buy_price += curr_src_sell_order.price * bought_volume as f64;
                        curr_src_sell_order.volume_remain -= bought_volume;
                        max_buy_price = curr_src_sell_order.price.max(max_buy_price);
                        sum_sell_price += buy_order.price * bought_volume as f64;
                        recommend_bought_volume += bought_volume;

                        if curr_src_sell_order.volume_remain == 0 {
                            curr_src_sell_order = if let Some(x) = source_sell_orders.next() {
                                x
                            } else {
                                break 'outer;
                            }
                        }
                    }
                }

                (
                    recommend_bought_volume,
                    sum_sell_price / recommend_bought_volume as f64,
                    max_buy_price,
                    sum_buy_price / recommend_bought_volume as f64,
                )
            };

            // multibuy can only buy at a fixed price, so all buys from multiple sell orders
            // with different prices have you paid the same price for all of them
            let expenses = max_buy_price;
            let buy_with_broker_fee = expenses * (1. + config.broker_fee_source);
            let fin_sell_price = dest_sell_price * (1. - config.sales_tax);

            let margin = (fin_sell_price - buy_with_broker_fee) / buy_with_broker_fee;

            let rough_profit = (fin_sell_price - buy_with_broker_fee) * recommend_buy_vol as f64;

            // also calculate avg buy price
            let best_expenses = avg_buy_price;
            let buy_with_broker_fee = best_expenses * (1. + config.broker_fee_source);
            let fin_sell_price = dest_sell_price * (1. - config.sales_tax);

            let best_margin = (fin_sell_price - buy_with_broker_fee) / buy_with_broker_fee;

            Some(PairCalculatedDataSellBuy {
                market: x,
                margin,
                best_margin,
                rough_profit,
                market_dest_volume: dst_mkt_volume,
                recommend_buy: recommend_buy_vol,
                expenses: buy_with_broker_fee,
                sell_price: fin_sell_price,
                src_buy_price: expenses,
                dest_min_sell_price: dest_sell_price,
                market_src_volume: src_mkt_volume,
                src_avgs,
                dst_avgs,
            })
        })
        .filter(|x| disable_filters || x.best_margin > config.margin_cutoff)
        .filter(|x| {
            disable_filters
                || config
                    .min_profit
                    .map_or(true, |min_prft| x.rough_profit > min_prft)
        })
        .sorted_unstable_by_key(|x| NotNan::new(-x.rough_profit).unwrap())
        .collect::<Vec<_>>()
        .take_maximizing_profit(config.sell_buy.cargo_capacity)
}

trait DataVecExt {
    fn take_maximizing_profit(self, max_cargo: i32) -> ProcessedSellBuyItems;
}

impl DataVecExt for Vec<PairCalculatedDataSellBuy> {
    fn take_maximizing_profit(self, max_cargo: i32) -> ProcessedSellBuyItems {
        use good_lp::{default_solver, variable, Expression, ProblemVariables, Solution, Variable};
        let mut vars = ProblemVariables::new();
        let mut var_refs = Vec::new();
        for item in &self {
            let var_def = variable().integer().min(0).max(item.recommend_buy);
            var_refs.push(vars.add(var_def));
        }

        let goal = var_refs
            .iter()
            .zip(self.iter())
            .map(
                |(&var, item): (&Variable, &PairCalculatedDataSellBuy)| -> Expression {
                    (item.sell_price - item.expenses) * var
                },
            )
            .sum::<Expression>();

        let space_constraint = var_refs
            .iter()
            .zip(self.iter())
            .map(
                |(&var, item): (&Variable, &PairCalculatedDataSellBuy)| -> Expression {
                    (item.market.desc.volume as f64) * var
                },
            )
            .sum::<Expression>()
            .leq(max_cargo);

        let solution = vars
            .maximise(&goal)
            .using(default_solver)
            .with(space_constraint)
            .solve()
            .unwrap();

        let recommended_items = var_refs.into_iter().zip(self.into_iter()).map(
            |(var, item): (Variable, PairCalculatedDataSellBuy)| -> PairCalculatedDataSellBuyFinal {
                let optimal = solution.value(var);
                let recommend_buy = optimal as i32;
                let volume = (recommend_buy as f64 * item.market.desc.volume as f64) as i32;
                PairCalculatedDataSellBuyFinal{
                    market: item.market,
                    margin: item.margin,
                    rough_profit: item.rough_profit,
                    market_dest_volume: item.market_dest_volume,
                    recommend_buy,
                    expenses: item.expenses,
                    sell_price: item.sell_price,
                    src_buy_price: item.src_buy_price,
                    dest_min_sell_price: item.dest_min_sell_price,
                    src_avgs: item.src_avgs,
                    dst_avgs: item.dst_avgs,
                    market_src_volume: item.market_src_volume,
                    volume,
                }
            },
        )
        .filter(|x: &PairCalculatedDataSellBuyFinal| x.recommend_buy > 0)
        .collect::<Vec<_>>();

        let volume = recommended_items
            .iter()
            .map(|x| x.market.desc.volume as f64 * x.recommend_buy as f64)
            .sum::<f64>() as i32;
        ProcessedSellBuyItems {
            items: recommended_items,
            sum_profit: solution.eval(&goal),
            sum_volume: volume,
        }
    }
}

pub fn make_table_sell_buy<'a, 'b>(
    good_items: &'a ProcessedSellBuyItems,
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
        TableCell::new("rcmnd"),
        TableCell::new("vlm"),
    ]))
    .chain(good_items.items.iter().map(|it| {
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
            TableCell::new(format!(
                "{:.2}",
                it.dst_avgs.map(|x| x.volume).unwrap_or(0f64)
            )),
            TableCell::new(format!("{:.2}", it.market_src_volume)),
            TableCell::new(format!("{:.2}", it.market_dest_volume)),
            TableCell::new(format!("{:.2}", it.rough_profit)),
            TableCell::new(format!("{}", it.recommend_buy)),
            TableCell::new(format!("{}", it.volume)),
        ])
    }))
    .chain(std::iter::once(Row::new(vec![
        TableCell::new("total profit"),
        TableCell::new_with_col_span(format!("{}", good_items.sum_profit), 13),
    ])))
    .chain(std::iter::once(Row::new(vec![
        TableCell::new("total volume"),
        TableCell::new_with_col_span(format!("{}", good_items.sum_volume), 13),
    ])))
    .collect::<Vec<_>>();
    rows
}

#[derive(Debug, Clone)]
struct PairCalculatedDataSellBuy {
    market: SystemMarketsItemData,
    margin: f64,
    rough_profit: f64,
    market_dest_volume: i32,
    recommend_buy: i32,
    expenses: f64,
    sell_price: f64,
    src_buy_price: f64,
    dest_min_sell_price: f64,
    src_avgs: Option<ItemTypeAveraged>,
    dst_avgs: Option<ItemTypeAveraged>,
    market_src_volume: i32,
    best_margin: f64,
}

#[derive(Debug, Clone)]
pub struct PairCalculatedDataSellBuyFinal {
    pub market: SystemMarketsItemData,
    margin: f64,
    rough_profit: f64,
    market_dest_volume: i32,
    pub recommend_buy: i32,
    expenses: f64,
    sell_price: f64,
    src_buy_price: f64,
    pub dest_min_sell_price: f64,
    src_avgs: Option<ItemTypeAveraged>,
    dst_avgs: Option<ItemTypeAveraged>,
    market_src_volume: i32,
    volume: i32,
}

pub struct ProcessedSellBuyItems {
    pub items: Vec<PairCalculatedDataSellBuyFinal>,
    pub sum_profit: f64,
    pub sum_volume: i32,
}
