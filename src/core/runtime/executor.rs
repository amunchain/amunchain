#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(clippy::all)]
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

//! EVM executor placeholder.
//!
//! The `revm` crate is included as a dependency and can be wired into `PersistentState`
//! via a proper `StateProvider` + commit path. This file intentionally provides a
//! minimal compile-safe interface.

use thiserror::Error;

/// Execution error.
#[derive(Debug, Error)]
pub enum ExecError {
    #[error("not implemented")]
    NotImplemented,
}

/// EVM executor.
#[derive(Clone, Debug, Default)]
pub struct EvmExecutor;

impl EvmExecutor {
    /// Create a new executor.
    pub fn new() -> Self {
        Self
    }

    /// Execute a transaction (placeholder).
    pub fn execute(&self) -> Result<(), ExecError> {
        Err(ExecError::NotImplemented)
    }
}
