use std::{
    fs::remove_file,
    io::{self, Read, Write},
    net::Shutdown,
    path::Path,
};

use anyhow::{anyhow, Context};
use chrono::Duration;
use rand::Rng;

use copypasta::{ClipboardContext, ClipboardProvider};
use interprocess::os::unix::udsocket::{UdStream, UdStreamListener};
use itertools::Itertools;
use oauth2::TokenResponse;
use rust_eveonline_esi::apis::configuration::Configuration;

use term_table::{row::Row, table_cell::TableCell, TableBuilder, TableStyle};

use crate::{
    consts::UD_SOCKET_PATH,
    good_items::{items_prices::ItemInput, station_trading::StationTradeData},
};

pub fn communicate_paste_into_game(items: &StationTradeData) -> anyhow::Result<()> {
    log::info!("Listening for hotkeys...");

    let path = &Path::new(UD_SOCKET_PATH);
    if path.exists() {
        remove_file(path)?;
    }

    let listener = UdStreamListener::bind(UD_SOCKET_PATH)?;

    let mut buy_orders_data = items.get_buy_order_data();
    let mut current_buy_order = buy_orders_data.next();
    let mut order_clicks = 0;
    for mut conn in listener.incoming().filter_map(handle_error) {
        match order_clicks {
            0 => click_cmd()?,
            1 => click_cmd()?,
            _ => return Err(anyhow!("Click count broken: {order_clicks} clicks counted")),
        };

        order_clicks += 1;
        conn.shutdown(Shutdown::Write)?;
    }
    Ok(())
}

pub fn communicate_paste_sell_order_prices(prices: Vec<f64>) -> anyhow::Result<()> {
    log::info!("Listening for hotkeys...");

    let path = &Path::new(UD_SOCKET_PATH);
    if path.exists() {
        remove_file(path)?;
    }

    let listener = UdStreamListener::bind(UD_SOCKET_PATH)?;
    for (conn, price) in listener.incoming().filter_map(handle_error).zip(prices) {
        log::info!("Pasting {price}");
        paste_string_cmd(&format!("{}", price))?;
        conn.shutdown(Shutdown::Write)?;
    }

    log::info!("Finished listening");

    Ok(())
}

pub fn paste_string_cmd(line: &str) -> anyhow::Result<()> {
    let mut rng = rand::thread_rng();
    let rnd1: i64 = rng.gen_range(40..80);
    let rnd2: i64 = rng.gen_range(40..80);
    let rnd3: i64 = rng.gen_range(40..80);
    let rnd4: i64 = rng.gen_range(40..80);
    let rnd5: i64 = rng.gen_range(40..80);
    let rnd6: i64 = rng.gen_range(40..80);
    let rnd7: i64 = rng.gen_range(40..80);
    log::debug!("wl-copy '{line}'");
    cmd_lib::spawn! {
        wl-copy "${line}"
    }?;

    log::debug!("ydotool click --next-delay {rnd1} 0xC0");
    cmd_lib::run_cmd!(
        ydotool click --next-delay ${rnd1} 0xC0
    )?;

    log::debug!("ydotool key -d {rnd2} 29:1");
    cmd_lib::run_cmd!(
        ydotool key -d ${rnd2} 29:1 // Keycode for Ctrl key down
    )?;

    log::debug!("ydotool key -d {rnd3} 30:1 -d {rnd6} 30:0");
    cmd_lib::run_cmd!(
        ydotool key -d ${rnd3} 30:1 -d ${rnd6} 30:0 // Keycode for 'a' key
    )?;

    log::debug!("ydotool key -d {rnd4} 47:1 -d {rnd7} 47:0");
    cmd_lib::run_cmd!(
        ydotool key -d ${rnd4} 47:1 -d ${rnd7} 47:0 // Keycode for 'v' key
    )?;

    log::debug!("ydotool key -d {rnd5} 29:0");
    cmd_lib::run_cmd!(
        ydotool key -d ${rnd5} 29:0 // Keycode for Ctrl key up
    )?;

    Ok(())
}

pub fn click_cmd() -> anyhow::Result<()> {
    let mut rng = rand::thread_rng();
    let rnd1: i64 = rng.gen_range(40..80);
    cmd_lib::run_cmd!(
        ydotool click -d ${rnd1} 0xC0;
    )?;
    Ok(())
}

fn handle_error(result: io::Result<UdStream>) -> Option<UdStream> {
    match result {
        Ok(val) => Some(val),
        Err(error) => {
            eprintln!("There was an error with an incoming connection: {}", error);
            None
        }
    }
}
pub fn parse_items_from_clipboard() -> Result<Vec<ItemInput>, anyhow::Error> {
    log::info!("Copying items from clipboard...");
    let mut ctx = ClipboardContext::new().unwrap();
    let content = ctx.get_contents().unwrap();
    log::debug!("Clipboard content: {content}");

    if content.trim().is_empty() {
        return Err(anyhow!(
            "Clipboard is empty! Fill it with item names and item amounts."
        ));
    }

    let parsed_items = content
        .lines()
        .map(|line| {
            let mut split = line.split(char::is_whitespace).collect_vec();
            let pop = split.pop();
            let amount: i32 = pop
                .ok_or_else(|| anyhow!("Incorrect format: {line}"))?
                .parse()
                .with_context(|| format!("Couldn't parse item amount: {pop:?}"))?;
            let name = split.join(" ");
            Ok(ItemInput { name, amount })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    log::debug!("Parsed: {parsed_items:?}");
    Ok(parsed_items)
}
