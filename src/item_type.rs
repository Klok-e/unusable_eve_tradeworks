use rust_eveonline_esi::models::GetUniverseTypesTypeIdOk;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MarketsRegionHistory {
    pub average: Option<f64>,
    pub date: String,
    pub highest: Option<f64>,
    pub lowest: Option<f64>,
    pub order_count: i64,
    pub volume: i64,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct ItemOrders {
    pub id: i32,
    pub orders: Vec<Order>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct ItemHistory {
    pub id: i32,
    pub history: Vec<MarketsRegionHistory>,
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
    pub volume_remain: i64,
    pub volume_total: i64,
}

#[derive(Debug, Serialize, Deserialize, Default, Copy, Clone)]
pub struct ItemTypeAveraged {
    pub average: f64,
    pub volume: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemHistoryDay {
    pub average: Option<f64>,
    pub highest: Option<f64>,
    pub lowest: Option<f64>,
    pub order_count: i64,
    pub volume: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketData {
    pub history: Vec<ItemHistoryDay>,
    pub orders: Vec<Order>,
}

impl MarketData {
    pub fn new(orders: ItemOrders, history: ItemHistory) -> Self {
        MarketData {
            history: history
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
            orders: orders.orders,
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
    pub adjusted_price: Option<f64>,
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
    pub portion_size: Option<i64>,
    pub published: bool,
    pub radius: Option<f32>,
    pub type_id: i32,
    pub volume: f32,
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
            portion_size: x.portion_size.map(|x| x as i64),
            published: x.published,
            radius: x.radius,
            type_id: x.type_id,
            volume: x.packaged_volume.or(x.volume).unwrap(),
        }
    }
}
