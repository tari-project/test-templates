//   Copyright 2024 The Tari Project
//   SPDX-License-Identifier: BSD-3-Clause

use tari_engine_types::instruction::Instruction;
use tari_template_lib::{
    args,
    models::{Amount, ComponentAddress},
    prelude::{NonFungibleAddress, ResourceAddress},
};
use tari_template_test_tooling::{SubstateType, TemplateTest};
use tari_template_test_tooling::support::assert_error::assert_reject_reason;
use tari_transaction::Transaction;
use tari_template_test_tooling::crypto::RistrettoSecretKey;

struct TariswapTest {
    template_test: TemplateTest,
    a_resource: ResourceAddress,
    b_resource: ResourceAddress,
    lp_resource: ResourceAddress,
    index_component: ComponentAddress,
    pool_component: ComponentAddress,
    account_address: ComponentAddress,
    account_proof: NonFungibleAddress,
}

fn setup(fee: u16) -> TariswapTest {
    let mut template_test = TemplateTest::new(["./templates/index", "./templates/pool"]);

    // create the pool index
    let index_component = create_index_component(&mut template_test, fee);

    // create the token pair for the swap pool
    let (a_faucet, a_resource) = create_faucet_component(&mut template_test, "A".to_string());
    let (b_faucet, b_resource) = create_faucet_component(&mut template_test, "B".to_string());

    let (pool_component, lp_resource) = create_pool_component(&mut template_test, a_resource, b_resource, index_component);

    let (account_address, account_proof, _) = template_test.create_owned_account();
    fund_account(&mut template_test, account_address, a_faucet);
    fund_account(&mut template_test, account_address, b_faucet);

    TariswapTest {
        template_test,
        a_resource,
        b_resource,
        lp_resource,
        index_component,
        pool_component,
        account_address,
        account_proof,
    }
}

fn create_faucet_component(template_test: &mut TemplateTest, symbol: String) -> (ComponentAddress, ResourceAddress) {
    let initial_supply = Amount(1_000_000_000_000);
    let component_address: ComponentAddress =
        template_test.call_function("TestFaucet", "mint_with_symbol", args![initial_supply, symbol], vec![]);

    let resource_address = template_test
        .get_previous_output_address(SubstateType::Resource)
        .as_resource_address()
        .unwrap();

    (component_address, resource_address)
}

fn create_index_component(template_test: &mut TemplateTest, fee: u16) -> ComponentAddress {
    let pool_template = template_test.get_template_address("TariswapPool");

    let component_address: ComponentAddress =
        template_test.call_function("TariswapIndex", "new", args![pool_template, fee], vec![]);

    component_address
}

fn create_pool_component(
    template_test: &mut TemplateTest,
    a_resource: ResourceAddress,
    b_resource: ResourceAddress,
    index_component: ComponentAddress
) -> (ComponentAddress, ResourceAddress) {
    let pool_module_name = "TariswapPool";

    let res = template_test
        .execute_and_commit(
            vec![Instruction::CallMethod {
                component_address: index_component,
                method: "create_pool".to_string(),
                args: args![a_resource, b_resource],
            }],
            vec![],
        )
        .unwrap();

    // extract the component address
    let (substate_addr, _) = res
        .expect_success()
        .up_iter()
        .find(|(address, substate)| {
            address.is_component() && substate.substate_value().component().unwrap().module_name == pool_module_name
        })
        .unwrap();
    let component_address = substate_addr.as_component_address().unwrap();

    // extract the LP token resource address
    let (substate_addr, _) = res
        .expect_success()
        .up_iter()
        .find(|(address, _)| address.is_resource())
        .unwrap();
    let lp_resource = substate_addr.as_resource_address().unwrap();

    (component_address, lp_resource)
}

fn fund_account(
    template_test: &mut TemplateTest,
    account_address: ComponentAddress,
    faucet_component: ComponentAddress,
) {
    template_test
        .execute_and_commit(
            vec![
                Instruction::CallMethod {
                    component_address: faucet_component,
                    method: "take_free_coins".to_string(),
                    args: args![],
                },
                Instruction::PutLastInstructionOutputOnWorkspace {
                    key: b"free_coins".to_vec(),
                },
                Instruction::CallMethod {
                    component_address: account_address,
                    method: "deposit".to_string(),
                    args: args![Variable("free_coins")],
                },
            ],
            vec![],
        )
        .unwrap();
}

fn swap(test: &mut TariswapTest, input_resource: &ResourceAddress, output_resource: &ResourceAddress, amount: Amount) {
    test.template_test
        .execute_and_commit(
            vec![
                Instruction::CallMethod {
                    component_address: test.account_address,
                    method: "withdraw".to_string(),
                    args: args![input_resource, amount],
                },
                Instruction::PutLastInstructionOutputOnWorkspace {
                    key: b"input_bucket".to_vec(),
                },
                Instruction::CallMethod {
                    component_address: test.pool_component,
                    method: "swap".to_string(),
                    args: args![Variable("input_bucket"), output_resource],
                },
                Instruction::PutLastInstructionOutputOnWorkspace {
                    key: b"output_bucket".to_vec(),
                },
                Instruction::CallMethod {
                    component_address: test.account_address,
                    method: "deposit".to_string(),
                    args: args![Variable("output_bucket"),],
                },
            ],
            // proof needed to withdraw
            vec![test.account_proof.clone()],
        )
        .unwrap();
}

