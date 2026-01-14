#![allow(missing_docs)]
// Copyright (c) 2026 Amunchain
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//     http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
#![forbid(unsafe_code)]

//! Consensus driver wiring for inbound messages.

use crate::core::consensus::tide::{NoopSlashing, TideConfig, TideFinalizer};
use crate::core::types::{ConsensusMsg, ValidatorId};
use std::collections::BTreeSet;
use thiserror::Error;

/// Driver errors.
#[derive(Debug, Error)]
pub enum DriverError {
    #[error("invalid validator set")]
    InvalidValidators,
}

/// Top-level consensus driver.
pub struct ConsensusDriver {
    /// Tide finality gadget.
    pub tide: TideFinalizer<NoopSlashing>,
}

impl ConsensusDriver {
    /// Create new driver.
    pub fn new(validators: BTreeSet<ValidatorId>) -> Result<Self, DriverError> {
        if validators.is_empty() {
            return Err(DriverError::InvalidValidators);
        }
        let cfg = TideConfig::new(validators);
        Ok(Self {
            tide: TideFinalizer::new(cfg, NoopSlashing),
        })
    }

    /// Handle inbound consensus message.
    pub fn on_msg(&mut self, msg: ConsensusMsg) {
        match msg {
            ConsensusMsg::Vote(v) => {
                let _ = self.tide.process_vote_verified(v);
            }
            ConsensusMsg::Commit(c) => {
                let _ = self.tide.process_commit_verified(c);
            }
        }
    }
}
