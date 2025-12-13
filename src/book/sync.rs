use crate::binance::types::DepthUpdate;

pub struct SyncState {
    last_update_id: Option<u64>,
    buffer: Vec<DepthUpdate>,
}


#[derive(Debug)]
pub enum SyncOutcome {
    Updates(Vec<DepthUpdate>),
    NoUpdates,
    GapBetweenUpdates,
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
    pub fn process_delta(&mut self, update: DepthUpdate) -> SyncOutcome {
        let Some(last_id) = self.last_update_id else {
            //buffers ws updates if haven't processed the depthsnapshot yet
            self.buffer.push(update);
            return SyncOutcome::NoUpdates;
        };

        // discard if fully old
        if update.final_update_id <= last_id {
            return SyncOutcome::NoUpdates;
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
                return SyncOutcome::GapBetweenUpdates;
            }

            // we are ok to apply
            to_apply.push(u);
            expected = to_apply.last().unwrap().final_update_id + 1;
        }

        if let Some(last) = to_apply.last() {
            self.set_last_update_id(last.final_update_id);
        }

        SyncOutcome::Updates(to_apply)
    }

    //caller takes ownership of vec, leaving an empty vec in the struct
    pub fn drain_buffer(&mut self) -> Vec<DepthUpdate> {
        std::mem::take(&mut self.buffer)
    }

}


#[cfg(test)]
mod tests {
    use super::*;

    fn mk_update(first: u64, final_id: u64, event_time: u64) -> DepthUpdate {
        DepthUpdate {
            event_time,
            s: "BTCUSDT".to_string(),
            first_update_id: first,
            final_update_id: final_id,
            b: vec![],
            a: vec![],
        }
    }

    #[test]
    fn buffers_when_last_id_unknown() {
        let mut state = SyncState::new();

        let res = state.process_delta(mk_update(5, 7, 1));

        assert!(matches!(res, SyncOutcome::NoUpdates));
        assert_eq!(state.buffer.len(), 1);
        assert_eq!(state.buffer[0].first_update_id, 5);
    }

    #[test]
    fn discards_fully_old_updates() {
        let mut state = SyncState::new();
        state.set_last_update_id(10);

        let res = state.process_delta(mk_update(5, 9, 1));

        assert!(matches!(res, SyncOutcome::NoUpdates));
        assert_eq!(state.last_update_id, Some(10));
        assert!(state.buffer.is_empty());
    }

    #[test]
    fn applies_buffered_then_current_in_order() {
        let mut state = SyncState::new();

        state.process_delta(mk_update(9, 10, 2)); // buffered while unknown last id
        state.process_delta(mk_update(6, 8, 1));

        state.set_last_update_id(5);

        let applied = match state.process_delta(mk_update(11, 12, 3)) {
            SyncOutcome::Updates(u) => u,
            other => panic!("expected updates, got {other:?}"),
        };

        assert_eq!(applied.len(), 3);
        assert_eq!(applied[0].first_update_id, 6);
        assert_eq!(applied[1].first_update_id, 9);
        assert_eq!(applied[2].first_update_id, 11);
        assert_eq!(state.last_update_id, Some(12));
        assert!(state.buffer.is_empty());
    }

    #[test]
    fn skips_stale_buffered_chunks() {
        let mut state = SyncState::new();

        state.process_delta(mk_update(7, 9, 1));
        state.set_last_update_id(10);

        let applied = match state.process_delta(mk_update(11, 12, 2)) {
            SyncOutcome::Updates(u) => u,
            other => panic!("expected updates, got {other:?}"),
        };

        assert_eq!(applied.len(), 1);
        assert_eq!(applied[0].first_update_id, 11);
        assert_eq!(state.last_update_id, Some(12));
        assert!(state.buffer.is_empty());
    }

    #[test]
    fn errors_on_gap_between_updates() {
        let mut state = SyncState::new();
        state.set_last_update_id(10);

        let outcome = state.process_delta(mk_update(12, 13, 1));

        assert!(matches!(outcome, SyncOutcome::GapBetweenUpdates));
        assert_eq!(state.last_update_id, Some(10));
        assert!(state.buffer.is_empty());
    }
}