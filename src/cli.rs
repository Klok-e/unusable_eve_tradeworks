use clap::{Arg, ArgMatches, Command};

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
pub const FILE_LOUD: &str = "file-loud";

pub fn matches() -> ArgMatches {
    let matches = Command::new("Eve Tradeworks")
        .arg(Arg::new(CONFIG).short('c').long("config").takes_value(true))
        .arg(
            Arg::new(SELL_SELL)
                .short('s')
                .long("sell-sell")
                .takes_value(false)
                .conflicts_with_all(&[SELL_BUY, SELL_SELL_ZKB]),
        )
        .arg(
            Arg::new(SELL_SELL_ZKB)
                .short('z')
                .long("sell-sell-zkb")
                .takes_value(false)
                .conflicts_with_all(&[SELL_BUY, SELL_SELL]),
        )
        .arg(
            Arg::new(SELL_BUY)
                .short('b')
                .long("sell-buy")
                .takes_value(false)
                .conflicts_with_all(&[SELL_SELL, SELL_SELL_ZKB]),
        )
        .arg(
            Arg::new(DISPLAY_SIMPLE_LIST)
                .short('l')
                .long("simple-list")
                .takes_value(false),
        )
        .arg(
            Arg::new(DISPLAY_SIMPLE_LIST_PRICE)
                .short('p')
                .long("simple-list-price")
                .takes_value(false),
        )
        .arg(
            Arg::new(NAME_LENGTH)
                .short('n')
                .long("name-length")
                .default_value(ITEM_NAME_LEN),
        )
        .arg(Arg::new(DEBUG_ITEM_ID).long("debug-item").takes_value(true))
        .arg(
            Arg::new(FORCE_REFRESH)
                .short('r')
                .long("force-refresh")
                .takes_value(false)
                .conflicts_with(FORCE_NO_REFRESH),
        )
        .arg(
            Arg::new(FORCE_NO_REFRESH)
                .long("force-no-refresh")
                .takes_value(false)
                .conflicts_with(FORCE_REFRESH),
        )
        .arg(Arg::new(QUIET).short('q').takes_value(false))
        .arg(Arg::new(FILE_LOUD).short('v').takes_value(false))
        .get_matches();
    matches
}
