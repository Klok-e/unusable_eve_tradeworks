use std::{collections::HashMap, io::Read};

use anyhow::anyhow;
use chrono::Duration;
use futures::{stream, StreamExt};

use oauth2::TokenResponse;
use rust_eveonline_esi::apis::configuration::Configuration;

use term_table::{row::Row, table_cell::TableCell, TableBuilder, TableStyle};
use tokio::join;

use unusable_eve_tradeworks_lib::{
    auth::Auth,
    cached_data::CachedStuff,
    cli::{self, DEST_NAME, SOURCE_NAME},
    config::{AuthConfig, CommonConfig, Config, RouteConfig},
    consts::{self, BUFFER_UNORDERED},
    datadump_service::DatadumpService,
    error,
    good_items::{
        sell_buy::{get_good_items_sell_buy, make_table_sell_buy},
        sell_reprocess::{get_good_items_sell_reprocess, make_table_sell_reprocess},
        sell_sell::{get_good_items_sell_sell, make_table_sell_sell},
        sell_sell_zkb::{get_good_items_sell_sell_zkb, make_table_sell_sell_zkb},
    },
    item_type::{SystemMarketsItem, SystemMarketsItemData, TypeDescription},
    logger,
    requests::service::EsiRequestsService,
    zkb::{killmails::KillmailService, zkb_requests::ZkbRequestsService},
    Station,
};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    run().await
}

const CACHE_AUTH: &str = "cache/auth";
const CACHE_DATADUMP: &str = "cache/datadump.json";
const CACHE_ALL_TYPES: &&str = &"cache/all_types.json";
const CACHE_ALL_TYPE_DESC: &&str = &"cache/all_type_descriptions.rmp";
const CONFIG_COMMON: &&str = &"config.common.json";

