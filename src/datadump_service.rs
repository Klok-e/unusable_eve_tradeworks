use std::sync::{Arc, Mutex};

use anyhow::anyhow;
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

    pub fn get_group_ids_for_groups(
        &self,
        groups: &Vec<String>,
    ) -> Result<Vec<i32>, anyhow::Error> {
        log::debug!("get_group_ids_for_groups {groups:?}");
        let groups = groups
            .iter()
            .map(|name| {
                let children = self.get_all_group_id_with_root_name(name.as_str())?;

                Ok(children)
            })
            .collect::<anyhow::Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        Ok(groups)
    }

    pub fn get_all_group_id_with_root_name(&self, name: &str) -> anyhow::Result<Vec<i32>> {
        let group_ids: Vec<i32> = {
            let connection = self.conn.lock().unwrap();
            let mut statement = connection
                .prepare("SELECT marketGroupID FROM invMarketGroups WHERE description = ? or marketGroupName = ?")?;
            let groups = statement.query([name, name])?;
            groups.mapped(|x| x.get(0)).collect::<Result<Vec<_>>>()?
        };
        if group_ids.len() > 1 {
            log::warn!("Multiple group ids found for item {name}: {group_ids:?}");
        }

        let group_id = group_ids
            .first()
            .ok_or_else(|| anyhow!("No group id found for name {name}"))?;

        log::debug!("group_id of group {name} is {group_id}");

        let children = self.get_child_groups_parent(*group_id)?;
        Ok(children)
    }

    pub fn get_child_groups_parent(&self, parent_id: i32) -> Result<Vec<i32>> {
        let groups = {
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
            groups.mapped(|x| x.get(0)).collect::<Result<Vec<_>>>()?
        };
        log::debug!("children of group {parent_id} are: {groups:?}");

        let groups = groups
            .iter()
            .map(|&group| self.get_child_groups_parent(group))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .chain(std::iter::once(parent_id))
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
