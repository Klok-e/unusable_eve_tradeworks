use serde::{Deserialize, Serialize};

pub struct ZkbRequestsService<'a> {
    client: &'a reqwest::Client,
}
impl<'a> ZkbRequestsService<'a> {
    pub fn new(client: &'a reqwest::Client) -> Self {
        Self { client }
    }

    pub async fn get_killmails(&self, pages: u32) -> Result<KillList, reqwest::Error> {
        let mut kills = KillList::new();
        log::info!("Getting killmails...");
        for pg in 1..=pages {
            let url = format!(
                "https://zkillboard.com/api/losses/allianceID/498125261/page/{}/",
                pg
            );
            let response = self.client.get(url).send().await?;

            // TODO: investigate why `response.json::<KillList>()` doesn't work
            let str = response.text().await.unwrap();

            // zkillboard allows only one request a per second
            tokio::time::sleep(std::time::Duration::from_secs_f32(1.01)).await;
            
            let mut kills_page = serde_json::from_str(str.as_str()).unwrap(); //response.json::<KillList>().await.unwrap();
            kills.append(&mut kills_page);
        }
        log::info!("{} page of killmails downloaded",pages);
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