fn add_liquidity(test: &mut TariswapTest, a_amount: Amount, b_amount: Amount) {
    test.template_test
        .execute_and_commit(
            vec![
                Instruction::CallMethod {
                    component_address: test.account_address,
                    method: "withdraw".to_string(),
                    args: args![test.a_resource, a_amount],
                },
                Instruction::PutLastInstructionOutputOnWorkspace {
                    key: b"a_bucket".to_vec(),
                },
                Instruction::CallMethod {
                    component_address: test.account_address,
                    method: "withdraw".to_string(),
                    args: args![test.b_resource, b_amount],
                },
                Instruction::PutLastInstructionOutputOnWorkspace {
                    key: b"b_bucket".to_vec(),
                },
                Instruction::CallMethod {
                    component_address: test.pool_component,
                    method: "add_liquidity".to_string(),
                    args: args![Variable("a_bucket"), Variable("b_bucket")],
                },
                Instruction::PutLastInstructionOutputOnWorkspace {
                    key: b"lp_bucket".to_vec(),
                },
                Instruction::CallMethod {
                    component_address: test.account_address,
                    method: "deposit".to_string(),
                    args: args![Variable("lp_bucket")],
                },
            ],
            // proof needed to withdraw (from account) and mint (the lp_resource owned by the test identity)
            // respectively
            vec![test.account_proof.clone(), test.template_test.get_test_proof()],
        )
        .unwrap();
}

fn remove_liquidity(test: &mut TariswapTest, lp_amount: Amount) {
    test.template_test
        .execute_and_commit(
            vec![
                Instruction::CallMethod {
                    component_address: test.account_address,
                    method: "withdraw".to_string(),
                    args: args![test.lp_resource, lp_amount],
                },
                Instruction::PutLastInstructionOutputOnWorkspace {
                    key: b"lp_bucket".to_vec(),
                },
                Instruction::CallMethod {
                    component_address: test.pool_component,
                    method: "remove_liquidity".to_string(),
                    args: args![Variable("lp_bucket")],
                },
                Instruction::PutLastInstructionOutputOnWorkspace {
                    key: b"pool_buckets".to_vec(),
                },
                Instruction::CallMethod {
                    component_address: test.account_address,
                    method: "deposit".to_string(),
                    args: args![Variable("pool_buckets.0"),],
                },
                Instruction::CallMethod {
                    component_address: test.account_address,
                    method: "deposit".to_string(),
                    args: args![Variable("pool_buckets.1"),],
                },
            ],
            // proof needed to withdraw (from account) and burn (the lp_resource owned by the test identity)
            // respectively
            vec![test.account_proof.clone(), test.template_test.get_test_proof()],
        )
        .unwrap();
}

fn get_pool_balance(test: &mut TariswapTest, resource_address: ResourceAddress) -> Amount {
    test.template_test
        .call_method(test.pool_component, "get_pool_balance", args![resource_address], vec![])
}

fn get_account_balance(test: &mut TariswapTest, resource_address: ResourceAddress) -> Amount {
    test.template_test
        .call_method(test.account_address, "balance", args![resource_address], vec![])
}

fn assert_swap(
    test: &mut TariswapTest,
    input_resource: &ResourceAddress,
    input_amount: i64,
    output_resource: &ResourceAddress,
    expected_output_amount: i64,
) {
    // create the amount objects
    let input_amount = Amount::new(input_amount);
    let expected_output_amount = Amount::new(expected_output_amount);

    // save the current pool balances for later comparison
    let input_pool_balance = get_pool_balance(test, *input_resource);
    let output_pool_balance = get_pool_balance(test, *output_resource);

    // call the component
    swap(test, input_resource, output_resource, input_amount);

    // check that the new pool balances are expected
    let new_input_pool_balance = get_pool_balance(test, *input_resource);
    let new_output_pool_balance = get_pool_balance(test, *output_resource);
    assert_eq!(new_input_pool_balance, input_pool_balance + input_amount);
    assert_eq!(new_output_pool_balance, output_pool_balance - expected_output_amount);
}

