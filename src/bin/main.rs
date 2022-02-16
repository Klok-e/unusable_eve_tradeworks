use std::collections::HashMap;

use chrono::Duration;
use futures::{stream, FutureExt, StreamExt};

use oauth2::TokenResponse;
use rust_eveonline_esi::apis::{
    configuration::Configuration,
    market_api::{self, GetMarketsRegionIdTypesParams},
};

use term_table::{row::Row, table_cell::TableCell, TableBuilder, TableStyle};
use tokio::join;

use serde::{Deserialize, Serialize};
use unusable_eve_tradeworks_lib::{
    auth::Auth,
    cached_data::CachedData,
    cli,
    config::{AuthConfig, Config},
    consts::{self, BUFFER_UNORDERED},
    error::Result,
    good_items::{
        sell_buy::{get_good_items_sell_buy, make_table_sell_buy},
        sell_sell::{get_good_items_sell_sell, make_table_sell_sell},
        sell_sell_zkb::{get_good_items_sell_sell_zkb, make_table_sell_sell_zkb},
    },
    item_type::{SystemMarketsItem, SystemMarketsItemData},
    logger,
    paged_all::get_all_pages,
    requests::EsiRequestsService,
    zkb::{killmails::KillmailService, zkb_requests::ZkbRequestsService},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterInfo {
    pub scp: Vec<String>,
    pub jti: String,
    pub kid: String,
    pub sub: String,
    pub azp: String,
    pub tenant: String,
    pub tier: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    run().await
}

async fn run() -> Result<()> {
    let cli_args = cli::matches();

    let quiet = cli_args.is_present(cli::QUIET);
    logger::setup_logger(quiet)?;

    let config_file_name = cli_args.value_of(cli::CONFIG).unwrap_or("config.json");
    let config = Config::from_file_json(config_file_name)?;

    let program_config = AuthConfig::from_file("auth.json");
    let auth = Auth::load_or_request_token(&program_config).await;

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

    // TODO: dangerous plese don't use in production
    let character_info = jsonwebtoken::dangerous_insecure_decode::<CharacterInfo>(
        auth.token.access_token().secret(),
    )
    .unwrap()
    .claims;
    let character_id = character_info
        .sub
        .split(':')
        .nth(2)
        .unwrap()
        .parse()
        .unwrap();

    let force_refresh = cli_args.is_present(cli::FORCE_REFRESH);
    let force_no_refresh = cli_args.is_present(cli::FORCE_NO_REFRESH);

    let mut pairs: Vec<SystemMarketsItemData> = CachedData::load_or_create_async(
        format!("cache/{}.rmp", config_file_name),
        force_refresh,
        if force_no_refresh {
            None
        } else {
            Some(Duration::hours(config.refresh_timeout_hours))
        },
        || {
            let esi_config = &esi_config;
            let config = &config;
            let esi_requests = &esi_requests;
            async move {
                let source_region = esi_requests
                    .find_region_id_station(config.source.clone(), character_id)
                    .await
                    .unwrap();

                let dest_region = esi_requests
                    .find_region_id_station(config.destination.clone(), character_id)
                    .await
                    .unwrap();

                // all item type ids
                let all_types = CachedData::load_or_create_json_async(
                    "cache/all_types.json",
                    force_refresh,
                    Some(Duration::days(7)),
                    || async {
                        let mut all_types =
                            get_all_item_types(esi_config, source_region.region_id).await;
                        let all_types_dest =
                            get_all_item_types(esi_config, dest_region.region_id).await;
                        all_types.extend(all_types_dest);
                        all_types.sort_unstable();
                        all_types.dedup();
                        all_types
                    },
                )
                .await
                .data;

                let source_history = CachedData::load_or_create_async(
                    format!("cache/{}.rmp", config.source.name),
                    force_refresh,
                    Some(Duration::hours(config.refresh_timeout_hours)),
                    || async { esi_requests.history(&all_types, source_region).await },
                )
                .map(|x| x.data);
                let dest_history = CachedData::load_or_create_async(
                    format!("cache/{}.rmp", config.destination.name),
                    force_refresh,
                    Some(Duration::hours(config.refresh_timeout_hours)),
                    || async { esi_requests.history(&all_types, dest_region).await },
                )
                .map(|x| x.data);

                let (source_history, dest_history) = join!(source_history, dest_history);

                // turn history into n day average
                let source_types_average = source_history
                    .into_iter()
                    .map(|x| (x.id, x))
                    .collect::<HashMap<_, _>>();

                let mut dest_types_average = dest_history
                    .into_iter()
                    .map(|x| (x.id, x))
                    .collect::<HashMap<_, _>>();

                // pair
                let pairs = source_types_average.into_iter().flat_map(|(k, v)| {
                    Some(SystemMarketsItem {
                        id: k,
                        source: v.into(),
                        destination: match dest_types_average.insert(k, Default::default()) {
                            Some(x) => x.into(),
                            None => {
                                log::warn!(
                                    "Destination history didn't have history for item: {}",
                                    k
                                );
                                return None;
                            }
                        },
                    })
                });

                stream::iter(pairs)
                    .map(|it| {
                        let it = it;
                        let esi_requests = &esi_requests;
                        async move {
                            Some(SystemMarketsItemData {
                                desc: match esi_requests.get_item_stuff(it.id).await {
                                    Some(x) => x.into(),
                                    None => return None,
                                },
                                source: it.source,
                                destination: it.destination,
                            })
                        }
                    })
                    .buffer_unordered(BUFFER_UNORDERED)
                    .collect::<Vec<Option<SystemMarketsItemData>>>()
                    .await
                    .into_iter()
                    .flatten()
                    .collect()
            }
        },
    )
    .await
    .data;

    let mut disable_filters = false;
    if let Some(v) = cli_args
        .value_of(cli::DEBUG_ITEM_ID)
        .and_then(|x| x.parse::<i32>().ok())
    {
        pairs.retain(|x| x.desc.type_id == v);
        disable_filters = true;
    }

    let simple_list: Vec<_>;
    let rows = {
        let cli_in = cli_args.value_of(cli::NAME_LENGTH);
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

        let sell_sell = cli_args.is_present(cli::SELL_SELL);
        let sell_sell_zkb = cli_args.is_present(cli::SELL_SELL_ZKB);
        let sell_buy = cli_args.is_present(cli::SELL_BUY);
        if sell_sell || (!sell_buy && !sell_sell_zkb) {
            log::trace!("Sell sell path.");
            let good_items = get_good_items_sell_sell(pairs, &config, disable_filters);
            simple_list = good_items
                .iter()
                .map(|x| SimpleDisplay {
                    name: x.market.desc.name.clone(),
                    recommend_buy: x.recommend_buy,
                    sell_price: x.dest_min_sell_price,
                })
                .collect();
            make_table_sell_sell(&good_items, name_len)
        } else if sell_buy {
            log::trace!("Sell buy path.");
            let good_items = get_good_items_sell_buy(pairs, &config, disable_filters);
            simple_list = good_items
                .iter()
                .map(|x| SimpleDisplay {
                    name: x.market.desc.name.clone(),
                    recommend_buy: x.recommend_buy,
                    sell_price: x.dest_min_sell_price,
                })
                .collect();
            make_table_sell_buy(&good_items, name_len)
        } else {
            log::trace!("Sell sell zkb path.");
            let kms = CachedData::load_or_create_async(
                "cache/zkb_losses",
                force_refresh,
                if force_no_refresh {
                    None
                } else {
                    Some(Duration::hours(config.refresh_timeout_hours))
                },
                || {
                    let esi_requests = &esi_requests;
                    let client = &esi_config.client;
                    let config = &config;
                    async move {
                        let zkb = ZkbRequestsService::new(client);
                        let km_service = KillmailService::new(&zkb, esi_requests);
                        km_service
                            .get_kill_item_frequencies(
                                &config.zkill_entity,
                                config.zkb_download_pages,
                            )
                            .await
                    }
                },
            )
            .await
            .data;

            let good_items = get_good_items_sell_sell_zkb(pairs, kms, &config, disable_filters);
            simple_list = good_items
                .iter()
                .map(|x| SimpleDisplay {
                    name: x.market.desc.name.clone(),
                    recommend_buy: x.recommend_buy,
                    sell_price: x.dest_min_sell_price,
                })
                .collect();
            make_table_sell_sell_zkb(&good_items, name_len)
        }
    };

    let table = TableBuilder::new().rows(rows).build();
    println!("{}", table.render());

    if cli_args.is_present(cli::DISPLAY_SIMPLE_LIST) {
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
    if cli_args.is_present(cli::DISPLAY_SIMPLE_LIST_PRICE) {
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

async fn get_all_item_types(esi_config: &Configuration, region_id: i32) -> Vec<i32> {
    get_all_pages(
        |page| {
            let config = &esi_config;
            async move {
                market_api::get_markets_region_id_types(
                    config,
                    GetMarketsRegionIdTypesParams {
                        region_id: region_id,
                        datasource: None,
                        if_none_match: None,
                        page: Some(page),
                    },
                )
                .await
                .unwrap()
                .entity
                .unwrap()
            }
        },
        1000,
    )
    .await
}

pub struct SimpleDisplay {
    pub name: String,
    pub recommend_buy: i32,
    pub sell_price: f64,
}
