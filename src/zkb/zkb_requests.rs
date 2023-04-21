use serde::{Deserialize, Serialize};

use crate::requests::retry::{retry_smart, Retry};

pub struct ZkbRequestsService<'a> {
    client: &'a reqwest::Client,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ZkillEntity {
    pub id: u32,
    pub tp: ZkillEntityType,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ZkillEntityType {
    Corporation,
    Alliance,
    Region,
}

impl ZkillEntityType {
    pub fn zkill_filter_string(&self) -> &'static str {
        match self {
            ZkillEntityType::Corporation => "corporationID",
            ZkillEntityType::Alliance => "allianceID",
            ZkillEntityType::Region => "regionID",
        }
    }
}

impl<'a> ZkbRequestsService<'a> {
    pub fn new(client: &'a reqwest::Client) -> Self {
        Self { client }
    }

    pub async fn get_killmails(
        &self,
        ZkillEntity {
            id: entity_id,
            tp: entity_type,
        }: &ZkillEntity,
        pages: u32,
    ) -> Result<KillList, reqwest::Error> {
        let mut kills = KillList::new();
        log::info!("Getting killmails...");
        for pg in 1..=pages {
            let mut kills_page = retry_smart(|| async {
                let url = format!(
                    "https://zkillboard.com/api/losses/{}/{}/page/{}/",
                    entity_type.zkill_filter_string(),
                    entity_id,
                    pg
                );
                let response = self.client.get(url.clone()).send().await?;
                if response.status() == 429 {
                    log::warn!("Zkill returned status 429. Retrying in 1 second...");
                    tokio::time::sleep(std::time::Duration::from_secs_f32(1.)).await;
                    return Ok(Retry::Retry);
                }

                let full = response.bytes().await?;
                let ser_res = serde_json::from_slice(&full);

                let kills_page = ser_res
                    .map_err(|e| {
                        let save_path = "cache/tmp-tst";
                        log::error!(
                            "Errorneous url: {}. Zkill server response saved to: {}",
                            url,
                            save_path
                        );
                        std::fs::write(save_path, &full).unwrap();
                        e
                    })
                    .unwrap();

                // zkillboard allows only one request per second
                tokio::time::sleep(std::time::Duration::from_secs_f32(1.)).await;

                Ok(Retry::Success(kills_page))
            })
            .await?
            .unwrap();
            kills.append(&mut kills_page);
        }
        log::info!("{} page of killmails downloaded", pages);
        Ok(kills)
    }
}

pub type KillList = Vec<Kill>;

#[derive(Debug, Serialize, Deserialize)]
pub struct Kill {
    pub killmail_id: i32,
    pub zkb: Zkb,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Zkb {
    #[serde(rename = "locationID")]
    pub location_id: i64,
    pub hash: String,
    #[serde(rename = "fittedValue")]
    pub fitted_value: f64,
    #[serde(rename = "droppedValue")]
    pub dropped_value: f64,
    #[serde(rename = "destroyedValue")]
    pub destroyed_value: f64,
    #[serde(rename = "totalValue")]
    pub total_value: f64,
    pub points: i64,
    pub npc: bool,
    pub solo: bool,
    pub awox: bool,
}
