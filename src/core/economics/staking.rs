#![allow(missing_docs)]
// Copyright (c) 2026 Amunchain
// Licensed under the Apache License, Version 2.0

//! Deterministic staking ledger: bonding, unbonding, slashing, rewards.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use thiserror::Error;

const SECONDS_PER_DAY: u64 = 86_400;
const MIN_UNBONDING_DAYS: u64 = 7;

#[derive(Debug, Error)]
pub enum StakingError {
    #[error("invalid amount")]
    InvalidAmount,
    #[error("insufficient stake")]
    InsufficientStake,
}

#[derive(Clone, Debug)]
pub struct Delegation {
    pub amount: u128,
}

#[derive(Clone, Debug)]
pub struct UnbondingEntry {
    pub amount: u128,
    pub unlock_time: u64,
}

#[derive(Clone, Debug, Default)]
pub struct Validator {
    pub commission_bps: u16,
    pub self_stake: u128,
    pub slashed: u128,
}

#[derive(Clone, Debug, Default)]
pub struct StakingLedger {
    /// Registered validators keyed by validator id bytes.
    pub validators: BTreeMap<Vec<u8>, Validator>,
    /// Delegations keyed by (delegator, validator).
    pub delegations: BTreeMap<(Vec<u8>, Vec<u8>), Delegation>,
    /// Pending unbonding entries keyed by (delegator, validator).
    pub unbonding: BTreeMap<(Vec<u8>, Vec<u8>), Vec<UnbondingEntry>>,
}

impl StakingLedger {
    /// Bond stake from a delegator to a validator.
    pub fn bond(
        &mut self,
        delegator: Vec<u8>,
        validator: Vec<u8>,
        amount: u128,
    ) -> Result<(), StakingError> {
        if amount == 0 {
            return Err(StakingError::InvalidAmount);
        }
        let key = (delegator, validator);
        let entry = self
            .delegations
            .entry(key)
            .or_insert(Delegation { amount: 0 });
        entry.amount = entry.amount.saturating_add(amount);
        Ok(())
    }

    /// Start unbonding: decreases delegation and creates a timed unbonding entry.
    pub fn begin_unbond(
        &mut self,
        delegator: Vec<u8>,
        validator: Vec<u8>,
        amount: u128,
        now_unix: u64,
    ) -> Result<(), StakingError> {
        if amount == 0 {
            return Err(StakingError::InvalidAmount);
        }
        let key = (delegator.clone(), validator.clone());
        let del = self
            .delegations
            .get_mut(&key)
            .ok_or(StakingError::InsufficientStake)?;
        if del.amount < amount {
            return Err(StakingError::InsufficientStake);
        }
        del.amount -= amount;

        let unlock_time =
            now_unix.saturating_add(MIN_UNBONDING_DAYS.saturating_mul(SECONDS_PER_DAY));
        let ub = self.unbonding.entry(key).or_default();
        ub.push(UnbondingEntry {
            amount,
            unlock_time,
        });
        Ok(())
    }

    /// Finalize matured unbonding entries and return released amount.
    pub fn finalize_unbond(
        &mut self,
        delegator: Vec<u8>,
        validator: Vec<u8>,
        now_unix: u64,
    ) -> Result<u128, StakingError> {
        let key = (delegator, validator);
        let Some(list) = self.unbonding.get_mut(&key) else {
            return Ok(0);
        };

        let mut released: u128 = 0;
        let mut remaining: Vec<UnbondingEntry> = Vec::with_capacity(list.len());
        for e in list.iter() {
            if now_unix >= e.unlock_time {
                released = released.saturating_add(e.amount);
            } else {
                remaining.push(e.clone());
            }
        }
        *list = remaining;
        Ok(released)
    }

    /// Apply slashing to all delegations to a validator by fraction in bps (0..=10000).
    pub fn slash_validator(&mut self, validator: &[u8], fraction_bps: u16) -> u128 {
        let frac = (fraction_bps.min(10_000)) as u128;
        let mut total_slashed: u128 = 0;

        for ((_, v), del) in self.delegations.iter_mut() {
            if v.as_slice() == validator {
                let sl = del.amount.saturating_mul(frac) / 10_000u128;
                del.amount = del.amount.saturating_sub(sl);
                total_slashed = total_slashed.saturating_add(sl);
            }
        }

        if let Some(val) = self.validators.get_mut(validator) {
            val.slashed = val.slashed.saturating_add(total_slashed);
        }
        total_slashed
    }

    /// Distribute rewards proportional to stake to delegators of a validator.
    pub fn distribute_rewards(&mut self, validator: &[u8], total_reward: u128) {
        if total_reward == 0 {
            return;
        }

        let mut total_stake: u128 = 0;
        for ((_, v), del) in self.delegations.iter() {
            if v.as_slice() == validator {
                total_stake = total_stake.saturating_add(del.amount);
            }
        }
        if total_stake == 0 {
            return;
        }

        for ((_, v), del) in self.delegations.iter_mut() {
            if v.as_slice() == validator {
                let share = total_reward.saturating_mul(del.amount) / total_stake;
                del.amount = del.amount.saturating_add(share);
            }
        }
    }
}
