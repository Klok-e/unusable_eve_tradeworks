use crate::{cached_data::CachedData, config::AuthConfig};
use chrono::{DateTime, Duration, Utc};
use reqwest::{self, Url};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Auth {
    pub access_token: String,
    pub refresh_token: String,
    pub expiration_date: DateTime<Utc>,
}

impl Auth {
    pub async fn load_or_request_token(config: &AuthConfig) -> Self {
        let path = "cache/auth";
        let mut data = CachedData::load_or_create_json_async(path, false,|| Self::request_new(config))
            .await
            .data;

        // if expired use refresh token
        if data.expiration_date < Utc::now() {
            let client = reqwest::Client::new();
            let response = client
                .post("https://login.eveonline.com/v2/oauth/token")
                .form(&AuthRefreshTokenParams {
                    grant_type: "refresh_token".into(),
                    refresh_token: data.refresh_token,
                    client_id: config.client_id.clone(),
                })
                .send()
                .await
                .unwrap()
                .json::<EveAuthResponse>()
                .await
                .unwrap();

            data = Auth {
                access_token: response.access_token,
                refresh_token: response.refresh_token,
                expiration_date: expiration_date(response.expires_in),
            };
            let vec = serde_json::to_vec_pretty(&data).unwrap();
            std::fs::write(path, vec).unwrap();
        }

        data
    }

    async fn request_new(config: &AuthConfig) -> Self {
        let scopes = vec![
            "esi-markets.structure_markets.v1",
            "esi-search.search_structures.v1",
            "esi-universe.read_structures.v1",
        ];
        let url: String = Url::parse_with_params(
            "https:/login.eveonline.com/v2/oauth/authorize",
            &[
                ("response_type", "code"),
                ("redirect_uri", "https://localhost/oauth-callback"),
                ("client_id", config.client_id.to_string().as_str()),
                ("scope", scopes.join(" ").as_str()),
                ("state", config.state.as_str()),
            ],
        )
        .unwrap()
        .into();

        println!("Go to this url then copy the redirected url here:");
        println!("{}", url);

        let mut str = String::new();
        std::io::stdin().read_line(&mut str).unwrap();
        let str = str.trim();
        let code = Url::parse(str).unwrap();
        let mut code = code.query_pairs();
        let code = code.find(|x| x.0 == "code").unwrap().1;

        let client = reqwest::Client::new();
        let res = client
            .post("https://login.eveonline.com/v2/oauth/token")
            .form(&AuthTokenParams {
                grant_type: "authorization_code".into(),
                code: code.into(),
                client_id: config.client_id.clone(),
            })
            .send()
            .await
            .unwrap()
            .json::<EveAuthResponse>()
            .await
            .unwrap();

        Self {
            access_token: res.access_token,
            refresh_token: res.refresh_token,
            expiration_date: expiration_date(res.expires_in),
        }
    }
}

fn expiration_date(expires_in: i64) -> DateTime<Utc> {
    Utc::now() + Duration::seconds(expires_in)
}

#[derive(Serialize)]
struct AuthTokenParams {
    pub grant_type: String,
    pub code: String,
    pub client_id: String,
}

#[derive(Serialize)]
struct AuthRefreshTokenParams {
    pub grant_type: String,
    pub refresh_token: String,
    pub client_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct EveAuthResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub refresh_token: String,
}
