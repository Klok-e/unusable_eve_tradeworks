use std::{fs::remove_file, net::Shutdown, path::Path};

use anyhow::{anyhow, Context};

use rand::Rng;

use copypasta::{ClipboardContext, ClipboardProvider};
use interprocess::os::unix::udsocket::UdStreamListener;
use itertools::Itertools;

use crate::{
    consts::UD_SOCKET_PATH,
    good_items::{items_prices::ItemInput, station_trading::StationTradeData},
    requests::service::EsiRequestsService,
};

pub fn send_notification(notification: &str) -> anyhow::Result<()> {
    log::debug!("Sending notification {notification}");
    cmd_lib::spawn! {
        notify-send "Unusable Eve Tradeworks" ${notification}
    }?;
    Ok(())
}

pub async fn communicate_paste_into_game<'a>(
    esi_requests: &EsiRequestsService<'a>,
    items: &StationTradeData,
) -> anyhow::Result<()> {
    log::info!("Listening for hotkeys...");
    send_notification("Listening to hotkeys")?;

    let path = &Path::new(UD_SOCKET_PATH);
    if path.exists() {
        remove_file(path)?;
    }

    let listener = UdStreamListener::bind(UD_SOCKET_PATH)?;

    for item in items.get_buy_order_data() {
        esi_requests.open_market_type(item.type_id).await?;

        let conn = listener.accept()?;
        // user clicks "create buy order"
        click_cmd()?;
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
        save_to_clipboard_cmd(&format!("{}", item.item_price))?;
        paste_from_clipboard_cmd()?;

        tab_cmd()?;

        save_to_clipboard_cmd(&format!("{}", item.item_quantity))?;
        paste_from_clipboard_cmd()?;
        conn.shutdown(Shutdown::Write)?;

        let conn = listener.accept()?;
        click_cmd()?;
        conn.shutdown(Shutdown::Write)?;
    }
    Ok(())
}

pub fn communicate_paste_sell_order_prices(prices: Vec<f64>) -> anyhow::Result<()> {
    log::info!("Listening for hotkeys...");
    send_notification("Listening to hotkeys")?;

    let path = &Path::new(UD_SOCKET_PATH);
    if path.exists() {
        remove_file(path)?;
    }

    let listener = UdStreamListener::bind(UD_SOCKET_PATH)?;
    for price in prices {
        let conn = listener.accept()?;

        log::info!("Pasting {price}");

        save_to_clipboard_cmd(&format!("{}", price))?;
        // click_cmd()?;
        paste_from_clipboard_cmd()?;
        tab_cmd()?;
        tab_cmd()?;

        conn.shutdown(Shutdown::Write)?;
    }

    log::info!("Finished listening");

    Ok(())
}

pub fn save_to_clipboard_cmd(line: &str) -> anyhow::Result<()> {
    log::info!("Copying '{line}' to clipboard...");
    cmd_lib::spawn! {
        wl-copy "${line}"
    }?;
    Ok(())
}

pub fn paste_from_clipboard_cmd() -> anyhow::Result<()> {
    log::info!("Pasting...");

    let mut rng = rand::thread_rng();
    let rnd2: i64 = rng.gen_range(50..80);
    let rnd3: i64 = rng.gen_range(50..80);
    let rnd4: i64 = rng.gen_range(50..80);
    let rnd5: i64 = rng.gen_range(50..80);
    let rnd6: i64 = rng.gen_range(50..80);
    let rnd7: i64 = rng.gen_range(50..80);
    cmd_lib::run_cmd!(
        ydotool key -d ${rnd2} 29:1 // Keycode for Ctrl key down
    )?;
    cmd_lib::run_cmd!(
        ydotool key -d ${rnd3} 30:1 -d ${rnd6} 30:0 // Keycode for 'a' key
    )?;
    cmd_lib::run_cmd!(
        ydotool key -d ${rnd4} 47:1 -d ${rnd7} 47:0 // Keycode for 'v' key
    )?;
    cmd_lib::run_cmd!(
        ydotool key -d ${rnd5} 29:0 // Keycode for Ctrl key up
    )?;

    Ok(())
}

pub fn click_cmd() -> anyhow::Result<()> {
    log::info!("Clicking...");

    let mut rng = rand::thread_rng();
    let rnd1: i64 = rng.gen_range(40..80);
    cmd_lib::run_cmd!(
        ydotool click --next-delay ${rnd1} 0xC0;
    )?;
    Ok(())
}

pub fn tab_cmd() -> anyhow::Result<()> {
    log::info!("Pressing tab...");

    let mut rng = rand::thread_rng();
    let rnd1: i64 = rng.gen_range(40..80);
    let rnd2: i64 = rng.gen_range(40..80);
    cmd_lib::run_cmd!(
        ydotool key -d ${rnd1} 15:1 -d ${rnd2} 15:0;
    )?;
    Ok(())
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
