use crate::{cached_data::CachedStuff, config::AuthConfig};

use chrono::{DateTime, Utc};

use jsonwebtoken::{DecodingKey, Validation};
use oauth2::{
    basic::{BasicClient, BasicTokenType},
    reqwest::async_http_client,
    AuthUrl, AuthorizationCode, ClientId, CsrfToken, EmptyExtraTokenFields, PkceCodeChallenge,
    RedirectUrl, Scope, StandardTokenResponse, TokenResponse, TokenUrl,
};
use reqwest::{self, Url};
use serde::{Deserialize, Serialize};

const AUTHORIZE_ISSUER: &str = "https://login.eveonline.com/v2/oauth/authorize";
const TOKEN_ISSUER: &str = "https://login.eveonline.com/v2/oauth/token";
const LOCALHOST_CALLBACK: &str = "http://localhost:8022/callback";
const SSO_META_DATA_URL: &str =
    "https://login.eveonline.com/.well-known/oauth-authorization-server";
const JWK_ALGORITHM: &str = "RS256";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterInfo {
    pub scp: Vec<String>,
    pub jti: String,
    pub kid: String,
    pub sub: String,
    pub azp: String,
    pub tenant: String,
    pub tier: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Auth {
    pub token: StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>,
    pub expiration_date: DateTime<Utc>,
    pub character_info: CharacterInfo,
}

impl Auth {
    pub async fn load_or_request_token(
        config: &AuthConfig,
        cache: &mut CachedStuff,
        path: &str,
    ) -> Self {
        let mut data = cache
            .load_or_create_json_async(path, vec![], false, None, || async {
                let token = Self::request_new(config).await;
                let expiration_date =
                    Utc::now() + chrono::Duration::from_std(token.expires_in().unwrap()).unwrap();

                Ok(Auth {
                    character_info: validate_token(&token).await,
                    token,
                    expiration_date,
                })
            })
            .await
            .unwrap();

        // if expired use refresh token
        if data.expiration_date < Utc::now() {
            let client = create_client(config);
            let token = client
                .exchange_refresh_token(data.token.refresh_token().unwrap())
                .request_async(async_http_client)
                .await
                .unwrap();

            let expiration_date =
                Utc::now() + chrono::Duration::from_std(token.expires_in().unwrap()).unwrap();
            data = Auth {
                character_info: validate_token(&token).await,
                token,
                expiration_date,
            };

            cache
                .load_or_create_json_async(path, vec![], true, None, || {
                    let data = data.clone();
                    async { Ok(data) }
                })
                .await
                .unwrap();
        }

        data
    }

    async fn request_new(
        config: &AuthConfig,
    ) -> StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType> {
        let scopes = vec![
            "esi-markets.structure_markets.v1",
            "esi-search.search_structures.v1",
            "esi-universe.read_structures.v1",
        ];

        let client = create_client(config);

        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

        // Generate the full authorization URL.
        let (auth_url, csrf_token) = client
            .authorize_url(CsrfToken::new_random)
            .add_scopes(scopes.into_iter().map(|s| Scope::new(s.to_string())))
            .set_pkce_challenge(pkce_challenge)
            .url();

        println!("Go to this url:");
        println!("{}", auth_url);

        let mut str = None;

        let server = tiny_http::Server::http("localhost:8022").unwrap();
        if let Some(request) = server.incoming_requests().next() {
            log::debug!(
                "received request. method: {:?}, url: {:?}, headers: {:?}",
                request.method(),
                request.url(),
                request.headers()
            );

            str = Some(request.url().to_string());
            let response = tiny_http::Response::from_string("Successful. You can close this tab.");
            request.respond(response).unwrap();
        }
        drop(server);

        let str = str.unwrap();
        log::debug!("Request string: {}", str);

        let str = str.trim();
        let code = Url::parse(format!("http://{}", str).as_str()).unwrap();
        let mut params = code.query_pairs();
        let code = params.find(|x| x.0 == "code").unwrap().1;
        let state = params.find(|x| x.0 == "state").unwrap().1;

        if state.as_ref() != csrf_token.secret() {
            panic!("Csrf token doesn't match!");
        }

        client
            .exchange_code(AuthorizationCode::new(code.to_string()))
            // Set the PKCE code verifier.
            .set_pkce_verifier(pkce_verifier)
            .request_async(async_http_client)
            .await
            .unwrap()
    }
}

type OauthClient = oauth2::Client<
    oauth2::StandardErrorResponse<oauth2::basic::BasicErrorResponseType>,
    StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>,
    BasicTokenType,
    oauth2::StandardTokenIntrospectionResponse<EmptyExtraTokenFields, BasicTokenType>,
    oauth2::StandardRevocableToken,
    oauth2::StandardErrorResponse<oauth2::RevocationErrorResponseType>,
>;

fn create_client(config: &AuthConfig) -> OauthClient {
    BasicClient::new(
        ClientId::new(config.client_id.clone()),
        None,
        AuthUrl::new(AUTHORIZE_ISSUER.to_string()).unwrap(),
        Some(TokenUrl::new(TOKEN_ISSUER.to_string()).unwrap()),
    )
    .set_auth_type(oauth2::AuthType::RequestBody)
    .set_redirect_uri(RedirectUrl::new(LOCALHOST_CALLBACK.to_string()).unwrap())
}

async fn validate_token(
    token: &StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>,
) -> CharacterInfo {
    let metadata: SsoMetadata = reqwest::get(SSO_META_DATA_URL)
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let jwks: SsoJwkKeys = reqwest::get(metadata.jwks_uri)
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let jwk_set = jwks.keys.iter().find(|x| x.alg == JWK_ALGORITHM).unwrap();

    let character_info = jsonwebtoken::decode::<CharacterInfo>(
        token.access_token().secret(),
        &DecodingKey::from_rsa_components(jwk_set.n.as_ref().unwrap(), jwk_set.e.as_ref().unwrap())
            .unwrap(),
        &Validation::new(jsonwebtoken::Algorithm::RS256),
    )
    .unwrap()
    .claims;

    character_info
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SsoMetadata {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub response_types_supported: Vec<String>,
    pub jwks_uri: String,
    pub revocation_endpoint: String,
    pub revocation_endpoint_auth_methods_supported: Vec<String>,
    pub token_endpoint_auth_methods_supported: Vec<String>,
    pub token_endpoint_auth_signing_alg_values_supported: Vec<String>,
    pub code_challenge_methods_supported: Vec<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SsoJwkKeys {
    pub keys: Vec<SsoKey>,
    #[serde(rename = "SkipUnresolvedJsonWebKeys")]
    pub skip_unresolved_json_web_keys: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SsoKey {
    pub alg: String,
    pub e: Option<String>,
    pub kid: String,
    pub kty: String,
    pub n: Option<String>,
    #[serde(rename = "use")]
    pub use_field: String,
    pub crv: Option<String>,
    pub x: Option<String>,
    pub y: Option<String>,
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
