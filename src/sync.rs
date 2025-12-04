use anyhow::{Result, bail};
use crate::binance::DepthUpdate;

pub struct SyncState {
    snapshot_update_id: Option<u64>,
    buffer: Vec<DepthUpdate>,
}


impl SyncState {
    pub fn new() -> Self {
        Self {
            snapshot_update_id: None,
            buffer: Vec::new(),
        }
    }

    pub fn set_snapshot(&mut self, last_update_id: u64) {
        self.snapshot_update_id = Some(last_update_id);
    }

    //return true if delta should be applied
    pub fn process_delta(&mut self, update: DepthUpdate) -> Result<bool>{
        let Some(snapshot_id) = self.snapshot_update_id else {
            self.buffer.push(update);
            return Ok(false);
        };

        //check sync condition U <= lastUpdateId + 1 <= u
        if update.first_update_id <= snapshot_id + 1 && snapshot_id + 1 <= update.final_update_id {
            self.buffer.clear();
            return Ok(true);
        }

        if update.final_update_id <= snapshot_id {
            //dont need this is old data
            return Ok(false);
        }

        if update.first_update_id > snapshot_id + 1 {
            //fuck missed an update lets crash the whole thing
            bail!("Gap between updates! expected {}, got {}", snapshot_id + 1, update.first_update_id)
        }

        //this update is future data, we shan't update yet and shall wait for snapshot_id + 1
        self.buffer.push(update);
        Ok(false)
    }

    //caller takes ownership of vec, leaving an empty vec in the struct
    pub fn drain_buffer(&mut self) -> Vec<DepthUpdate> {
        std::mem::take(&mut self.buffer)
    }

}