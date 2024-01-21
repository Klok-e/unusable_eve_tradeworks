mod models;

use self::models::{SsoJwkKeys, SsoMetadata};
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

use self::models::CharacterInfo;

const AUTHORIZE_ISSUER: &str = "https://login.eveonline.com/v2/oauth/authorize";
const TOKEN_ISSUER: &str = "https://login.eveonline.com/v2/oauth/token";
const LOCALHOST_CALLBACK: &str = "http://localhost:8022/callback";
const SSO_META_DATA_URL: &str =
    "https://login.eveonline.com/.well-known/oauth-authorization-server";
const JWK_ALGORITHM: &str = "RS256";
const JWK_ISSUERS: &[&str] = &["login.eveonline.com", "https://login.eveonline.com"];
const JWK_AUDIENCE: &str = "EVE Online";

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
            .load_or_create_json_async(&path, vec![], None, || async {
                Ok(create_auth(request_new_token(config).await).await)
            })
            .await
            .expect("Json load failed");

        if is_token_expired(&data) {
            data = refresh_token(config, data, cache, path).await;
        }

        data
    }
}

async fn refresh_token(
    config: &AuthConfig,
    mut data: Auth,
    cache: &mut CachedStuff,
    path: &str,
) -> Auth {
    let client = create_client(config);
    let token = match client
        .exchange_refresh_token(data.token.refresh_token().unwrap())
        .request_async(async_http_client)
        .await
    {
        Ok(t) => t,
        Err(_) => request_new_token(config).await,
    };

    data = create_auth(token).await;

    cache.save_json(data, &path)
}

fn is_token_expired(data: &Auth) -> bool {
    data.expiration_date < Utc::now()
}

async fn request_new_token(
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

async fn create_auth(token: StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>) -> Auth {
    let expiration_date =
        Utc::now() + chrono::Duration::from_std(token.expires_in().unwrap()).unwrap();
    Auth {
        character_info: validate_token(&token).await,
        token,
        expiration_date,
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

    let mut validation = Validation::new(jsonwebtoken::Algorithm::RS256);
    validation.set_issuer(JWK_ISSUERS);
    validation.set_audience(&[JWK_AUDIENCE]);

    let character_info = jsonwebtoken::decode::<CharacterInfo>(
        token.access_token().secret(),
        &DecodingKey::from_rsa_components(jwk_set.n.as_ref().unwrap(), jwk_set.e.as_ref().unwrap())
            .unwrap(),
        &validation,
    )
    .expect("Couldn't decode token")
    .claims;

    character_info
}
