use rust_eveonline_esi::models::GetMarketsRegionIdHistory200Ok;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ItemType {
    pub id: i32,
    pub history: Vec<GetMarketsRegionIdHistory200Ok>,
}
