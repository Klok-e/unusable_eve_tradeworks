use anyhow::anyhow;
use rust_eveonline_esi::{
    apis::{
        configuration::Configuration,
        wallet_api::{self, GetCharactersCharacterIdWalletTransactionsParams},
    },
    models::GetCharactersCharacterIdWalletTransactions200Ok,
};

use super::{error::EsiApiError, paged_all::OnlyOk, retry::retry_smart};

pub struct WalletEsiService<'a> {
    pub esi_config: &'a Configuration,
}
impl<'a> WalletEsiService<'a> {
    pub async fn get_transactions_history(
        &self,
        character_id: i32,
    ) -> anyhow::Result<Vec<GetCharactersCharacterIdWalletTransactions200Ok>> {
        let transactions = retry_smart(|| async {
            Ok::<_, EsiApiError>(super::retry::RetryResult::Success(
                wallet_api::get_characters_character_id_wallet_transactions(
                    self.esi_config,
                    GetCharactersCharacterIdWalletTransactionsParams {
                        character_id,
                        from_id: None,
                        datasource: None,
                        if_none_match: None,
                        token: None,
                    },
                )
                .await?
                .entity
                .unwrap()
                .into_ok()
                .unwrap(),
            ))
        })
        .await?
        .ok_or(anyhow!(
            "Couldn't load wallet transactions after multiple retries"
        ))?;

        Ok(transactions)
    }
}
