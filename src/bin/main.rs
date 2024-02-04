use std::io::Read;

use anyhow::anyhow;
use chrono::Duration;

use itertools::Itertools;
use oauth2::TokenResponse;
use rust_eveonline_esi::apis::configuration::Configuration;

use term_table::{row::Row, table_cell::TableCell, TableBuilder, TableStyle};

use unusable_eve_tradeworks_lib::{
    auth::Auth,
    cached_data::CachedStuff,
    cli::{self, DEST_NAME, SOURCE_NAME},
    config::{AuthConfig, CommonConfig, Config, RouteConfig},
    consts::{self, CACHE_AUTH, CACHE_DATADUMP, CONFIG_COMMON},
    datadump_service::DatadumpService,
    good_items::{
        items_prices::ItemsPricesService,
        sell_reprocess::{get_good_items_sell_reprocess, make_table_sell_reprocess},
        station_trading::StationTradingService,
    },
    item_type::SystemMarketsItemData,
    items_list::{compute_pairs, compute_sell_buy, compute_sell_sell, SimpleDisplay},
    logger,
    requests::{
        item_history::ItemHistoryEsiService, service::EsiRequestsService,
        transactions::WalletEsiService,
    },
    system_interaction::{
        communicate_paste_into_game, communicate_paste_sell_order_prices,
        parse_items_from_clipboard,
    },
    Station,
};

#[tokio::main]
async fn main() {
    let result = run().await;
    if let Err(ref err) = result {
        log::error!("ERROR: {}", err);
        err.chain()
            .skip(1)
            .for_each(|cause| log::error!("because: {}", cause));
        std::process::exit(1)
    }
}

