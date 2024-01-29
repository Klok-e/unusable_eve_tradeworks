use crate::requests::transactions::WalletEsiService;

pub struct ItemsPricesService<'a> {
    item_history: WalletEsiService<'a>,
}

impl<'a> ItemsPricesService<'a> {
    pub fn get_transactions(&self) {
        //todo
    }
}
