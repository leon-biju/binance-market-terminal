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

    // returns list of updates to apply
    pub fn process_delta(&mut self, update: DepthUpdate) -> Result<Option<Vec<DepthUpdate>>> {
        let Some(last_id) = self.last_update_id else {
            self.buffer.push(update);
            return Ok(None);
        };

        // discard if fully old
        if update.final_update_id <= last_id {
            return Ok(None);
        }

        // collect buffered + current, oldest first
        let mut candidates = self.drain_buffer();
        candidates.push(update);
        candidates.sort_by_key(|u| u.first_update_id);

        let mut to_apply = Vec::new();
        let mut expected = last_id + 1;

        for u in candidates {
            // skip stale chunks
            if u.final_update_id < expected {
                continue;
            }
            // require contiguity
            if u.first_update_id > expected {
                //To handle
                bail!("Gap between updates! expected {}, got {}", expected, u.first_update_id);
            }
            // ok to apply
            to_apply.push(u);
            expected = to_apply.last().unwrap().final_update_id + 1;
        }

        if let Some(last) = to_apply.last() {
            self.set_last_update_id(last.final_update_id);
        }

        Ok(Some(to_apply))
    }

    //caller takes ownership of vec, leaving an empty vec in the struct
    pub fn drain_buffer(&mut self) -> Vec<DepthUpdate> {
        std::mem::take(&mut self.buffer)
    }

}