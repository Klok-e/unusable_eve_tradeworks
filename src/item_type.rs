use chrono::DateTime;
use rust_eveonline_esi::models::{GetMarketsRegionIdHistory200Ok, GetUniverseTypesTypeIdOk};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize,Default)]
pub struct ItemType {
    pub id: i32,
    pub history: Vec<GetMarketsRegionIdHistory200Ok>,
    pub orders: Vec<Order>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub duration: i32,
    pub is_buy_order: bool,
    pub issued: String,
    pub location_id: i64,
    pub min_volume: i32,
    pub order_id: i64,
    pub price: f64,
    pub type_id: i32,
    pub volume_remain: i32,
    pub volume_total: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ItemTypeAveraged {
    pub average: f64,
    pub highest: f64,
    pub lowest: f64,
    pub order_count: f64,
    pub volume: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemHistoryDay {
    pub average: f64,
    pub highest: f64,
    pub lowest: f64,
    pub order_count: i64,
    pub volume: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketData {
    pub history: Vec<ItemHistoryDay>,
    pub orders: Vec<Order>,
}

impl From<ItemType> for MarketData {
    fn from(x: ItemType) -> Self {
        MarketData {
            history: x
                .history
                .iter()
                .map(|x| ItemHistoryDay {
                    average: x.average,
                    highest: x.highest,
                    lowest: x.lowest,
                    order_count: x.order_count,
                    volume: x.volume,
                })
                .collect(),
            orders: x.orders,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SystemMarketsItem {
    pub id: i32,
    pub source: MarketData,
    pub destination: MarketData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMarketsItemData {
    pub desc: TypeDescription,
    pub source: MarketData,
    pub destination: MarketData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeDescription {
    pub capacity: Option<f32>,
    pub description: String,
    pub graphic_id: Option<i32>,
    pub group_id: i32,
    pub icon_id: Option<i32>,
    pub market_group_id: Option<i32>,
    pub mass: Option<f32>,
    pub name: String,
    pub packaged_volume: Option<f32>,
    pub portion_size: Option<i32>,
    pub published: bool,
    pub radius: Option<f32>,
    pub type_id: i32,
    pub volume: Option<f32>,
}

impl From<GetUniverseTypesTypeIdOk> for TypeDescription {
    fn from(x: GetUniverseTypesTypeIdOk) -> Self {
        Self {
            capacity: x.capacity,
            description: x.description,
            graphic_id: x.graphic_id,
            group_id: x.group_id,
            icon_id: x.icon_id,
            market_group_id: x.market_group_id,
            mass: x.mass,
            name: x.name,
            packaged_volume: x.packaged_volume,
            portion_size: x.portion_size,
            published: x.published,
            radius: x.radius,
            type_id: x.type_id,
            volume: x.volume,
        }
    }
}
