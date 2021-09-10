mod error;

use std::error::Error;

use error::Result;
use rust_eveonline_esi::apis::{configuration::Configuration, market_api::{self, GetMarketsGroupsParams, GetMarketsGroupsSuccess}};

#[tokio::main]
async fn main() -> Result<()> {
    run().await
}

async fn run() -> Result<()> {
    let config = Configuration::new();

    let groups = market_api::get_markets_groups(
        &config,
        GetMarketsGroupsParams {
            datasource: None,
            if_none_match: None,
        },
    )
    .await?;
    if let GetMarketsGroupsSuccess::Status200(res)=groups.entity.unwrap(){
        
    }

    std::fs::write("./test.txt", "").unwrap();

    Ok(())
}
