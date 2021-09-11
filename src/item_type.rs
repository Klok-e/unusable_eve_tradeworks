use rust_eveonline_esi::models::GetMarketsRegionIdHistory200Ok;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ItemType {
    pub id: i32,
    pub history: Vec<GetMarketsRegionIdHistory200Ok>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ItemTypeAveraged {
    pub id: i32,
    pub market_data: MarketData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketData {
    pub average: f64,
    pub highest: f64,
    pub lowest: f64,
    pub order_count: f64,
    pub volume: f64,
}
