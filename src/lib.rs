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
pub mod zkb;

use serde::{Deserialize, Serialize};

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
