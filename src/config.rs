use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{zkb::zkb_requests::ZkillEntity, Station};

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthConfig {
    pub client_id: String,
}

impl AuthConfig {
    pub fn from_file(path: &str) -> Self {
        let config_data = std::fs::read(path).unwrap();
        serde_json::from_slice(config_data.as_slice()).unwrap()
    }
}

pub struct Config {
    pub route: RouteConfig,
    pub common: CommonConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RouteConfig {
    pub source: Station,
    pub destination: Station,
}

impl RouteConfig {
    pub fn from_file_json<P: AsRef<Path>>(path: P) -> crate::error::Result<Self> {
        let str = std::fs::read_to_string(path.as_ref())?;
        let config: RouteConfig = serde_json::from_str(str.as_ref())?;
        let str = serde_json::to_string_pretty(&config)?;
        std::fs::write(path, str)?;

        Ok(config)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommonConfig {
    pub days_average: usize,
    pub margin_cutoff: f64,
    pub sales_tax: f64,
    pub items_take: usize,
    pub zkill_entity: ZkillEntity,
    pub refresh_timeout_hours: f64,
    pub item_history_timeout_hours: i64,
    pub min_profit: Option<f64>,
    pub include_groups: Option<Vec<String>>,
    pub sell_sell: ConfigSellSell,
    pub station_trade: ConfigStationTrade,
    pub cargo_capacity: i32,
    pub max_investment_per_item: f64,
    pub sell_reprocess: ConfigSellReprocess,
    pub stations: Vec<Station>,
}

impl CommonConfig {
    pub fn from_file_json<P: AsRef<Path>>(path: P) -> crate::error::Result<Self> {
        let str = std::fs::read_to_string(path.as_ref())?;
        let config: Self = serde_json::from_str(str.as_ref())?;
        let str = serde_json::to_string_pretty(&config)?;
        std::fs::write(path, str)?;

        Ok(config)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigSellSell {
    pub rcmnd_fill_days: f64,
    pub min_src_volume: f64,
    pub min_dst_volume: f64,
    pub max_filled_for_days_cutoff: f64,
    pub freight_cost_iskm3: f64,
    pub freight_cost_collateral_percent: f64,
    pub sell_sell_zkb: ConfigSellSellZkb,
    pub markup_if_no_orders_dest: f64,
    pub dst_ignore_orders_under_volume_pct: f64,
    pub exclude_groups: Option<Vec<String>>,
    pub include_groups: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigStationTrade {
    pub daily_volume_pct: f64,
    pub min_item_volume: f64,
    pub markup_if_no_orders_dest: f64,
    pub dst_ignore_orders_under_volume_pct: f64,
    pub exclude_groups: Option<Vec<String>>,
    pub include_groups: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigSellSellZkb {
    pub min_dst_zkb_lost_volume: f64,
    pub zkb_download_pages: u32,
    pub zkb_losses_volume_multiplier: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigSellReprocess {
    pub repro_portion: f64,
    pub repro_tax: f64,
}
