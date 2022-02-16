use clap::{App, Arg, ArgMatches};

use crate::consts::ITEM_NAME_LEN;

pub const CONFIG: &str = "config";
pub const SELL_SELL: &str = "sell-sell";
pub const SELL_SELL_ZKB: &str = "sell-sell-zkb";
pub const SELL_BUY: &str = "sell-buy";
pub const DISPLAY_SIMPLE_LIST: &str = "simple-list";
pub const DISPLAY_SIMPLE_LIST_PRICE: &str = "simple-list-price";
pub const DEBUG_ITEM_ID: &str = "debug-item";
pub const FORCE_REFRESH: &str = "force-refresh";
pub const FORCE_NO_REFRESH: &str = "force-no-refresh";
pub const NAME_LENGTH: &str = "name-length";
pub const QUIET: &str = "quiet";

pub fn matches() -> ArgMatches<'static> {
    let matches = App::new("Eve Tradeworks")
        .arg(
            Arg::with_name(CONFIG)
                .short("c")
                .long("config")
                .takes_value(true),
        )
        .arg(
            Arg::with_name(SELL_SELL)
                .short("s")
                .long("sell-sell")
                .takes_value(false)
                .conflicts_with_all(&[SELL_BUY, SELL_SELL_ZKB]),
        )
        .arg(
            Arg::with_name(SELL_SELL_ZKB)
                .short("z")
                .long("sell-sell-zkb")
                .takes_value(false)
                .conflicts_with_all(&[SELL_BUY, SELL_SELL]),
        )
        .arg(
            Arg::with_name(SELL_BUY)
                .short("b")
                .long("sell-buy")
                .takes_value(false)
                .conflicts_with_all(&[SELL_SELL, SELL_SELL_ZKB]),
        )
        .arg(
            Arg::with_name(DISPLAY_SIMPLE_LIST)
                .short("l")
                .long("simple-list")
                .takes_value(false),
        )
        .arg(
            Arg::with_name(DISPLAY_SIMPLE_LIST_PRICE)
                .short("p")
                .long("simple-list-price")
                .takes_value(false),
        )
        .arg(
            Arg::with_name(NAME_LENGTH)
                .short("n")
                .long("name-length")
                .default_value(ITEM_NAME_LEN),
        )
        .arg(
            Arg::with_name(DEBUG_ITEM_ID)
                .long("debug-item")
                .takes_value(true),
        )
        .arg(
            Arg::with_name(FORCE_REFRESH)
                .short("r")
                .long("force-refresh")
                .takes_value(false)
                .conflicts_with(FORCE_NO_REFRESH),
        )
        .arg(
            Arg::with_name(FORCE_NO_REFRESH)
                .long("force-no-refresh")
                .takes_value(false)
                .conflicts_with(FORCE_REFRESH),
        )
        .arg(Arg::with_name(QUIET).short("q").takes_value(false))
        .get_matches();
    matches
}
