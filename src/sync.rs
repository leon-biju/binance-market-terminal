use anyhow::{Result, bail};
use crate::binance::DepthUpdate;

pub struct SyncState {
    last_update_id: Option<u64>,
    buffer: Vec<DepthUpdate>,
}


impl SyncState {
    pub fn new() -> Self {
        Self {
            last_update_id: None,
            buffer: Vec::new(),
        }
    }

    pub fn set_last_update_id(&mut self, last_update_id: u64) {
        self.last_update_id = Some(last_update_id);
    }

    //return list of updates to apply
    pub fn process_delta(&mut self, update: DepthUpdate) -> Result<Vec<DepthUpdate>>{
        let Some(last_id) = self.last_update_id else {
            //still waiting for snapshot lets buffer this update
            self.buffer.push(update);
            return Ok(Vec::new());
        };

        //check sync condition U <= lastUpdateId + 1 <= u
        if update.first_update_id <= last_id + 1 && last_id + 1 <= update.final_update_id {
            //apply buffered data first
            let mut to_apply = vec![update];
            self.buffer.sort_by_key(|u| u.first_update_id);
            let buffered = self.drain_buffer();

            for buffered_update in buffered {
                if buffered_update.first_update_id > last_id {
                    to_apply.push(buffered_update);
                }
            }

            return Ok(to_apply);
        }

        if update.final_update_id <= last_id {
            //dont need this is old data
            return Ok(Vec::new());
        }

        if update.first_update_id > last_id + 1 {
            //fuck missed an update lets crash the whole thing
            bail!("Gap between updates! expected {}, got {}", last_id + 1, update.first_update_id)
        }

        //this update is future data, we shan't update yet and shall wait for snapshot_id + 1
        self.buffer.push(update);
        Ok(Vec::new())
    }

    //caller takes ownership of vec, leaving an empty vec in the struct
    pub fn drain_buffer(&mut self) -> Vec<DepthUpdate> {
        std::mem::take(&mut self.buffer)
    }

}