async fn run() -> Result<(), anyhow::Error> {
    std::fs::create_dir_all("cache/")?;

    let cli_args = cli::matches();

    let quiet = cli_args.get_flag(cli::QUIET);
    let file_loud = cli_args.get_flag(cli::FILE_LOUD);
    logger::setup_logger(quiet, file_loud, true)?;

    let mut cache = CachedStuff::new();

    let program_config = AuthConfig::from_file("auth.json");
    let auth = Auth::load_or_request_token(&program_config, &mut cache, CACHE_AUTH).await;

    let mut esi_config = Configuration {
        client: reqwest::ClientBuilder::new()
            .gzip(true)
            .user_agent("Your Ozuwara (evemail)")
            .build()
            .unwrap(),
        ..Default::default()
    };
    esi_config.oauth_access_token = Some(auth.token.access_token().secret().clone());

    let esi_requests = EsiRequestsService::new(&esi_config);

    let path_to_datadump = cache
        .load_or_create_json_async(
            CACHE_DATADUMP,
            vec![],
            Some(Duration::days(14)),
            |_| async {
                let client = &esi_config.client;
                let res = client
                    .get("https://www.fuzzwork.co.uk/dump/sqlite-latest.sqlite.bz2")
                    .send()
                    .await?;
                let bytes = res.bytes().await?.to_vec();

                // decompress
                let mut decompressor = bzip2::read::BzDecoder::new(bytes.as_slice());
                let mut contents = Vec::new();
                decompressor.read_to_end(&mut contents).unwrap();

                let path = "cache/datadump.db".to_string();
                std::fs::write(&path, contents)?;
                Ok(path)
            },
        )
        .await?;

    let db = rusqlite::Connection::open_with_flags(
        path_to_datadump,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    let data_service = DatadumpService::new(db);

    let force_no_refresh = cli_args.get_flag(cli::FORCE_NO_REFRESH);

    let esi_history = ItemHistoryEsiService::new(&esi_config);

    let config_common = CommonConfig::from_file_json(CONFIG_COMMON)?;

    let sell_sell = cli_args.get_flag(cli::SELL_SELL);
    let sell_buy = cli_args.get_flag(cli::SELL_BUY);
    let reprocess_flag = cli_args.get_flag(cli::REPROCESS);
    if sell_sell || reprocess_flag || sell_buy {
        let source = cli_args.get_one::<String>(SOURCE_NAME).unwrap();
        let dest = cli_args.get_one::<String>(DEST_NAME).unwrap();
        let config = Config {
            route: RouteConfig {
                source: find_station(&config_common.stations, source)?,
                destination: find_station(&config_common.stations, dest)?,
            },
            common: config_common,
        };

        log::info!(
            "Calculating route {} ---> {}",
            config.route.source.name,
            config.route.destination.name
        );

        print_buy_tables(
            config,
            &cli_args,
            esi_requests,
            esi_history,
            auth,
            cache,
            data_service,
            &esi_config,
            sell_sell,
            force_no_refresh,
        )
        .await?;
    } else if cli_args.get_flag(cli::ITEMS_PRICES) {
        log::debug!("Items prices");
        let wallet_service = WalletEsiService {
            esi_config: &esi_config,
        };

        let mut items_prices_service = ItemsPricesService {
            wallet_esi_service: &wallet_service,
            cache: &mut cache,
            esi_requests: &esi_requests,
            esi_history: &esi_history,
            config: &config_common,
        };

        let source = cli_args.get_one::<String>(SOURCE_NAME).unwrap();
        let station = find_station(&config_common.stations, source)?;

        let parsed_items = parse_items_from_clipboard()?;

        let prices = items_prices_service
            .get_prices_for_items(auth.get_character_id(), parsed_items, station)
            .await?;

        // join reverted prices because order of items in multi sell are reversed
        let prices = prices.iter().rev().map(|x| x.price).collect();
        communicate_paste_sell_order_prices(prices)?;
    } else if cli_args.get_flag(cli::STATION_TRADING) {
        log::debug!("Station trading");
        let mut items_prices_service = StationTradingService {
            cache: &mut cache,
            esi_requests: &esi_requests,
            esi_history: &esi_history,
            config: &config_common,
        };

        let source = cli_args.get_one::<String>(SOURCE_NAME).unwrap();
        let station = find_station(&config_common.stations, source)?;
        let items = items_prices_service
            .get_prices_for_items(station, auth.get_character_id(), get_debug_item(&cli_args))
            .await?;

        let rows = items.make_table_station_trade(get_name_len(&cli_args));
        let table = TableBuilder::new().rows(rows).build();
        println!("{}", table.render());

        communicate_paste_into_game(&esi_requests, &items).await?;
    }

    Ok(())
}

async fn print_buy_tables(
    config: Config,
    cli_args: &clap::ArgMatches,
    esi_requests: EsiRequestsService<'_>,
    esi_history: ItemHistoryEsiService<'_>,
    auth: Auth,
    mut cache: CachedStuff,
    data_service: DatadumpService,
    esi_config: &Configuration,
    sell_sell: bool,
    force_no_refresh: bool,
) -> Result<(), anyhow::Error> {
    let mut pairs: Vec<SystemMarketsItemData> = compute_pairs(
        &config,
        &esi_requests,
        &esi_history,
        auth.get_character_id(),
        &mut cache,
        &data_service,
    )
    .await?;
    let reprocess_flag = cli_args.get_flag(cli::REPROCESS);
    let mut disable_filters = false;
    if let Some(v) = get_debug_item(cli_args) {
        let reprocess = if reprocess_flag {
            data_service
                .get_reprocess_items(v)?
                .reprocessed_into
                .into_iter()
                .map(|x| x.item_id)
                .collect::<Vec<_>>()
        } else {
            vec![]
        };

        pairs.retain(|x| x.desc.type_id == v || reprocess.contains(&x.desc.type_id));
        disable_filters = true;
    }
    let esi_config = &esi_config;
    let mut simple_list: Vec<_> = Vec::new();
    let rows = {
        let name_len = get_name_len(cli_args);

        if sell_sell {
            log::debug!("Sell sell path.");
            compute_sell_sell(
                pairs,
                &config,
                disable_filters,
                &mut simple_list,
                name_len,
                cache,
                force_no_refresh,
                esi_requests,
                esi_config,
                data_service,
            )
            .await?
        } else if reprocess_flag {
            compute_reprocess_rows(
                cli_args,
                pairs,
                &config,
                disable_filters,
                data_service,
                &mut simple_list,
                name_len,
            )?
        } else {
            log::debug!("Sell buy path.");
            compute_sell_buy(pairs, &config, disable_filters, &mut simple_list, name_len)?
        }
    };
    let table = TableBuilder::new().rows(rows).build();
    println!("{}", table.render());
    if cli_args.get_flag(cli::DISPLAY_SIMPLE_LIST) {
        print_simple_list(&simple_list);
    }
    if cli_args.get_flag(cli::DISPLAY_SIMPLE_LIST_PRICE) {
        print_simple_list_with_price(simple_list);
    };
    Ok(())
}

fn get_debug_item(cli_args: &clap::ArgMatches) -> Option<i32> {
    cli_args
        .get_one::<String>(cli::DEBUG_ITEM_ID)
        .and_then(|x| x.parse::<i32>().ok())
}

fn get_name_len(cli_args: &clap::ArgMatches) -> usize {
    let cli_in = cli_args.get_one::<String>(cli::NAME_LENGTH);

    if let Some(v) = cli_in.and_then(|x| x.parse::<usize>().ok()) {
        v
    } else {
        log::warn!(
            "Value '{:?}' can't be parsed as an int. Using '{}'",
            cli_in,
            consts::ITEM_NAME_LEN
        );
        consts::ITEM_NAME_LEN.parse().unwrap()
    }
}

fn compute_reprocess_rows<'b>(
    cli_args: &clap::ArgMatches,
    pairs: Vec<SystemMarketsItemData>,
    config: &Config,
    disable_filters: bool,
    data_service: DatadumpService,
    simple_list: &mut Vec<SimpleDisplay>,
    name_len: usize,
) -> Result<Vec<Row<'b>>, anyhow::Error> {
    log::debug!("Reprocess path.");
    let pairs_clone = if let Some(v) = cli_args
        .get_one::<String>(cli::DEBUG_ITEM_ID)
        .and_then(|x| x.parse::<i32>().ok())
    {
        let mut clone = pairs.clone();
        clone.retain(|x| x.desc.type_id == v);
        clone
    } else {
        pairs.clone()
    };
    let good_items =
        get_good_items_sell_reprocess(pairs_clone, pairs, config, disable_filters, &data_service)?;
    *simple_list = good_items
        .items
        .iter()
        .map(|x| SimpleDisplay {
            name: x.market.desc.name.clone(),
            recommend_buy: x.recommend_buy,
            sell_price: x.dest_min_sell_price,
        })
        .collect();
    Ok(make_table_sell_reprocess(&good_items, name_len))
}

