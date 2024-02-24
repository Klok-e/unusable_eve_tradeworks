use clap::{Arg, ArgAction, ArgMatches, Command};

use crate::consts::ITEM_NAME_LEN;

pub const CONFIG: &str = "config";
pub const SELL_SELL: &str = "sell-sell";
pub const SELL_BUY: &str = "sell-buy";
pub const REPROCESS: &str = "reprocess";
pub const ITEMS_PRICES: &str = "items-prices";
pub const STATION_TRADING: &str = "station-trading";
pub const DISPLAY_SIMPLE_LIST: &str = "simple-list";
pub const DISPLAY_SIMPLE_LIST_PRICE: &str = "simple-list-price";
pub const DEBUG_ITEM_ID: &str = "debug-item";
pub const FORCE_NO_REFRESH: &str = "force-no-refresh";
pub const NAME_LENGTH: &str = "name-length";
pub const QUIET: &str = "quiet";
pub const SOURCE_NAME: &str = "source-name";
pub const DEST_NAME: &str = "destination-name";

pub fn matches() -> ArgMatches {
    Command::new("Eve Tradeworks")
        .arg(Arg::new(SOURCE_NAME).required(true))
        .arg(Arg::new(DEST_NAME).required(false))
        .arg(
            Arg::new(SELL_SELL)
                .short('s')
                .long("sell-sell")
                .action(ArgAction::SetTrue)
                .conflicts_with_all([SELL_BUY, REPROCESS, ITEMS_PRICES]),
        )
        .arg(
            Arg::new(SELL_BUY)
                .short('b')
                .long("sell-buy")
                .action(ArgAction::SetTrue)
                .conflicts_with_all([SELL_SELL, REPROCESS, ITEMS_PRICES]),
        )
        .arg(
            Arg::new(REPROCESS)
                .short('e')
                .long("reprocess")
                .action(ArgAction::SetTrue)
                .conflicts_with_all([SELL_SELL, SELL_BUY, ITEMS_PRICES]),
        )
        .arg(
            Arg::new(ITEMS_PRICES)
                .short('i')
                .long("items-prices")
                .action(ArgAction::SetTrue)
                .conflicts_with_all([SELL_SELL, SELL_BUY, REPROCESS]),
        )
        .arg(
            Arg::new(STATION_TRADING)
                .short('t')
                .long("station-trading")
                .action(ArgAction::SetTrue)
                .conflicts_with_all([SELL_SELL, SELL_BUY, REPROCESS]),
        )
        .arg(
            Arg::new(DISPLAY_SIMPLE_LIST)
                .short('l')
                .long("simple-list")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(DISPLAY_SIMPLE_LIST_PRICE)
                .short('p')
                .long("simple-list-price")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(NAME_LENGTH)
                .short('n')
                .long("name-length")
                .default_value(ITEM_NAME_LEN),
        )
        .arg(Arg::new(DEBUG_ITEM_ID).long("debug-item").num_args(1))
        .arg(
            Arg::new(FORCE_NO_REFRESH)
                .long("force-no-refresh")
                .action(ArgAction::SetTrue),
        )
        .arg(Arg::new(QUIET).short('q').action(ArgAction::SetTrue))
        .get_matches()
}
