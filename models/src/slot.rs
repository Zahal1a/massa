use crate::error::ModelsError;
use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, fmt};

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct Slot {
    pub period: u64,
    pub thread: u8,
}

impl PartialOrd for Slot {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        (self.period, self.thread).partial_cmp(&(other.period, other.thread))
    }
}

impl Ord for Slot {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.period, self.thread).cmp(&(other.period, other.thread))
    }
}

impl fmt::Display for Slot {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "(period: {}, thread: {})", self.period, self.thread)
    }
}

impl Slot {
    pub fn new(period: u64, thread: u8) -> Slot {
        Slot { period, thread }
    }

    pub fn to_bytes(&self) -> [u8; 9] {
        let mut res = [0u8; 9];
        res[..8].clone_from_slice(&self.period.to_be_bytes());
        res[8] = self.thread;
        res
    }

    /// Returns the next Slot
    pub fn get_next_slot(&self, thread_count: u8) -> Result<Slot, ModelsError> {
        if self.thread.saturating_add(1u8) >= thread_count {
            Ok(Slot::new(
                self.period
                    .checked_add(1u64)
                    .ok_or(ModelsError::SlotOverflowError)?,
                0u8,
            ))
        } else {
            Ok(Slot::new(
                self.period,
                self.thread
                    .checked_add(1u8)
                    .ok_or(ModelsError::ThreadOverflowError)?,
            ))
        }
    }
}
