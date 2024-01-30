use anyhow::Ok;
use chrono::Duration;
use rust_eveonline_esi::models::GetCharactersCharacterIdWalletTransactions200Ok;

use crate::{cached_data::CachedStuff, requests::transactions::WalletEsiService};

pub struct ItemsPricesService<'a> {
    pub item_history: WalletEsiService<'a>,
    pub cache: &'a mut CachedStuff,
}

impl<'a> ItemsPricesService<'a> {
    pub async fn get_prices_for_items(
        &mut self,
        character_id: i32,
        
    ) -> anyhow::Result<ItemsSellPrices> {
        let transactions = self.download_or_load_transactions(character_id).await?;

        Ok(ItemsSellPrices { items: vec![] })
    }

    pub async fn download_or_load_transactions(
        &mut self,
        character_id: i32,
    ) -> anyhow::Result<Vec<GetCharactersCharacterIdWalletTransactions200Ok>> {
        let transactions = self
            .cache
            .load_or_create_async(
                format!("wallet-history-{character_id}.rmp"),
                vec![],
                Some(Duration::seconds(10 * 60)),
                |mut previous| {
                    let item_hist = &mut self.item_history;
                    async move {
                        let mut new_transactions =
                            item_hist.get_transactions_history(character_id).await?;

                        if let Some(ref mut previous) = previous {
                            new_transactions.append(previous)
                        }

                        new_transactions.sort_unstable_by_key(|x| x.transaction_id);
                        new_transactions.dedup_by_key(|x| x.transaction_id);

                        Ok(new_transactions)
                    }
                },
            )
            .await?;

        Ok(transactions)
    }
}

pub struct 

pub struct ItemsSellPrices {
    pub items: Vec<ItemSellPrice>,
}

pub struct ItemSellPrice {
    pub price: f64,
    pub item_id: i32,
}
