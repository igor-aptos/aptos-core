// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use crate::{
    bounded_math::{code_invariant_error, expect_ok, ok_overflow, BoundedMath, SignedU128},
    delta_math::DeltaHistory,
    resolver::{AggregatorReadMode, AggregatorResolver},
};
use aptos_types::{
    state_store::{state_key::StateKey, table::TableHandle},
    vm_status::StatusCode,
};
use move_binary_format::errors::{PartialVMError, PartialVMResult};
use move_core_types::account_address::AccountAddress;
use std::collections::{btree_map::Entry, BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct AggregatorHandle(pub AccountAddress);

/// Uniquely identifies an aggregator or aggregator snapshot instance in storage.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum AggregatorID {
    // Aggregator V1 is implemented as a state item, and so can be queried by
    // the state key.
    Legacy(StateKey),
    // Aggregator V2 is embedded into resources, and uses ephemeral identifiers
    // which are unique per block.
    Ephemeral(u64),
}

impl AggregatorID {
    pub fn legacy(handle: TableHandle, key: AggregatorHandle) -> Self {
        let state_key = StateKey::table_item(handle, key.0.to_vec());
        AggregatorID::Legacy(state_key)
    }

    pub fn ephemeral(id: u64) -> Self {
        AggregatorID::Ephemeral(id)
    }

    pub fn as_state_key(&self) -> Option<&StateKey> {
        match self {
            Self::Legacy(state_key) => Some(state_key),
            Self::Ephemeral(_) => None,
        }
    }

    pub fn into_state_key(self) -> Option<StateKey> {
        match self {
            Self::Legacy(state_key) => Some(state_key),
            Self::Ephemeral(_) => None,
        }
    }
}

/// Describes how the `speculative_start_value` in `AggregatorState` was obtained.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpeculativeStartValue {
    // The speculative_start_value is not yet initialized
    Unset,
    // The speculative_start_value was obtained by reading
    // the last committed value of the aggregator from MVHashmap.
    // WARNING: any use of this value should be captured as a restriction
    // in the change set, as value received here is not track as part of the
    // read conflict!!
    // Only current restriction is DeltaHistory, and only correct usage is
    // that can be returned to the caller is via try_add/try_sub methods!
    LastCommittedValue(u128),
    // The speculative_start_value was obtained by performing a read
    // procedure on the aggregator, which involves aggregating deltas
    // present at the read time
    AggregatedValue(u128),
}

impl SpeculativeStartValue {
    // WARNING: any use of this value should be captured as a restriction
    // in the change set, as value received here is not track as part of the
    // read conflict!!
    // Only current restriction is DeltaHistory, and only correct usage is
    // that can be returned to the caller is via try_add/try_sub methods!
    pub fn get_any_value(&self) -> PartialVMResult<u128> {
        match self {
            SpeculativeStartValue::Unset => Err(code_invariant_error(
                "Tried calling get_any_value on Unset speculative value",
            )),
            SpeculativeStartValue::LastCommittedValue(value) => Ok(*value),
            SpeculativeStartValue::AggregatedValue(value) => Ok(*value),
        }
    }

