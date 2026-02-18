//! Decision logging for deterministic replay.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Decision {
    RandU64 { value: u64 },
    RandBytes { hex: String },
    TimeSleepMs { ms: u64 },
    TimeAdvanceMs { ms: u64 },
    Step { index: usize, name: String },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DecisionLog {
    pub decisions: Vec<Decision>,
}

impl DecisionLog {
    pub fn push(&mut self, decision: Decision) {
        self.decisions.push(decision);
    }
}

#[derive(Debug)]
pub struct DecisionCursor<'a> {
    decisions: &'a [Decision],
    index: usize,
}

impl<'a> DecisionCursor<'a> {
    pub fn new(decisions: &'a [Decision]) -> Self {
        Self { decisions, index: 0 }
    }

    pub fn next(&mut self) -> Option<&'a Decision> {
        let d = self.decisions.get(self.index);
        self.index = self.index.saturating_add(1);
        d
    }

    pub fn remaining(&self) -> usize {
        self.decisions.len().saturating_sub(self.index)
    }
}