fn print_simple_list_with_price(simple_list: Vec<SimpleDisplay>) {
    let rows = simple_list
        .iter()
        .enumerate()
        .flat_map(|(i, it)| {
            let mut rows = Vec::new();
            if i % 100 == 99 {
                rows.push(Row::new(vec![TableCell::new_with_col_span("---", 3)]));
            }
            rows.push(Row::new(vec![
                TableCell::new(it.name.clone()),
                TableCell::new(it.recommend_buy),
                TableCell::new(format!("{:.2}", it.sell_price)),
            ]));
            rows.into_iter()
        })
        .collect::<Vec<_>>();

    let table = TableBuilder::new()
        .style(TableStyle::empty())
        .separate_rows(false)
        .has_bottom_boarder(false)
        .has_top_boarder(false)
        .rows(rows)
        .build();
    println!("Item sell prices:\n{}", table.render());
}

fn print_simple_list(simple_list: &[SimpleDisplay]) {
    let rows = simple_list
        .iter()
        .enumerate()
        .flat_map(|(i, it)| {
            let mut rows = Vec::new();
            if i % 100 == 99 {
                rows.push(Row::new(vec![TableCell::new_with_col_span("---", 2)]));
            }
            rows.push(Row::new(vec![
                TableCell::new(it.name.clone()),
                TableCell::new(it.recommend_buy),
            ]));
            rows.into_iter()
        })
        .collect::<Vec<_>>();

    let table = TableBuilder::new()
        .style(TableStyle::empty())
        .separate_rows(false)
        .has_bottom_boarder(false)
        .has_top_boarder(false)
        .rows(rows)
        .build();
    println!("Item names:\n{}", table.render());
}

fn find_station(stations: &[Station], source: &String) -> Result<Station, anyhow::Error> {
    Ok(stations
        .iter()
        .find(|x| x.short.as_ref().unwrap_or(&x.name) == source)
        .ok_or(anyhow!("Can't find {}", source))?
        .clone())
}
