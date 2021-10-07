pub mod sell_sell;
pub mod help;
pub mod sell_buy;
pub mod sell_sell_zkb;

use itertools::Itertools;
use ordered_float::NotNan;
use term_table::{row::Row, table_cell::TableCell};

use crate::{
    config::Config,
    item_type::{ItemHistoryDay, ItemTypeAveraged, Order, SystemMarketsItemData},
    order_ext::OrderIterExt,
    requests::to_not_nan,
    stat::AverageStat,
};

