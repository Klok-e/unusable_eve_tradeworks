#![feature(bool_to_option)]

pub mod auth;
pub mod cached_data;
pub mod cli;
pub mod config;
pub mod consts;
pub mod error;
pub mod good_items;
pub mod item_type;
pub mod logger;
pub mod order_ext;
pub mod paged_all;
pub mod requests;
pub mod retry;
pub mod stat;
use std::collections::HashMap;

use error::Result;

use futures::{stream, StreamExt};

use rust_eveonline_esi::apis::{
    configuration::Configuration,
    market_api::{self, GetMarketsRegionIdTypesParams},
};

use term_table::TableBuilder;
use tokio::join;

use crate::{
    auth::Auth,
    cached_data::CachedData,
    config::{AuthConfig, Config},
    consts::BUFFER_UNORDERED,
    good_items::{get_good_items_sell_sell, make_table_sell_sell},
    item_type::{SystemMarketsItem, SystemMarketsItemData},
    paged_all::get_all_pages,
    requests::{find_region_id_station, get_item_stuff, history},
};
use serde::{Deserialize, Serialize};

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
    logger::setup_logger()?;

    let cli_args = cli::matches();

    let config_file_name = cli_args.value_of(cli::CONFIG).unwrap_or("config.json");
    let config = Config::from_file_json(config_file_name)?;

    let program_config = AuthConfig::from_file("auth.json");
    let auth = Auth::load_or_request_token(&program_config).await;

    let mut esi_config = Configuration::new();
    esi_config.oauth_access_token = Some(auth.access_token.clone());

    // TODO: dangerous plese don't use in production
    let character_info =
        jsonwebtoken::dangerous_insecure_decode::<CharacterInfo>(auth.access_token.as_str())
            .unwrap()
            .claims;
    let character_id = character_info
        .sub
        .split(':')
        .nth(2)
        .unwrap()
        .parse()
        .unwrap();

    let pairs: Vec<SystemMarketsItemData> =
        CachedData::load_or_create_async(format!("cache/{}-path_data", config_file_name), || {
            let esi_config = &esi_config;
            let config = &config;
            async move {
                let source_region =
                    find_region_id_station(esi_config, config.source.clone(), character_id)
                        .await
                        .unwrap();

                // all item type ids
                let all_types = get_all_pages(
                    |page| {
                        let config = &esi_config;
                        async move {
                            market_api::get_markets_region_id_types(
                                config,
                                GetMarketsRegionIdTypesParams {
                                    region_id: source_region.region_id,
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
                .await;
                // let all_types = vec![16278];

                let dest_region =
                    find_region_id_station(esi_config, config.destination.clone(), character_id)
                        .await
                        .unwrap();

                let source_history = history(esi_config, &all_types, source_region);
                let dest_history = history(esi_config, &all_types, dest_region);

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
                        async move {
                            Some(SystemMarketsItemData {
                                desc: match get_item_stuff(esi_config, it.id).await {
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
        })
        .await
        .data;

    let good_items = get_good_items_sell_sell(pairs, &config);
    let rows = make_table_sell_sell(&good_items);

    let table = TableBuilder::new().rows(rows).build();
    println!("Maybe good items:\n{}", table.render());
    println!();

    let format = good_items
        .iter()
        .map(|it| format!("{} {}", it.market.desc.name, it.recommend_buy))
        .collect::<Vec<_>>();
    println!("Item names only:\n{}", format.join("\n"));

    Ok(())
}

#[derive(Clone, Copy)]
pub struct StationIdData {
    pub station_id: StationId,
    pub system_id: i32,
    pub region_id: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Station {
    pub is_citadel: bool,
    pub name: String,
}
#[derive(Clone, Copy)]
pub struct StationId {
    pub is_citadel: bool,
    pub id: i64,
}
