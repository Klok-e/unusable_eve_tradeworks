use rust_eveonline_esi::apis::{
    character_api,
    configuration::Configuration,
    wallet_api::{self, GetCharactersCharacterIdWalletTransactionsParams},
};

pub struct WalletEsiService<'a> {
    pub config: &'a Configuration,
}
impl<'a> WalletEsiService<'a> {
    pub fn new(config: &'a Configuration) -> Self {
        Self { config }
    }

    pub async fn get_transactions_history(&self, character_id: i32) -> anyhow::Result<()> {
        wallet_api::get_characters_character_id_wallet_transactions(
            self.config,
            GetCharactersCharacterIdWalletTransactionsParams {
                character_id: character_id,
                from_id: None,
                datasource: None,
                if_none_match: None,
                token: None,
            },
        )
        .await;
        Ok(())
    }
}
