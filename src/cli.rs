use clap::{App, Arg, ArgMatches};

pub const CONFIG: &str = "config";
pub const SELL_SELL: &str = "sell-sell";
pub const SELL_BUY: &str = "sell-buy";
pub const DISPLAY_SIMPLE_LIST: &str = "simple-list";
pub const DEBUG_ITEM_ID: &str = "debug-item";

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
                .conflicts_with(SELL_BUY),
        )
        .arg(
            Arg::with_name(SELL_BUY)
                .short("b")
                .long("sell-buy")
                .takes_value(false)
                .conflicts_with(SELL_SELL),
        )
        .arg(
            Arg::with_name(DISPLAY_SIMPLE_LIST)
                .short("l")
                .long("simple-list")
                .takes_value(false),
        )
        .arg(
            Arg::with_name(DEBUG_ITEM_ID)
                .long("debug-item")
                .takes_value(true),
        )
        .get_matches();
    matches
}