    pub fn get_value_for_read(&self) -> PartialVMResult<u128> {
        match self {
            SpeculativeStartValue::Unset => Err(code_invariant_error(
                "Tried calling get_value_for_read on Unset speculative value",
            )),
            SpeculativeStartValue::LastCommittedValue(_) => Err(code_invariant_error(
                "Tried calling get_value_for_read on LastCommittedValue speculative value",
            )),
            SpeculativeStartValue::AggregatedValue(value) => Ok(*value),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DerivedFormula {
    Concat { prefix: Vec<u8>, suffix: Vec<u8> },
}

impl DerivedFormula {
    pub fn apply(&self, base: &SnapshotValue) -> SnapshotValue {
        match self {
            DerivedFormula::Concat { prefix, suffix } => {
                let mut result = prefix.clone();
                match base {
                    SnapshotValue::Integer(value) => result.extend(value.to_string().as_bytes()),
                    SnapshotValue::String(value) => result.extend(value),
                };
                result.extend(suffix);
                SnapshotValue::String(result)
            },
        }
    }
}

/// Describes the state of each aggregator instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AggregatorState {
    // If aggregator stores a known value.
    Data {
        value: u128,
    },
    Delta {
        speculative_start_value: SpeculativeStartValue,
        delta: SignedU128,
        history: DeltaHistory,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotValue {
    Integer(u128),
    String(Vec<u8>),
}

impl SnapshotValue {
    pub fn into_aggregator_value(self) -> PartialVMResult<u128> {
        match self {
            SnapshotValue::Integer(value) => Ok(value),
            SnapshotValue::String(_) => Err(code_invariant_error(
                "Tried calling into_aggregator_value on String SnapshotValue",
            )),
        }
    }
}

// Aggregator snapshot is immutable struct, once created - value is fixed.
// If we want to provide mutability APIs in the future, it should be
// copy-on-write - i.e. a new aggregator_id should be created for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AggregatorSnapshotState {
    // Created in this transaction, with explicit value
    Data {
        value: SnapshotValue,
    },
    // Created in this transaction, via snapshot(&aggregator)
    Delta {
        base_aggregator: AggregatorID,
        delta: SignedU128,
    },
    // Created in this transaction, via string_concat(prefix, &snapshot, suffix)
    Concat {
        base_snapshot: AggregatorID,
        formula: DerivedFormula,
    },
    // Accessed in this transaction, based on the ID
    Reference {
        // always expensive/aggregated read
        speculative_value: SnapshotValue,
    },
}

#[derive(Debug)]
pub struct AggregatorSnapshot {
    // The identifier used to identify the aggregator.
    #[allow(dead_code)]
    id: AggregatorID,

    state: AggregatorSnapshotState,
}

impl AggregatorSnapshot {
    pub fn into(self) -> AggregatorSnapshotState {
        self.state
    }
}

/// Internal aggregator data structure.
#[derive(Debug)]
pub struct Aggregator {
    // The identifier used to identify the aggregator.
    id: AggregatorID,
    // Describes an upper bound of an aggregator. If value of the aggregator
    // exceeds it, the aggregator overflows.
    // TODO: Currently this is a single u128 value since we use 0 as a trivial
    // lower bound. If we want to support custom lower bounds, or have more
    // complex postconditions, we should factor this out in its own struct.
    max_value: u128,
    // Describes a state of an aggregator.
    state: AggregatorState,
}

impl Aggregator {
    #[cfg(test)]
    pub fn get_history(&self) -> Option<&DeltaHistory> {
        match &self.state {
            AggregatorState::Data { .. } => None,
            AggregatorState::Delta { history, .. } => Some(history),
        }
    }

    /// Returns error if transaction is in invalid state, and should be re-executed.
    /// Returns true if addition succeeded, and false if it would overflow.
    pub fn try_add(
        &mut self,
        resolver: &dyn AggregatorResolver,
        input: u128,
    ) -> PartialVMResult<bool> {
        if input > self.max_value {
            // We do not have to record the overflow.
            // We record the delta that result in overflows/underflows so that when we compute the actual value
            // of aggregator, we can figure out if the output of try_add/try_sub changes.
            // When input exceeds max_value, we know that no matter what the starting value of the
            // aggregator is, it always results in an overflow.
            return Ok(false);
        }
        let math = BoundedMath::new(self.max_value);
        self.read_last_committed_aggregator_value(resolver)?;
        match &mut self.state {
            AggregatorState::Data { value } => {
                // If aggregator knows the value, add directly and keep the state.
                match math.unsigned_add(*value, input) {
                    Ok(new_value) => {
                        *value = new_value;
                        Ok(true)
                    },
                    Err(_) => Ok(false),
                }
            },
            AggregatorState::Delta {
                speculative_start_value,
                delta,
                history,
            } => {
                let cur_value = expect_ok(
                    math.unsigned_add_delta(speculative_start_value.get_any_value()?, delta),
                )?;

                if math.unsigned_add(cur_value, input).is_err() {
                    let overflow_delta =
                        expect_ok(ok_overflow(math.unsigned_add_delta(input, delta)))?;

                    // if value overflowed, we don't need to record it
                    if let Some(overflow_delta) = overflow_delta {
                        history.record_overflow(overflow_delta);
                    }
                    Ok(false)
                } else {
                    let new_delta =
                        expect_ok(math.signed_add(delta, &SignedU128::Positive(input)))?;
                    *delta = new_delta;
                    history.record_success(new_delta);
                    Ok(true)
                }
            },
        }
    }

    /// Returns error if transaction is in invalid state, and should be re-executed.
    /// Returns true if subtraction succeeded, and false if it would underflow.
    pub fn try_sub(
        &mut self,
        resolver: &dyn AggregatorResolver,
        input: u128,
    ) -> PartialVMResult<bool> {
        if input > self.max_value {
            // We do not have to record the underflow.
            // We record the delta that result in overflows/underflows so that when we compute the actual value
            // of aggregator, we can figure out if the output of try_add/try_sub changes.
            // When input exceeds max_value, we know that no matter what the starting value of the
            // aggregator is, it always results in an underflow.
            return Ok(false);
        }
        let math = BoundedMath::new(self.max_value);
        self.read_last_committed_aggregator_value(resolver)?;
        match &mut self.state {
            AggregatorState::Data { value } => {
                // If aggregator knows the value, add directly and keep the state.
                match math.unsigned_subtract(*value, input) {
                    Ok(new_value) => {
                        *value = new_value;
                        Ok(true)
                    },
                    Err(_) => Ok(false),
                }
            },
            AggregatorState::Delta {
                speculative_start_value,
                delta,
                history,
            } => {
                let cur_value = expect_ok(
                    math.unsigned_add_delta(speculative_start_value.get_any_value()?, delta),
                )?;

                if cur_value < input {
                    let underflow_delta =
                        expect_ok(ok_overflow(math.unsigned_add_delta(input, &delta.minus())))?;
                    // If value overflowed (delta was smaller than -max_value), we don't need to record it.
                    if let Some(underflow_delta) = underflow_delta {
                        history.record_underflow(underflow_delta);
                    }
                    Ok(false)
                } else {
                    let new_delta =
                        expect_ok(math.signed_add(delta, &SignedU128::Negative(input)))?;
                    *delta = new_delta;
                    history.record_success(new_delta);
                    Ok(true)
                }
            },
        }
    }

    /// Implements logic for doing a "cheap read" of an aggregator.
    /// Reads the last committed value of the aggregator that's known at the
    /// time of the call, and as such, can be computed efficiently (i.e. no
    /// need to consider any speculative state, deltas, etc)
    /// This method has a sideffect, of updating `speculative_start_value` with
    /// `LastCommittedValue` variant.
    /// `get_any_value()` is guaranteed to succeed after this call.
    /// This needs to be called before updating aggregator with delta's, i.e. if
    /// aggregator is in Delta state, delta should be 0, and history should be empty.
    pub fn read_last_committed_aggregator_value(
        &mut self,
        resolver: &dyn AggregatorResolver,
    ) -> PartialVMResult<()> {
        if let AggregatorState::Delta {
            speculative_start_value,
            delta,
            history,
        } = &mut self.state
        {
            // If value is Unset, we read it
            if let SpeculativeStartValue::Unset = speculative_start_value {
                if delta.is_zero() || !history.is_empty() {
                    return Err(code_invariant_error(
                        "Delta or history not empty with Unset speculative value",
                    ));
                }

                let maybe_value_from_storage = match &self.id {
                    AggregatorID::Legacy(state_key) => resolver
                        .get_aggregator_v1_value(state_key, AggregatorReadMode::LastCommitted),
                    // TODO: use integers directly, or some wrapped type.
                    id => resolver
                        .get_aggregator_v2_value(id, AggregatorReadMode::LastCommitted)
                        .and_then(|v| Ok(v.into_aggregator_value()?))
                        .map(Some),
                };
                let value_from_storage = maybe_value_from_storage
                    .map_err(|e| {
                        extension_error(format!(
                            "Could not find the value of the aggregator: {}",
                            e
                        ))
                    })?
                    .ok_or_else(|| {
                        extension_error(format!(
                            "Could not read from deleted aggregator at {:?}",
                            self.id
                        ))
                    })?;

                *speculative_start_value =
                    SpeculativeStartValue::LastCommittedValue(value_from_storage)
            }
        }
        Ok(())
    }

    /// Implements logic for doing an "expensive read" of an aggregator.
    /// This means that we perform a full read of an aggregator, that may involve
    /// aggregating any speculative delta operations and can thus be more expensive
    /// than reading the latest committed value.
    /// This method has a sideffect, of updating `speculative_start_value` with
    /// `LastCommittedValue` variant.
    /// Both `get_any_value()` and `get_value_for_read()` are guaranteed to succeed
    /// after this call.
    pub fn read_most_recent_aggregator_value(
        &mut self,
        resolver: &dyn AggregatorResolver,
    ) -> PartialVMResult<u128> {
        match &mut self.state {
            AggregatorState::Data { value } => {
                // If aggregator knows the value, return it.
                Ok(*value)
            },
            AggregatorState::Delta {
                speculative_start_value,
                delta,
                history,
            } => {
                let math = BoundedMath::new(self.max_value);
                // If we performed an "expensive read" operation before, use it.
                if let SpeculativeStartValue::AggregatedValue(start_value) = speculative_start_value
                {
                    return Ok(math.unsigned_add_delta(*start_value, delta)?);
                }

                // Otherwise, we have to go to storage and read the value.
                let maybe_value_from_storage = match &self.id {
                    AggregatorID::Legacy(state_key) => {
                        resolver.get_aggregator_v1_value(state_key, AggregatorReadMode::Aggregated)
                    },
                    // TODO: use integers directly, or some wrapped type.
                    id => resolver
                        .get_aggregator_v2_value(id, AggregatorReadMode::Aggregated)
                        .and_then(|v| Ok(v.into_aggregator_value()?))
                        .map(Some),
                };
                let value_from_storage = maybe_value_from_storage
                    .map_err(|e| {
                        extension_error(format!(
                            "Could not find the value of the aggregator: {}",
                            e
                        ))
                    })?
                    .ok_or_else(|| {
                        extension_error(format!(
                            "Could not read from deleted aggregator at {:?}",
                            self.id
                        ))
                    })?;

                // Validate history.
                history.validate_against_base_value(value_from_storage, self.max_value)?;
                // Applyng shouldn't fail after validation
                let result = expect_ok(math.unsigned_add_delta(value_from_storage, delta))?;

                *speculative_start_value =
                    SpeculativeStartValue::AggregatedValue(value_from_storage);
                Ok(result)
            },
        }
    }

    /// Unpacks aggregator into its fields.
    pub fn into(self) -> (u128, AggregatorState) {
        (self.max_value, self.state)
    }
}

/// Stores all information about aggregators (how many have been created or
/// removed), what are their states, etc. per single transaction).
#[derive(Default)]
pub struct AggregatorData {
    // All aggregators that were created in the current transaction, stored as ids.
    // Used to filter out aggregators that were created and destroyed in the
    // within a single transaction.
    new_aggregators: BTreeSet<AggregatorID>,
    // All aggregators that were destroyed in the current transaction, stored as ids.
    destroyed_aggregators: BTreeSet<AggregatorID>,
    // All aggregator instances that exist in the current transaction.
    aggregators: BTreeMap<AggregatorID, Aggregator>,
    // All aggregatorsnapshot instances that exist in the current transaction.
    aggregator_snapshots: BTreeMap<AggregatorID, AggregatorSnapshot>,
    // Counter for generating identifiers for Aggregators and AggregatorSnapshots.
    pub id_counter: u64,
}

impl AggregatorData {
    pub fn new(id_counter: u64) -> Self {
        Self {
            id_counter,
            ..Default::default()
        }
    }

    /// Returns a mutable reference to an aggregator with `id` and a `max_value`.
    /// If transaction that is currently executing did not initialize it, a new aggregator instance is created.
    /// Note: when we say "aggregator instance" here we refer to Rust struct and
    /// not to the Move aggregator.
    pub fn get_aggregator(
        &mut self,
        id: AggregatorID,
        max_value: u128,
    ) -> PartialVMResult<&mut Aggregator> {
        let aggregator = self
            .aggregators
            .entry(id.clone())
            .or_insert_with(|| Aggregator {
                id,
                state: AggregatorState::Delta {
                    speculative_start_value: SpeculativeStartValue::Unset,
                    delta: SignedU128::Positive(0),
                    history: DeltaHistory::new(),
                },
                max_value,
            });
        Ok(aggregator)
    }

    /// Returns the number of aggregators that are used in the current transaction.
    pub fn num_aggregators(&self) -> u128 {
        self.aggregators.len() as u128
    }

    /// Creates and a new Aggregator with a given `id` and a `max_value`. The value
    /// of a new aggregator is always known, therefore it is created in a data
    /// state, with a zero-initialized value.
    pub fn create_new_aggregator(&mut self, id: AggregatorID, max_value: u128) {
        let aggregator = Aggregator {
            id: id.clone(),
            state: AggregatorState::Data { value: 0 },
            max_value,
        };
        self.aggregators.insert(id.clone(), aggregator);
        self.new_aggregators.insert(id);
    }

    /// If aggregator has been used in this transaction, it is removed. Otherwise,
    /// it is marked for deletion.
    /// TODO: Should we return an error if aggregator id is v2?
    pub fn remove_aggregator(&mut self, id: AggregatorID) {
        // Aggregator no longer in use during this transaction: remove it.
        self.aggregators.remove(&id);

        if self.new_aggregators.contains(&id) {
            // Aggregator has been created in the same transaction. Therefore, no
            // side-effects.
            self.new_aggregators.remove(&id);
        } else {
            // Otherwise, aggregator has been created somewhere else.
            self.destroyed_aggregators.insert(id);
        }
    }

    pub fn snapshot(&mut self, id: &AggregatorID, max_value: u128) -> PartialVMResult<u64> {
        let new_id = self.generate_id();
        let snapshot_id = AggregatorID::ephemeral(new_id);

        let aggregator = self.get_aggregator(id.clone(), max_value)?;

        let snapshot_state = match aggregator.state {
            // If aggregator is in Data state, we don't need to depend on it, and can just take the value.
            AggregatorState::Data { value } => AggregatorSnapshotState::Data {
                value: SnapshotValue::Integer(value),
            },
            AggregatorState::Delta { delta, .. } => AggregatorSnapshotState::Delta {
                base_aggregator: id.clone(),
                delta,
            },
        };

        self.aggregator_snapshots
            .insert(snapshot_id.clone(), AggregatorSnapshot {
                id: snapshot_id,
                state: snapshot_state,
            });
        Ok(new_id)
    }

    pub fn create_new_snapshot(&mut self, id: AggregatorID, value: SnapshotValue) {
        let snapshot_state = AggregatorSnapshotState::Data { value };

        self.aggregator_snapshots
            .insert(id.clone(), AggregatorSnapshot {
                id,
                state: snapshot_state,
            });
    }

    /// Returns a mutable reference to an aggregator snapshot with `id`.
    /// If transaction that is currently executing did not initialize it, a new aggregator snapshot instance is created.
    /// Note: when we say "aggregator snapshot instance" here we refer to Rust struct and
    /// not to the Move aggregator snapshot.
    pub fn get_snapshot(
        &mut self,
        id: AggregatorID,
        resolver: &dyn AggregatorResolver,
    ) -> PartialVMResult<&AggregatorSnapshot> {
        let snapshot = match self.aggregator_snapshots.entry(id.clone()) {
            Entry::Vacant(entry) => {
                // Otherwise, we have to go to storage and read the value.
                let value_from_storage = resolver
                    .get_aggregator_v2_value(&id, AggregatorReadMode::Aggregated)
                    .map_err(|e| {
                        extension_error(format!(
                            "Could not find the value of the aggregator: {}",
                            e
                        ))
                    })?;
                entry.insert(AggregatorSnapshot {
                    id,
                    state: AggregatorSnapshotState::Reference {
                        speculative_value: value_from_storage,
                    },
                })
            },
            Entry::Occupied(entry) => entry.into_mut(),
        };
        Ok(snapshot)
    }

    pub fn read_snapshot(
        &mut self,
        id: AggregatorID,
        resolver: &dyn AggregatorResolver,
    ) -> PartialVMResult<SnapshotValue> {
        let snapshot_state = self.get_snapshot(id.clone(), resolver)?.state.clone();
        match snapshot_state {
            AggregatorSnapshotState::Data { value } => Ok(value.clone()),
            AggregatorSnapshotState::Delta {
                base_aggregator,
                delta,
            } => match self.aggregators.get_mut(&base_aggregator) {
                Some(aggregator) => {
                    let value = aggregator.read_most_recent_aggregator_value(resolver)?;
                    Ok(SnapshotValue::Integer(expect_ok(
                        BoundedMath::new(aggregator.max_value).unsigned_add_delta(value, &delta),
                    )?))
                },
                None => resolver
                    .get_aggregator_v2_value(&id, AggregatorReadMode::Aggregated)
                    .map_err(|e| {
                        extension_error(format!(
                            "Could not find the value of the aggregator: {}",
                            e
                        ))
                    }),
            },
            AggregatorSnapshotState::Concat {
                base_snapshot,
                formula,
            } => {
                let base = self.read_snapshot(base_snapshot.clone(), resolver)?;
                Ok(formula.apply(&base))
            },
            AggregatorSnapshotState::Reference { speculative_value } => {
                Ok(speculative_value.clone())
            },
        }
    }

    pub fn string_concat(&mut self, id: AggregatorID, prefix: Vec<u8>, suffix: Vec<u8>) -> u64 {
        let new_id = self.generate_id();
        let snapshot_id = AggregatorID::ephemeral(new_id);

        let snapshot_state = AggregatorSnapshotState::Concat {
            base_snapshot: id.clone(),
            formula: DerivedFormula::Concat { prefix, suffix },
        };

        self.aggregator_snapshots
            .insert(snapshot_id.clone(), AggregatorSnapshot {
                id: snapshot_id,
                state: snapshot_state,
            });
        new_id
    }

    pub fn generate_id(&mut self) -> u64 {
        self.id_counter += 1;
        self.id_counter
    }

    /// Unpacks aggregator data.
    pub fn into(
        self,
    ) -> (
        BTreeSet<AggregatorID>,
        BTreeSet<AggregatorID>,
        BTreeMap<AggregatorID, Aggregator>,
        BTreeMap<AggregatorID, AggregatorSnapshot>,
    ) {
        (
            self.new_aggregators,
            self.destroyed_aggregators,
            self.aggregators,
            self.aggregator_snapshots,
        )
    }
}

/// Returns partial VM error on extension failure.
pub fn extension_error(message: impl ToString) -> PartialVMError {
    PartialVMError::new(StatusCode::VM_EXTENSION_ERROR).with_message(message.to_string())
}

// ================================= Tests =================================

#[cfg(test)]
mod test {
    use super::*;
    use crate::{aggregator_v1_id_for_test, AggregatorStore};
    use claims::{assert_err, assert_ok, assert_ok_eq, assert_some_eq};
    use once_cell::sync::Lazy;

    #[allow(clippy::redundant_closure)]
    static TEST_RESOLVER: Lazy<AggregatorStore> = Lazy::new(|| AggregatorStore::default());

    #[test]
    fn test_aggregator_not_in_storage() {
        let mut aggregator_data = AggregatorData::default();
        let aggregator = aggregator_data
            .get_aggregator(aggregator_v1_id_for_test(300), 700)
            .unwrap();
        assert_err!(aggregator.read_last_committed_aggregator_value(&*TEST_RESOLVER));
        assert_err!(aggregator.read_most_recent_aggregator_value(&*TEST_RESOLVER));
        assert_err!(aggregator.try_add(&*TEST_RESOLVER, 100));
        assert_err!(aggregator.try_sub(&*TEST_RESOLVER, 1));
    }

    #[test]
    fn test_operations_on_new_aggregator() {
        let mut aggregator_data = AggregatorData::default();
        aggregator_data.create_new_aggregator(aggregator_v1_id_for_test(200), 200);

        let aggregator = aggregator_data
            .get_aggregator(aggregator_v1_id_for_test(200), 200)
            .expect("Get aggregator failed");

        assert_eq!(aggregator.state, AggregatorState::Data { value: 0 });
        assert_ok!(aggregator.try_add(&*TEST_RESOLVER, 100));
        assert_eq!(aggregator.state, AggregatorState::Data { value: 100 });
        assert!(aggregator.try_sub(&*TEST_RESOLVER, 50).unwrap());
        assert_eq!(aggregator.state, AggregatorState::Data { value: 50 });
        assert!(!aggregator.try_sub(&*TEST_RESOLVER, 70).unwrap());
        assert_eq!(aggregator.state, AggregatorState::Data { value: 50 });
        assert!(!aggregator.try_add(&*TEST_RESOLVER, 170).unwrap());
        assert_eq!(aggregator.state, AggregatorState::Data { value: 50 });
        assert_ok_eq!(
            aggregator.read_most_recent_aggregator_value(&*TEST_RESOLVER),
            50
        );
    }
    #[test]
    fn test_successful_operations_in_delta_mode() {
        let mut aggregator_data = AggregatorData::default();
        let mut sample_resolver: AggregatorStore = AggregatorStore::default();
        sample_resolver.set_from_id(aggregator_v1_id_for_test(600), 100);

        let aggregator = aggregator_data
            .get_aggregator(aggregator_v1_id_for_test(600), 600)
            .expect("Get aggregator failed");

        assert_eq!(aggregator.state, AggregatorState::Delta {
            speculative_start_value: SpeculativeStartValue::Unset,
            delta: SignedU128::Positive(0),
            history: DeltaHistory {
                max_achieved_positive_delta: 0,
                min_achieved_negative_delta: 0,
                min_overflow_positive_delta: None,
                max_underflow_negative_delta: None,
            }
        });
        assert_ok!(aggregator.try_add(&sample_resolver, 400));
        assert_eq!(aggregator.state, AggregatorState::Delta {
            speculative_start_value: SpeculativeStartValue::LastCommittedValue(100),
            delta: SignedU128::Positive(400),
            history: DeltaHistory {
                max_achieved_positive_delta: 400,
                min_achieved_negative_delta: 0,
                min_overflow_positive_delta: None,
                max_underflow_negative_delta: None,
            }
        });
        assert_ok!(aggregator.try_sub(&sample_resolver, 470));
        assert_eq!(aggregator.state, AggregatorState::Delta {
            speculative_start_value: SpeculativeStartValue::LastCommittedValue(100),
            delta: SignedU128::Negative(70),
            history: DeltaHistory {
                max_achieved_positive_delta: 400,
                min_achieved_negative_delta: 70,
                min_overflow_positive_delta: None,
                max_underflow_negative_delta: None,
            }
        });
        assert_ok_eq!(
            aggregator.read_most_recent_aggregator_value(&sample_resolver),
            30
        );
        assert_eq!(aggregator.state, AggregatorState::Delta {
            speculative_start_value: SpeculativeStartValue::AggregatedValue(100),
            delta: SignedU128::Negative(70),
            history: DeltaHistory {
                max_achieved_positive_delta: 400,
                min_achieved_negative_delta: 70,
                min_overflow_positive_delta: None,
                max_underflow_negative_delta: None,
            }
        });
    }

    #[test]
    fn test_history_updates() {
        let mut aggregator_data = AggregatorData::default();
        let mut sample_resolver: AggregatorStore = AggregatorStore::default();
        sample_resolver.set_from_id(aggregator_v1_id_for_test(600), 100);

        let aggregator = aggregator_data
            .get_aggregator(aggregator_v1_id_for_test(600), 600)
            .expect("Get aggregator failed");

        assert_eq!(aggregator.state, AggregatorState::Delta {
            speculative_start_value: SpeculativeStartValue::Unset,
            delta: SignedU128::Positive(0),
            history: DeltaHistory {
                max_achieved_positive_delta: 0,
                min_achieved_negative_delta: 0,
                min_overflow_positive_delta: None,
                max_underflow_negative_delta: None,
            }
        });
        assert_ok!(aggregator.try_add(&sample_resolver, 300));
        assert_some_eq!(aggregator.get_history(), &DeltaHistory {
            max_achieved_positive_delta: 300,
            min_achieved_negative_delta: 0,
            min_overflow_positive_delta: None,
            max_underflow_negative_delta: None,
        });
        assert_ok!(aggregator.try_add(&sample_resolver, 100));
        assert_some_eq!(aggregator.get_history(), &DeltaHistory {
            max_achieved_positive_delta: 400,
            min_achieved_negative_delta: 0,
            min_overflow_positive_delta: None,
            max_underflow_negative_delta: None,
        });
        assert_ok!(aggregator.try_sub(&sample_resolver, 450));
        assert_some_eq!(aggregator.get_history(), &DeltaHistory {
            max_achieved_positive_delta: 400,
            min_achieved_negative_delta: 50,
            min_overflow_positive_delta: None,
            max_underflow_negative_delta: None,
        });
        assert_ok!(aggregator.try_add(&sample_resolver, 200));
        assert_some_eq!(aggregator.get_history(), &DeltaHistory {
            max_achieved_positive_delta: 400,
            min_achieved_negative_delta: 50,
            min_overflow_positive_delta: None,
            max_underflow_negative_delta: None,
        });
        assert_ok!(aggregator.try_add(&sample_resolver, 350));
        assert_some_eq!(aggregator.get_history(), &DeltaHistory {
            max_achieved_positive_delta: 500,
            min_achieved_negative_delta: 50,
            min_overflow_positive_delta: None,
            max_underflow_negative_delta: None,
        });
        assert_ok!(aggregator.try_sub(&sample_resolver, 600));
        assert_some_eq!(aggregator.get_history(), &DeltaHistory {
            max_achieved_positive_delta: 500,
            min_achieved_negative_delta: 100,
            min_overflow_positive_delta: None,
            max_underflow_negative_delta: None,
        });
    }

    #[test]
    fn test_aggregator_overflows() {
        let mut aggregator_data = AggregatorData::default();
        let mut sample_resolver: AggregatorStore = AggregatorStore::default();
        sample_resolver.set_from_id(aggregator_v1_id_for_test(600), 100);

        let aggregator = aggregator_data
            .get_aggregator(aggregator_v1_id_for_test(600), 600)
            .expect("Get aggregator failed");

        assert!(aggregator.try_add(&sample_resolver, 400).unwrap());
        assert_some_eq!(aggregator.get_history(), &DeltaHistory {
            max_achieved_positive_delta: 400,
            min_achieved_negative_delta: 0,
            min_overflow_positive_delta: None,
            max_underflow_negative_delta: None,
        });
        assert!(aggregator.try_sub(&sample_resolver, 450).unwrap());
        assert_some_eq!(aggregator.get_history(), &DeltaHistory {
            max_achieved_positive_delta: 400,
            min_achieved_negative_delta: 50,
            min_overflow_positive_delta: None,
            max_underflow_negative_delta: None,
        });
        assert!(!aggregator.try_add(&sample_resolver, 601).unwrap());
        assert_some_eq!(aggregator.get_history(), &DeltaHistory {
            max_achieved_positive_delta: 400,
            min_achieved_negative_delta: 50,
            min_overflow_positive_delta: None,
            max_underflow_negative_delta: None,
        });
        assert!(!aggregator.try_add(&sample_resolver, 575).unwrap());
        assert_some_eq!(aggregator.get_history(), &DeltaHistory {
            max_achieved_positive_delta: 400,
            min_achieved_negative_delta: 50,
            min_overflow_positive_delta: Some(525),
            max_underflow_negative_delta: None,
        });
        assert!(!aggregator.try_add(&sample_resolver, 551).unwrap());
        assert_some_eq!(aggregator.get_history(), &DeltaHistory {
            max_achieved_positive_delta: 400,
            min_achieved_negative_delta: 50,
            min_overflow_positive_delta: Some(501),
            max_underflow_negative_delta: None,
        });
        assert!(!aggregator.try_add(&sample_resolver, 570).unwrap());
        assert_some_eq!(aggregator.get_history(), &DeltaHistory {
            max_achieved_positive_delta: 400,
            min_achieved_negative_delta: 50,
            min_overflow_positive_delta: Some(501),
            max_underflow_negative_delta: None,
        });
    }

    fn assert_delta_state(
        aggregator: &AggregatorState,
        speculative_start_value: u128,
        delta: i128,
        history: DeltaHistory,
    ) {
        assert_eq!(aggregator, &AggregatorState::Delta {
            speculative_start_value: SpeculativeStartValue::LastCommittedValue(
                speculative_start_value
            ),
            delta: if delta > 0 {
                SignedU128::Positive(delta as u128)
            } else {
                SignedU128::Negative((-delta) as u128)
            },
            history,
        });
    }

    #[test]
    fn test_aggregator_underflows() {
        let mut aggregator_data = AggregatorData::default();
        let mut sample_resolver: AggregatorStore = AggregatorStore::default();
        sample_resolver.set_from_id(aggregator_v1_id_for_test(600), 200);

        let aggregator = aggregator_data
            .get_aggregator(aggregator_v1_id_for_test(600), 600)
            .expect("Get aggregator failed");

        assert!(aggregator.try_add(&sample_resolver, 300).unwrap());
        assert_delta_state(&aggregator.state, 200, 300, DeltaHistory {
            max_achieved_positive_delta: 300,
            min_achieved_negative_delta: 0,
            min_overflow_positive_delta: None,
            max_underflow_negative_delta: None,
        });
        assert!(!aggregator.try_sub(&sample_resolver, 650).unwrap());
        assert_delta_state(&aggregator.state, 200, 300, DeltaHistory {
            max_achieved_positive_delta: 300,
            min_achieved_negative_delta: 0,
            min_overflow_positive_delta: None,
            max_underflow_negative_delta: None,
        });
        assert!(!aggregator.try_sub(&sample_resolver, 550).unwrap());
        assert_delta_state(&aggregator.state, 200, 300, DeltaHistory {
            max_achieved_positive_delta: 300,
            min_achieved_negative_delta: 0,
            min_overflow_positive_delta: None,
            max_underflow_negative_delta: Some(250),
        });
        assert!(!aggregator.try_sub(&sample_resolver, 525).unwrap());
        assert_delta_state(&aggregator.state, 200, 300, DeltaHistory {
            max_achieved_positive_delta: 300,
            min_achieved_negative_delta: 0,
            min_overflow_positive_delta: None,
            max_underflow_negative_delta: Some(225),
        });
        assert!(!aggregator.try_sub(&sample_resolver, 540).unwrap());
        assert_delta_state(&aggregator.state, 200, 300, DeltaHistory {
            max_achieved_positive_delta: 300,
            min_achieved_negative_delta: 0,
            min_overflow_positive_delta: None,
            max_underflow_negative_delta: Some(225),
        });
        assert!(!aggregator.try_sub(&sample_resolver, 501).unwrap());
        assert_delta_state(&aggregator.state, 200, 300, DeltaHistory {
            max_achieved_positive_delta: 300,
            min_achieved_negative_delta: 0,
            min_overflow_positive_delta: None,
            max_underflow_negative_delta: Some(201),
        });
    }

    #[test]
    fn test_change_in_base_value_1() {
        let mut aggregator_data = AggregatorData::default();
        let mut sample_resolver: AggregatorStore = AggregatorStore::default();
        sample_resolver.set_from_id(aggregator_v1_id_for_test(600), 200);

        let aggregator = aggregator_data
            .get_aggregator(aggregator_v1_id_for_test(600), 600)
            .expect("Get aggregator failed");

        assert_ok!(aggregator.try_add(&sample_resolver, 300));
        assert_ok!(aggregator.try_sub(&sample_resolver, 400));
        assert_ok!(aggregator.try_add(&sample_resolver, 400));
        assert_ok!(aggregator.try_sub(&sample_resolver, 500));
        assert_eq!(aggregator.state, AggregatorState::Delta {
            speculative_start_value: SpeculativeStartValue::LastCommittedValue(200),
            delta: SignedU128::Negative(200),
            history: DeltaHistory {
                max_achieved_positive_delta: 300,
                min_achieved_negative_delta: 200,
                min_overflow_positive_delta: None,
                max_underflow_negative_delta: None,
            }
        });
        if let AggregatorState::Delta { history, .. } = aggregator.state {
            assert_ok!(history.validate_against_base_value(200, aggregator.max_value,));
            assert_err!(history.validate_against_base_value(199, aggregator.max_value,));
            assert_ok!(history.validate_against_base_value(300, aggregator.max_value,));
            assert_err!(history.validate_against_base_value(301, aggregator.max_value,));
        }
    }

    #[test]
    fn test_change_in_base_value_2() {
        let mut aggregator_data = AggregatorData::default();
        let mut sample_resolver: AggregatorStore = AggregatorStore::default();
        sample_resolver.set_from_id(aggregator_v1_id_for_test(600), 200);

        let aggregator = aggregator_data
            .get_aggregator(aggregator_v1_id_for_test(600), 600)
            .expect("Get aggregator failed");

        assert!(!aggregator.try_add(&sample_resolver, 401).unwrap());
        assert!(aggregator.try_add(&sample_resolver, 300).unwrap());
        assert_eq!(aggregator.state, AggregatorState::Delta {
            speculative_start_value: SpeculativeStartValue::LastCommittedValue(200),
            delta: SignedU128::Positive(300),
            history: DeltaHistory {
                max_achieved_positive_delta: 300,
                min_achieved_negative_delta: 0,
                min_overflow_positive_delta: Some(401),
                max_underflow_negative_delta: None,
            }
        });

        if let AggregatorState::Delta { history, .. } = aggregator.state {
            assert_err!(history.validate_against_base_value(199, aggregator.max_value,));
            assert_ok!(history.validate_against_base_value(200, aggregator.max_value,));
            assert_ok!(history.validate_against_base_value(300, aggregator.max_value,));
            assert_err!(history.validate_against_base_value(301, aggregator.max_value,));
        }
    }

    #[test]
    fn test_change_in_base_value_3() {
        let mut aggregator_data = AggregatorData::default();
        let mut sample_resolver: AggregatorStore = AggregatorStore::default();
        sample_resolver.set_from_id(aggregator_v1_id_for_test(600), 200);

        let aggregator = aggregator_data
            .get_aggregator(aggregator_v1_id_for_test(600), 600)
            .expect("Get aggregator failed");

        assert!(aggregator.try_sub(&sample_resolver, 100).unwrap());
        assert!(!aggregator.try_sub(&sample_resolver, 101).unwrap());
        assert!(aggregator.try_add(&sample_resolver, 300).unwrap());
        assert_eq!(aggregator.state, AggregatorState::Delta {
            speculative_start_value: SpeculativeStartValue::LastCommittedValue(200),
            delta: SignedU128::Positive(200),
            history: DeltaHistory {
                max_achieved_positive_delta: 200,
                min_achieved_negative_delta: 100,
                min_overflow_positive_delta: None,
                max_underflow_negative_delta: Some(201),
            }
        });

        if let AggregatorState::Delta { history, .. } = aggregator.state {
            assert_ok!(history.validate_against_base_value(100, aggregator.max_value,));
            assert_ok!(history.validate_against_base_value(199, aggregator.max_value,));
            assert_ok!(history.validate_against_base_value(200, aggregator.max_value,));
            assert_err!(history.validate_against_base_value(201, aggregator.max_value,));
            assert_err!(history.validate_against_base_value(400, aggregator.max_value,));
        }
    }
}
