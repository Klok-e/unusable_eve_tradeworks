use crate::{cached_data::CachedData, config::AuthConfig};
use chrono::{DateTime, Duration, Utc};
use oauth2::{
    basic::{BasicClient, BasicTokenType},
    reqwest::async_http_client,
    AuthUrl, AuthorizationCode, ClientId, CsrfToken, EmptyExtraTokenFields, HttpResponse,
    PkceCodeChallenge, RedirectUrl, Scope, StandardTokenResponse, TokenResponse, TokenUrl,
};
use reqwest::{self, Url};
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub struct Auth {
    pub token: StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>,
}

impl Auth {
    pub async fn load_or_request_token(config: &AuthConfig) -> Self {
        let path = "cache/auth";
        let mut data =
            CachedData::load_or_create_json_async(path, false, || Self::request_new(config))
                .await
                .data;

        // if expired use refresh token
        if data.expires_in().unwrap() < std::time::Duration::ZERO {
            let client = BasicClient::new(
                ClientId::new(config.client_id.clone()),
                None,
                AuthUrl::new("https://login.eveonline.com/v2/oauth/authorize".to_string()).unwrap(),
                Some(
                    TokenUrl::new("https://login.eveonline.com/v2/oauth/token".to_string())
                        .unwrap(),
                ),
            );
            data = client
                .exchange_refresh_token(data.refresh_token().unwrap())
                .request_async(async_http_client)
                .await
                .unwrap();

            let vec = serde_json::to_vec_pretty(&data).unwrap();
            std::fs::write(path, vec).unwrap();
        }

        Self { token: data }
    }

    async fn request_new(
        config: &AuthConfig,
    ) -> StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType> {
        let scopes = vec![
            "esi-markets.structure_markets.v1",
            "esi-search.search_structures.v1",
            "esi-universe.read_structures.v1",
        ];

        let client = BasicClient::new(
            ClientId::new(config.client_id.clone()),
            None,
            AuthUrl::new("https://login.eveonline.com/v2/oauth/authorize".to_string()).unwrap(),
            Some(TokenUrl::new("https://login.eveonline.com/v2/oauth/token".to_string()).unwrap()),
        )
        .set_redirect_uri(
            RedirectUrl::new("https://localhost/oauth-callback".to_string()).unwrap(),
        );

        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

        // Generate the full authorization URL.
        let (auth_url, csrf_token) = client
            .authorize_url(CsrfToken::new_random)
            .add_scopes(scopes.into_iter().map(|s| Scope::new(s.to_string())))
            .set_pkce_challenge(pkce_challenge)
            .url();

        println!("Go to this url then copy the redirected url here:");
        println!("{}", auth_url);

        let mut str = String::new();
        std::io::stdin().read_line(&mut str).unwrap();
        let str = str.trim();
        let code = Url::parse(str).unwrap();
        let mut params = code.query_pairs();
        let code = params.find(|x| x.0 == "code").unwrap().1;
        let state = params.find(|x| x.0 == "state").unwrap().1;

        if state.as_ref() != csrf_token.secret() {
            panic!("Csrf token doesn't match!");
        }

        let token_result = client
            .exchange_code(AuthorizationCode::new(code.to_string()))
            // Set the PKCE code verifier.
            .set_pkce_verifier(pkce_verifier)
            .request_async(async_http_client)
            .await
            .unwrap();

        token_result
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
