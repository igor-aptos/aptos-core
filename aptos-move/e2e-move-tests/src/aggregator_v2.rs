// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use crate::{assert_success, harness::MoveHarness};
use aptos_language_e2e_tests::{account::Account, executor::FakeExecutor};
use aptos_types::{
    account_address::AccountAddress, on_chain_config::FeatureFlag, transaction::SignedTransaction,
};
use move_core_types::language_storage::TypeTag;
use std::path::PathBuf;

pub fn initialize(path: PathBuf) -> (MoveHarness, Account) {
    // Aggregator tests should use parallel execution.
    let executor = FakeExecutor::from_head_genesis().set_parallel();

    let mut harness = MoveHarness::new_with_executor(executor);
    harness.enable_features(vec![FeatureFlag::AGGREGATOR_API], vec![]);
    let account = harness.new_account_at(AccountAddress::ONE);
    assert_success!(harness.publish_package(&account, &path));
    assert_success!(harness.run_entry_function(
        &account,
        str::parse("0x1::aggregator_v2_test::initialize").unwrap(),
        vec![],
        vec![],
    ));
    (harness, account)
}

pub fn check(
    harness: &mut MoveHarness,
    account: &Account,
    index: u64,
    expected: u128,
) -> SignedTransaction {
    harness.create_entry_function(
        account,
        str::parse("0x1::aggregator_v2_test::check").unwrap(),
        vec![],
        vec![
            bcs::to_bytes(&index).unwrap(),
            bcs::to_bytes(&expected).unwrap(),
        ],
    )
}

pub fn new(
    harness: &mut MoveHarness,
    account: &Account,
    index: u64,
    limit: u128,
) -> SignedTransaction {
    harness.create_entry_function(
        account,
        str::parse("0x1::aggregator_v2_test::new").unwrap(),
        vec![],
        vec![
            bcs::to_bytes(&index).unwrap(),
            bcs::to_bytes(&limit).unwrap(),
        ],
    )
}

pub fn try_add(
    harness: &mut MoveHarness,
    account: &Account,
    index: u64,
    value: u128,
) -> SignedTransaction {
    harness.create_entry_function(
        account,
        str::parse("0x1::aggregator_v2_test::try_add").unwrap(),
        vec![],
        vec![
            bcs::to_bytes(&index).unwrap(),
            bcs::to_bytes(&value).unwrap(),
        ],
    )
}

pub fn try_sub(
    harness: &mut MoveHarness,
    account: &Account,
    index: u64,
    value: u128,
) -> SignedTransaction {
    harness.create_entry_function(
        account,
        str::parse("0x1::aggregator_v2_test::try_sub").unwrap(),
        vec![],
        vec![
            bcs::to_bytes(&index).unwrap(),
            bcs::to_bytes(&value).unwrap(),
        ],
    )
}

pub fn try_sub_add(
    harness: &mut MoveHarness,
    account: &Account,
    index: u64,
    a: u128,
    b: u128,
) -> SignedTransaction {
    harness.create_entry_function(
        account,
        str::parse("0x1::aggregator_v2_test::try_sub_add").unwrap(),
        vec![],
        vec![
            bcs::to_bytes(&index).unwrap(),
            bcs::to_bytes(&a).unwrap(),
            bcs::to_bytes(&b).unwrap(),
        ],
    )
}

pub fn materialize(harness: &mut MoveHarness, account: &Account, index: u64) -> SignedTransaction {
    harness.create_entry_function(
        account,
        str::parse("0x1::aggregator_v2_test::materialize").unwrap(),
        vec![],
        vec![bcs::to_bytes(&index).unwrap()],
    )
}

pub fn materialize_and_try_add(
    harness: &mut MoveHarness,
    account: &Account,
    index: u64,
    value: u128,
) -> SignedTransaction {
    harness.create_entry_function(
        account,
        str::parse("0x1::aggregator_v2_test::materialize_and_try_add").unwrap(),
        vec![],
        vec![
            bcs::to_bytes(&index).unwrap(),
            bcs::to_bytes(&value).unwrap(),
        ],
    )
}

