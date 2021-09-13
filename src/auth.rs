use crate::cached_data::CachedData;
use reqwest;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Auth {
    pub access_token: String,
    pub refresh_token: String,
}

impl Auth {
    pub async fn load_or_request_token() -> Self {
        CachedData::load_or_create_json_async("cache/auth", || async {
            let client_id = "test";
            let url = format!(
                "https:/login.eveonline.com/oauth/authorize\
                ?response_type=code\
                &redirect_uri=eveauth-app://callback/\
                &client_id={}\
                &scope=esi-markets.structure_markets.v1 esi-search.search_structures.v1 esi-universe.read_structures.v1",
                client_id
            );

            println!("Go to this url then copy the redirected url here:");
            println!("{}", url);

            let mut str = String::new();
            std::io::stdin().read_line(&mut str).unwrap();
            let str = str.trim();
            let code = str.split("=").skip(1).next().unwrap();
            let client_id = "stub";
            let client_secret = "stub";

            let client = reqwest::Client::new();
            let res = client
                .post("https://login.eveonline.com/oauth/token")
                .header("Content-Type", "application/json")
                .header(
                    "Authorization",
                    format!(
                        "Basic {}",
                        base64::encode(format!("{}:{}", client_id, client_secret))
                    ),
                )
                .body(format!(
                    "{{\"grant_type\":\"authorization_code\", \"code\":\"{}\"}}",
                    code
                ))
                .send()
                .await
                .unwrap()
                .json::<EveAuthResponse>()
                .await
                .unwrap();

            Self {
                access_token: res.access_token,
                refresh_token: res.refresh_token,
            }
        })
        .await
        .data
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct EveAuthResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u32,
    pub refresh_token: String,
}
