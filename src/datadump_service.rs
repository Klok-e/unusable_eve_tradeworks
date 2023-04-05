use std::sync::{Arc, Mutex};

use rusqlite::{Connection, Result};

#[derive(Debug)]
pub struct DatadumpService {
    conn: Arc<Mutex<Connection>>,
}

impl DatadumpService {
    pub fn new(conn: Connection) -> Self {
        Self {
            conn: Arc::new(Mutex::new(conn)),
        }
    }

    pub fn get_all_group_id_with_root_name(&self, name: &str) -> Result<Vec<i32>> {
        let group_id = self.conn.lock().unwrap().query_row(
            "SELECT marketGroupID FROM invMarketGroups WHERE marketGroupName like '%' || ? || '%' and parentGroupID  is NULL",
            [name,],
            |row| row.get(0),
        )?;
        let children = self.get_child_groups_parent(group_id)?;
        Ok(children)
    }

    pub fn get_child_groups_parent(&self, parent_id: i32) -> Result<Vec<i32>> {
        let connection = self.conn.lock().unwrap();
        let mut statement = connection.prepare(
            "SELECT 
                        marketGroupID 
                    FROM 
                        invMarketGroups 
                    WHERE 
                        parentGroupID = ?",
        )?;
        let groups = statement.query([parent_id])?;
        let groups = groups.mapped(|x| x.get(0)).collect::<Result<Vec<_>>>()?;

        let groups = groups
            .iter()
            .map(|&group| {
                self.get_child_groups_parent(group)
                    .map(|x| x.into_iter().chain(std::iter::once(group)))
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect();

        Ok(groups)
    }

    pub fn get_reprocess_items(&self, item_id: i32) -> anyhow::Result<ReprocessItemInfo> {
        let reprocess_into = self.get_reprocess_into(item_id)?;

        Ok(ReprocessItemInfo {
            item_id,
            reprocessed_into: reprocess_into,
        })
    }

    fn get_reprocess_into(&self, item_id: i32) -> Result<Vec<ReprocessInfo>, anyhow::Error> {
        let connection = self.conn.lock().unwrap();
        let mut statement = connection.prepare(
            "SELECT 
                        materialTypeID, quantity 
                    FROM 
                        invTypeMaterials itm 
                    WHERE 
                        typeID = ?",
        )?;
        let groups = statement.query([item_id])?;
        let groups = groups
            .mapped(|x| {
                Ok(ReprocessInfo {
                    item_id: x.get(0)?,
                    quantity: x.get(1)?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(groups)
    }

    // pub fn get_reprocess_quantity(&self, item_id: i32) -> anyhow::Result<i64> {
    //     let lock = self.conn.lock().unwrap();
    //     let mut statement = lock.prepare(
    //         "SELECT quantity
    //                 FROM
    //                     industryActivityProducts iap
    //                 WHERE
    //                     productTypeID = ?",
    //     )?;
    //     let groups = statement.query([item_id])?;
    //     let groups = groups
    //         .mapped(|x| x.get(0))
    //         .next()
    //         .transpose()?
    //         .ok_or_else(|| {
    //             anyhow::anyhow!("get_reprocess_quantity: No rows returned for {item_id}")
    //         })?;
    //     Ok(groups)
    // }
}

#[derive(Debug)]
pub struct ReprocessItemInfo {
    pub reprocessed_into: Vec<ReprocessInfo>,
    pub item_id: i32,
}

#[derive(Debug)]
pub struct ReprocessInfo {
    pub item_id: i32,
    pub quantity: i64,
}