fn assert_add_liquidity(test: &mut TariswapTest, a_amount: i64, b_amount: i64, expected_lp_amount: i64) {
    // create the amount objects
    let a_amount = Amount::new(a_amount);
    let b_amount = Amount::new(b_amount);
    let expected_lp_amount = Amount::new(expected_lp_amount);

    // save the resource addreses to keep the compiler happy
    let a_resource = test.a_resource;
    let b_resource = test.b_resource;
    let lp_resource = test.lp_resource;

    // save the current balances for later comparison
    let pool_a_balance = get_pool_balance(test, a_resource);
    let pool_b_balance = get_pool_balance(test, b_resource);
    let account_lp_balance = get_account_balance(test, lp_resource);

    // call the component
    add_liquidity(test, a_amount, b_amount);

    // the account should have now more LP tokens
    let new_account_lp_balance = get_account_balance(test, lp_resource);
    assert_eq!(new_account_lp_balance, account_lp_balance + expected_lp_amount);

    // check pool balances
    let new_pool_a_balance = get_pool_balance(test, a_resource);
    let new_pool_b_balance = get_pool_balance(test, b_resource);
    assert_eq!(new_pool_a_balance, pool_a_balance + a_amount);
    assert_eq!(new_pool_b_balance, pool_b_balance + b_amount);
}

fn assert_remove_liquidity(
    test: &mut TariswapTest,
    lp_amount_to_remove: i64,
    expected_a_amount: i64,
    expected_b_amount: i64,
) {
    // create the amount objects
    let lp_amount_to_remove = Amount::new(lp_amount_to_remove);
    let expected_a_amount = Amount::new(expected_a_amount);
    let expected_b_amount = Amount::new(expected_b_amount);

    // save the resource addreses to keep the compiler happy
    let a_resource = test.a_resource;
    let b_resource = test.b_resource;
    let lp_resource = test.lp_resource;

    // save the current balances for later comparison
    let lp_balance = get_account_balance(test, lp_resource);
    let account_a_balance = get_account_balance(test, a_resource);
    let account_b_balance = get_account_balance(test, b_resource);

    // call the component
    remove_liquidity(test, lp_amount_to_remove);

    // check the lp tokens in account
    assert_eq!(get_account_balance(test, lp_resource), lp_balance - lp_amount_to_remove);

    // check the account balances
    let new_account_a_balance = get_account_balance(test, a_resource);
    let new_account_b_balance = get_account_balance(test, b_resource);
    assert_eq!(new_account_a_balance, account_a_balance + expected_a_amount);
    assert_eq!(new_account_b_balance, account_b_balance + expected_b_amount);
}

#[test]
fn it_swaps_fungible_tokens() {
    // init the test
    let fee = 50; // 5% market fee
    let mut test = setup(fee);

    // copy the resource addresses to keep the borrow checker happy
    let a_resource = test.a_resource;
    let b_resource = test.b_resource;

    // add some liquidity
    let liquidity_amount = 500;
    let expected_lp_amount = liquidity_amount * 2; // we provided both "a" and "b" tokens
    assert_add_liquidity(&mut test, liquidity_amount, liquidity_amount, expected_lp_amount);

    // let's do a swap, giving "A" tokens for "B" tokens
    let a_amount = 50;
    let expected_b_amount = 44; // applyng market fees and the constant product formula: b = k / a
    assert_swap(&mut test, &a_resource, a_amount, &b_resource, expected_b_amount);

    // let's do another swap
    // this time we are providing "B" tokens which are more scarce now, so we receive a more of "A" tokens in return
    let b_amount = 50;
    let expected_a_amount = 53; // applyng market fees and the constant product formula: b = k / a
    assert_swap(&mut test, &b_resource, b_amount, &a_resource, expected_a_amount);

    // remove liquidity
    let lp_amount_to_remove = 100;
    let expected_a_amount = 50;
    let expected_b_amount = 51;
    assert_remove_liquidity(&mut test, lp_amount_to_remove, expected_a_amount, expected_b_amount);
}

#[test]
fn it_detects_existing_pools() {
    // init the test
    let fee = 50; // 5% market fee
    let mut test = setup(fee);

    // copy values to keep the borrow checker happy
    let a_resource = test.a_resource;
    let b_resource = test.b_resource;

    // try creating a pool that was already created in the test setup
    assert_duplicated_pool_error(&mut test, a_resource, b_resource);

    // a pool is bidirectional, so it should also fail when we switch the pool resource order
    assert_duplicated_pool_error(&mut test, b_resource, a_resource);

    // new pools for different pairs should be able to be created without problems
    let (_, c_resource) = create_faucet_component(&mut test.template_test, "C".to_string());
    let c_transaction = new_pool_transaction(&mut test, a_resource, c_resource);
    test.template_test.execute_expect_success(c_transaction, vec![]);
}

fn assert_duplicated_pool_error(
    test: &mut TariswapTest,
    a_resource: ResourceAddress,
    b_resource: ResourceAddress
) {
    let transaction = new_pool_transaction(test, a_resource, b_resource);
    let reason = test.template_test.execute_expect_failure(
        transaction,
        vec![],
    );
    assert_reject_reason(reason, "A pool already exists for the input resources");
}

fn new_pool_transaction(
    test: &mut TariswapTest,
    a_resource: ResourceAddress,
    b_resource: ResourceAddress
) -> Transaction {
    Transaction::builder()
        .call_method(
            test.index_component,
            "create_pool",
            args![a_resource, b_resource],
        )
        // there are no user-protected methods in the tariswap
        .sign(&RistrettoSecretKey::default())
        .build()
}