pub fn materialize_and_try_sub(
    harness: &mut MoveHarness,
    account: &Account,
    index: u64,
    value: u128,
) -> SignedTransaction {
    harness.create_entry_function(
        account,
        str::parse("0x1::aggregator_v2_test::materialize_and_try_sub").unwrap(),
        vec![],
        vec![
            bcs::to_bytes(&index).unwrap(),
            bcs::to_bytes(&value).unwrap(),
        ],
    )
}

pub fn try_add_and_materialize(
    harness: &mut MoveHarness,
    account: &Account,
    index: u64,
    value: u128,
) -> SignedTransaction {
    harness.create_entry_function(
        account,
        str::parse("0x1::aggregator_v2_test::try_add_and_materialize").unwrap(),
        vec![],
        vec![
            bcs::to_bytes(&index).unwrap(),
            bcs::to_bytes(&value).unwrap(),
        ],
    )
}

pub fn try_sub_and_materialize(
    harness: &mut MoveHarness,
    account: &Account,
    index: u64,
    value: u128,
) -> SignedTransaction {
    harness.create_entry_function(
        account,
        str::parse("0x1::aggregator_v2_test::try_sub_and_materialize").unwrap(),
        vec![],
        vec![
            bcs::to_bytes(&index).unwrap(),
            bcs::to_bytes(&value).unwrap(),
        ],
    )
}

pub fn verify_copy_snapshot(harness: &mut MoveHarness, account: &Account) -> SignedTransaction {
    harness.create_entry_function(
        account,
        str::parse("0x1::aggregator_v2_test::verify_copy_snapshot").unwrap(),
        vec![],
        vec![],
    )
}

pub fn verify_copy_string_snapshot(
    harness: &mut MoveHarness,
    account: &Account,
) -> SignedTransaction {
    harness.create_entry_function(
        account,
        str::parse("0x1::aggregator_v2_test::verify_copy_string_snapshot").unwrap(),
        vec![],
        vec![],
    )
}

pub fn verify_string_concat(harness: &mut MoveHarness, account: &Account) -> SignedTransaction {
    harness.create_entry_function(
        account,
        str::parse("0x1::aggregator_v2_test::verify_string_concat").unwrap(),
        vec![],
        vec![],
    )
}

pub fn verify_string_snapshot_concat(
    harness: &mut MoveHarness,
    account: &Account,
) -> SignedTransaction {
    harness.create_entry_function(
        account,
        str::parse("0x1::aggregator_v2_test::verify_string_snapshot_concat").unwrap(),
        vec![],
        vec![],
    )
}

pub fn snapshot(harness: &mut MoveHarness, account: &Account, index: u64) -> SignedTransaction {
    harness.create_entry_function(
        account,
        str::parse("0x1::aggregator_v2_test::snapshot").unwrap(),
        vec![],
        vec![bcs::to_bytes(&index).unwrap()],
    )
}

pub fn read_snapshot(
    harness: &mut MoveHarness,
    account: &Account,
    index: u64,
) -> SignedTransaction {
    harness.create_entry_function(
        account,
        str::parse("0x1::aggregator_v2_test::read_snapshot").unwrap(),
        vec![TypeTag::U128],
        vec![bcs::to_bytes(&index).unwrap()],
    )
}

pub fn try_add_and_read_snapshot_u128(
    harness: &mut MoveHarness,
    account: &Account,
    index: u64,
    value: u128,
) -> SignedTransaction {
    harness.create_entry_function(
        account,
        str::parse("0x1::aggregator_v2_test::try_add_and_read_snapshot").unwrap(),
        vec![],
        vec![
            bcs::to_bytes(&index).unwrap(),
            bcs::to_bytes(&value).unwrap(),
        ],
    )
}
