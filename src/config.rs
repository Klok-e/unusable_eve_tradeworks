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

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub days_average: usize,
    pub max_filled_for_days_cutoff: f64,
    pub rcmnd_fill_days: f64,
    pub margin_cutoff: f64,
    pub sales_tax: f64,
    pub broker_fee_source: f64,
    pub broker_fee_destination: f64,
    pub freight_cost_iskm3: f64,
    pub freight_cost_collateral_percent: f64,
    pub items_take: usize,
    pub min_src_volume: f64,
    pub min_dst_volume: f64,
    pub min_dst_zkb_lost_volume: f64,
    pub zkb_download_pages: u32,
    pub source: Station,
    pub destination: Station,
    pub zkill_entity: ZkillEntity,
    pub refresh_timeout_hours: i64,
    pub min_profit: Option<f64>,
    pub include_groups: Option<Vec<String>>,
}

impl Config {
    pub fn from_file_json<P: AsRef<Path>>(path: P) -> crate::error::Result<Self> {
        let str = std::fs::read_to_string(path)?;
        let config: Config = serde_json::from_str(str.as_ref())?;

        Ok(config)
    }
}