async fn run() -> Result<(), anyhow::Error> {
    std::fs::create_dir_all("cache/")?;

    let cli_args = cli::matches();

    let quiet = cli_args.get_flag(cli::QUIET);
    let file_loud = cli_args.get_flag(cli::FILE_LOUD);
    logger::setup_logger(quiet, file_loud)?;

    let source = cli_args.get_one::<String>(SOURCE_NAME).unwrap();
    let dest = cli_args.get_one::<String>(DEST_NAME).unwrap();
    let common = CommonConfig::from_file_json(CONFIG_COMMON)?;
    let config = Config {
        route: RouteConfig {
            source: find_station(&common.stations, source)?,
            destination: find_station(&common.stations, dest)?,
        },
        common,
    };

    log::info!(
        "Calculating route {} ---> {}",
        config.route.source.name,
        config.route.destination.name
    );

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
            false,
            Some(Duration::days(14)),
            || async {
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

    let character_id = auth
        .character_info
        .sub
        .split(':')
        .nth(2)
        .unwrap()
        .parse()
        .unwrap();

    let force_refresh = cli_args.get_flag(cli::FORCE_REFRESH);
    let force_no_refresh = cli_args.get_flag(cli::FORCE_NO_REFRESH);

    let mut pairs: Vec<SystemMarketsItemData> = compute_pairs(
        &config,
        &esi_requests,
        character_id,
        &mut cache,
        force_refresh,
        &data_service,
    )
    .await?;

    let reprocess_flag = cli_args.get_flag(cli::REPROCESS);
    let mut disable_filters = false;
    if let Some(v) = cli_args
        .get_one::<String>(cli::DEBUG_ITEM_ID)
        .and_then(|x| x.parse::<i32>().ok())
    {
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

    let mut simple_list: Vec<_> = Vec::new();
    let rows = {
        let cli_in = cli_args.get_one::<String>(cli::NAME_LENGTH);
        let name_len = if let Some(v) = cli_in.and_then(|x| x.parse::<usize>().ok()) {
            v
        } else {
            log::warn!(
                "Value '{:?}' can't be parsed as an int. Using '{}'",
                cli_in,
                consts::ITEM_NAME_LEN
            );
            consts::ITEM_NAME_LEN.parse().unwrap()
        };

        let sell_sell = cli_args.get_flag(cli::SELL_SELL);
        let sell_sell_zkb = cli_args.get_flag(cli::SELL_SELL_ZKB);

        if sell_sell {
            log::trace!("Sell sell path.");
            compute_sell_sell(pairs, &config, disable_filters, &mut simple_list, name_len)
        } else if sell_sell_zkb {
            log::trace!("Sell sell zkb path.");
            compute_sell_sell_zkb(
                config,
                cache,
                force_no_refresh,
                esi_requests,
                &esi_config,
                pairs,
                disable_filters,
                &mut simple_list,
                name_len,
            )
            .await?
        } else if reprocess_flag {
            log::trace!("Reprocess path.");
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
            let good_items = get_good_items_sell_reprocess(
                pairs_clone,
                pairs,
                &config,
                disable_filters,
                &data_service,
            )?;
            simple_list = good_items
                .items
                .iter()
                .map(|x| SimpleDisplay {
                    name: x.market.desc.name.clone(),
                    recommend_buy: x.recommend_buy,
                    sell_price: x.dest_min_sell_price,
                })
                .collect();
            make_table_sell_reprocess(&good_items, name_len)
        } else {
            log::trace!("Sell buy path.");
            compute_sell_buy(pairs, &config, disable_filters, &mut simple_list, name_len)?
        }
    };

    let table = TableBuilder::new().rows(rows).build();
    println!("{}", table.render());

    if cli_args.get_flag(cli::DISPLAY_SIMPLE_LIST) {
        let rows = simple_list
            .iter()
            .map(|it| {
                Row::new(vec![
                    TableCell::new(it.name.clone()),
                    TableCell::new(it.recommend_buy),
                ])
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
    if cli_args.get_flag(cli::DISPLAY_SIMPLE_LIST_PRICE) {
        let rows = simple_list
            .iter()
            .map(|it| {
                Row::new(vec![
                    TableCell::new(it.name.clone()),
                    TableCell::new(it.recommend_buy),
                    TableCell::new(format!("{:.2}", it.sell_price)),
                ])
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
    Ok(())
}

fn find_station(stations: &Vec<Station>, source: &String) -> Result<Station, anyhow::Error> {
    Ok(stations
        .iter()
        .find(|x| x.short.as_ref().unwrap_or(&x.name) == source)
        .ok_or(anyhow!("Can't find {}", source))?
        .clone())
}

fn compute_sell_sell<'a>(
    pairs: Vec<SystemMarketsItemData>,
    config: &Config,
    disable_filters: bool,
    simple_list: &mut Vec<SimpleDisplay>,
    name_len: usize,
) -> Vec<Row<'a>> {
    let good_items = get_good_items_sell_sell(pairs, config, disable_filters);
    *simple_list = good_items
        .iter()
        .map(|x| SimpleDisplay {
            name: x.market.desc.name.clone(),
            recommend_buy: x.recommend_buy,
            sell_price: x.dest_min_sell_price,
        })
        .collect();
    make_table_sell_sell(&good_items, name_len)
}

fn compute_sell_buy<'a>(
    pairs: Vec<SystemMarketsItemData>,
    config: &Config,
    disable_filters: bool,
    simple_list: &mut Vec<SimpleDisplay>,
    name_len: usize,
) -> anyhow::Result<Vec<Row<'a>>> {
    let good_items = get_good_items_sell_buy(pairs, config, disable_filters)?;
    *simple_list = good_items
        .items
        .iter()
        .map(|x| SimpleDisplay {
            name: x.market.desc.name.clone(),
            recommend_buy: x.recommend_buy,
            sell_price: x.dest_min_sell_price,
        })
        .collect();
    Ok(make_table_sell_buy(&good_items, name_len))
}

async fn compute_sell_sell_zkb<'a>(
    config: Config,
    mut cache: CachedStuff,
    force_no_refresh: bool,
    esi_requests: EsiRequestsService<'a>,
    esi_config: &Configuration,
    pairs: Vec<SystemMarketsItemData>,
    disable_filters: bool,
    simple_list: &mut Vec<SimpleDisplay>,
    name_len: usize,
) -> Result<Vec<Row<'a>>, anyhow::Error> {
    let cache_zkb_entity = format!(
        "cache/zkb_losses.{}.{}.rmp",
        config.common.zkill_entity.tp.zkill_filter_string(),
        config.common.zkill_entity.id
    );
    let kms = cache
        .load_or_create_async(
            cache_zkb_entity,
            vec![],
            false,
            if force_no_refresh {
                None
            } else {
                Some(Duration::hours(24))
            },
            || {
                let esi_requests = &esi_requests;
                let client = &esi_config.client;
                let config = &config;
                async move {
                    let zkb = ZkbRequestsService::new(client);
                    let km_service = KillmailService::new(&zkb, esi_requests);
                    Ok(km_service
                        .get_kill_item_frequencies(
                            &config.common.zkill_entity,
                            config.common.sell_sell.sell_sell_zkb.zkb_download_pages,
                        )
                        .await?)
                }
            },
        )
        .await?;
    let good_items = get_good_items_sell_sell_zkb(pairs, kms, &config, disable_filters);
    *simple_list = good_items
        .iter()
        .map(|x| SimpleDisplay {
            name: x.market.desc.name.clone(),
            recommend_buy: x.recommend_buy,
            sell_price: x.dest_min_sell_price,
        })
        .collect();
    Ok(make_table_sell_sell_zkb(&good_items, name_len))
}

async fn compute_pairs<'a>(
    config: &Config,
    esi_requests: &EsiRequestsService<'a>,
    character_id: i32,
    cache: &mut CachedStuff,
    force_refresh: bool,
    data_service: &DatadumpService,
) -> Result<Vec<SystemMarketsItemData>, anyhow::Error> {
    let config = config;
    let esi_requests = esi_requests;
    let source_region = esi_requests
        .find_region_id_station(config.route.source.clone(), character_id)
        .await
        .unwrap();
    let dest_region = esi_requests
        .find_region_id_station(config.route.destination.clone(), character_id)
        .await
        .unwrap();
    let all_types = cache
        .load_or_create_json_async(
            CACHE_ALL_TYPES,
            vec![],
            false,
            Some(Duration::days(7)),
            || async {
                let all_types = esi_requests.get_all_item_types(source_region.region_id);
                let all_types_dest = esi_requests.get_all_item_types(dest_region.region_id);
                let (all_types, all_types_dest) = join!(all_types, all_types_dest);
                let (mut all_types, all_types_dest) = (all_types?, all_types_dest?);

                all_types.extend(all_types_dest);
                all_types.sort_unstable();
                all_types.dedup();
                Ok(all_types)
            },
        )
        .await?;
    let all_type_descriptions: HashMap<i32, Option<TypeDescription>> = cache
        .load_or_create_async(
            CACHE_ALL_TYPE_DESC,
            vec![CACHE_ALL_TYPES],
            false,
            Some(Duration::days(7)),
            || async {
                let res = stream::iter(all_types.clone())
                    .map(|id| {
                        let esi_requests = &esi_requests;
                        async move {
                            let req_res = esi_requests.get_item_stuff(id).await?;

                            Ok((id, req_res.map(|x| x.into())))
                        }
                    })
                    .buffer_unordered(BUFFER_UNORDERED)
                    .collect::<Vec<error::Result<_>>>()
                    .await
                    .into_iter()
                    .collect::<error::Result<Vec<_>>>()?
                    .into_iter()
                    .collect();

                Ok(res)
            },
        )
        .await?;
    let cache_source_hist = format!("cache/{}.rmp", config.route.source.name);
    let source_item_data = cache
        .load_or_create_async(
            cache_source_hist,
            vec![CACHE_ALL_TYPES],
            force_refresh,
            Some(Duration::hours(config.common.refresh_timeout_hours)),
            || async {
                Ok(esi_requests
                    .all_item_data(&all_types, source_region)
                    .await?)
            },
        )
        .await?;
    let cache_dest_hist = format!("cache/{}.rmp", config.route.destination.name);
    let dest_item_data = cache
        .load_or_create_async(
            cache_dest_hist,
            vec![CACHE_ALL_TYPES],
            force_refresh,
            Some(Duration::hours(config.common.refresh_timeout_hours)),
            || async { Ok(esi_requests.all_item_data(&all_types, dest_region).await?) },
        )
        .await?;
    let source_types_average = source_item_data
        .into_iter()
        .map(|x| (x.id, x))
        .collect::<HashMap<_, _>>();
    let mut dest_types_average = dest_item_data
        .into_iter()
        .map(|x| (x.id, x))
        .collect::<HashMap<_, _>>();
    let pairs = source_types_average.into_iter().flat_map(|(k, v)| {
        Some(SystemMarketsItem {
            id: k,
            source: v.into(),
            destination: match dest_types_average.insert(k, Default::default()) {
                Some(x) => x.into(),
                None => {
                    log::warn!("Destination history didn't have history for item: {}", k);
                    return None;
                }
            },
        })
    });
    let group_ids = config
        .common
        .include_groups
        .as_ref()
        .map(|x| {
            let groups = x
                .iter()
                .map(|name| {
                    let children = data_service.get_all_group_id_with_root_name(name.as_str())?;

                    Ok(children)
                })
                .collect::<error::Result<Vec<_>>>()?
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();
            error::Result::Ok(groups)
        })
        .transpose()?;
    Ok(pairs
        .filter_map(|it| {
            let req_res = all_type_descriptions[&it.id].clone();
            let req_res = match req_res {
                Some(x) => x,
                None => return None,
            };

            // include only specific groups
            if let Some(ids) = &group_ids {
                if !req_res
                    .market_group_id
                    .map(|x| ids.contains(&x))
                    .unwrap_or(false)
                {
                    return None;
                }
            }

            Some(SystemMarketsItemData {
                desc: req_res,
                source: it.source,
                destination: it.destination,
            })
        })
        .collect::<Vec<_>>())
}

pub struct SimpleDisplay {
    pub name: String,
    pub recommend_buy: i32,
    pub sell_price: f64,
}
