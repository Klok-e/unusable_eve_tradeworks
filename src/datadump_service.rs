use rusqlite::{Connection, Result};

pub struct DatadumpService {
    conn: Connection,
}

impl DatadumpService {
    pub fn new(conn: Connection) -> Self {
        Self { conn }
    }

    pub fn get_all_group_id_with_root_name(&self, name: &str) -> Result<Vec<i32>> {
        let group_id = self.conn.query_row(
            "SELECT marketGroupID FROM invMarketGroups WHERE marketGroupName like '%' || ? || '%' and parentGroupID  is NULL",
            [name,],
            |row| row.get(0),
        )?;
        let children = self.get_child_groups_parent(group_id)?;
        Ok(children)
    }

    pub fn get_child_groups_parent(&self, parent_id: i32) -> Result<Vec<i32>> {
        let mut statement = self.conn.prepare(
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

    pub fn get_reprocess_items(&self, item_id: i32) -> Result<ReprocessItemInfo> {
        let mut statement = self.conn.prepare(
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
        Ok(ReprocessItemInfo {
            item_id,
            reprocessed_into: groups,
        })
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
    pub quantity: i32,
}
