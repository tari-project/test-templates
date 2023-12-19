use tari_template_lib::args;
use tari_template_lib::prelude::Amount;
use tari_template_lib::models::{
    ComponentAddress, NonFungibleAddress, ResourceAddress,
};
use tari_template_test_tooling::crypto::RistrettoSecretKey;
use tari_template_test_tooling::TemplateTest;
use tari_transaction::Transaction;
use tari_template_lib::prelude::Metadata;
use tari_template_test_tooling::SubstateType;

use tari_engine_types::{
    virtual_substate::{VirtualSubstate, VirtualSubstateAddress},
};

use tari_template_lib::constants::XTR2;

#[test]
fn auction_period_ends_with_winning_bid() {
    let TestSetup {
        mut test,
        marketplace_component,
        seller_account,
        seller_owner_token,
        seller_key,
        seller_nft_address,
        ..
    } = setup();

    // create an auction for the NFT
    let epoch_period: u64 = 10;
    let min_price: Option<Amount> = None;
    let buy_price: Option<Amount> = None;
    let result = test.execute_expect_success(
        Transaction::builder()
            .call_method(seller_account, "withdraw", args![seller_nft_address.resource_address(), Amount(1)])
            .put_last_instruction_output_on_workspace("nft_bucket")
            .call_method(marketplace_component, "start_auction", args![Workspace("nft_bucket"), seller_account, min_price, buy_price, epoch_period])
            .put_last_instruction_output_on_workspace("seller_badge")
            .call_method(seller_account, "deposit", args![Workspace("seller_badge")])
            .sign(&seller_key)
            .build(),
        vec![seller_owner_token.clone()],
    );
    let output = test.get_previous_output_address(SubstateType::NonFungible);
    let seller_badge = output.as_non_fungible_address().unwrap().clone();

    // store the seller account balance for later checks
    let result = test.execute_expect_success(
        Transaction::builder()
            .call_method(seller_account, "balance", args![XTR2])
            .sign(&seller_key)
            .build(),
        vec![seller_owner_token.clone()],
    );
    let seller_balance = result.finalize.execution_results[0].decode::<Amount>().unwrap();

    // place a valid bid
    let (bidder1_account, bidder1_owner_token, bidder1_key) = test.create_owned_account();
    let bidder1_bid = Amount(100);
    let result = test.execute_expect_success(
        Transaction::builder()
            .call_method(bidder1_account, "withdraw", args![XTR2, bidder1_bid])
            .put_last_instruction_output_on_workspace("payment")
            .call_method(marketplace_component, "bid", args![bidder1_account, seller_nft_address, Workspace("payment")])
            .call_method(bidder1_account, "balance", args![XTR2])
            .sign(&bidder1_key)
            .build(),
        vec![bidder1_owner_token.clone()],
    );
    let bidder1_balance = result.finalize.execution_results[3].decode::<Amount>().unwrap();

    // place a higher bid 
    let (bidder2_account, bidder2_owner_token, bidder2_key) = test.create_owned_account();
    let bidder2_bid = Amount(200);
    let result = test.execute_expect_success(
        Transaction::builder()
            .call_method(bidder2_account, "withdraw", args![XTR2, bidder2_bid])
            .put_last_instruction_output_on_workspace("payment")
            .call_method(marketplace_component, "bid", args![bidder2_account, seller_nft_address, Workspace("payment")])
            .sign(&bidder2_key)
            .build(),
        vec![bidder2_owner_token.clone()],
    );

    // bidder2 is now the highest bidder, so the previous bid must have been refunded to bidder1
    let result = test.execute_expect_success(
        Transaction::builder()
            .call_method(bidder1_account, "balance", args![XTR2])
            .sign(&bidder1_key)
            .build(),
        vec![bidder1_owner_token.clone()],
    );
    let bidder1_balance_after_refund = result.finalize.execution_results[0].decode::<Amount>().unwrap();
    assert_eq!(bidder1_balance_after_refund, bidder1_balance + bidder1_bid);

    // advance the epoch so the auction period expires
    test.set_virtual_substate(VirtualSubstateAddress::CurrentEpoch, VirtualSubstate::CurrentEpoch(epoch_period + 1));

    // the winning bidder (bidder2) withdraws the NFT
    let result = test.execute_expect_success(
        Transaction::builder()
            .call_method(marketplace_component, "finish_auction", args![seller_nft_address])
            .sign(&bidder2_key)
            .build(),
        vec![bidder2_owner_token.clone()],
    );

    // the seller received the bid payment
    let result = test.execute_expect_success(
        Transaction::builder()
            .call_method(seller_account, "balance", args![XTR2])
            .sign(&seller_key)
            .build(),
        vec![seller_owner_token.clone()],
    );
    let seller_balance_after_sell = result.finalize.execution_results[0].decode::<Amount>().unwrap();
    assert_eq!(seller_balance_after_sell, seller_balance + bidder2_bid);
}

// TODO: auction_period_ends_with_no_winning_bid
// TODO: auction_finishes_by_buying_price_bid
// TODO: auction_cancelled_by_seller
// TODO: it_rejects_invalid_auctions
// TODO: it_rejects_invalid_bids
// TODO: it_rejects_invalid_auction_withdrawals
// TODO: it_rejects_invalid_auction_cancellations

struct TestSetup {
    test: TemplateTest,
    marketplace_component: ComponentAddress,
    seller_account: ComponentAddress,
    seller_owner_token: NonFungibleAddress,
    seller_key: RistrettoSecretKey,
    seller_badge_resource: ResourceAddress,
    seller_nft_address: NonFungibleAddress,
}

fn setup() -> TestSetup {
    let mut test = TemplateTest::new(["./"]);

    // create the seller account
    let (seller_account, seller_owner_token, seller_key) = test.create_owned_account();
    
    // create the NFT marketplace component
    let template = test.get_template_address("NftMarketplace");
    let result = test.execute_expect_success(
        Transaction::builder()
            .call_function(template, "new", args![])
            .sign(&seller_key)
            .build(),
        vec![seller_owner_token.clone()],
    );
    let marketplace_component = result.finalize.execution_results[0]
        .decode::<ComponentAddress>()
        .unwrap();
    let indexed = test
        .read_only_state_store()
        .inspect_component(marketplace_component)
        .unwrap();
    let seller_badge_resource = indexed
        .get_value("$.seller_badge_resource")
        .unwrap()
        .expect("seller_badge_resource not found");

    // create a new account NFT that the seller is going to put on sale
    let account_nft_template = test.get_template_address("AccountNonFungible");
    let result = test.execute_expect_success(
        Transaction::builder()
            .call_function(account_nft_template, "create", args![seller_owner_token])
            .sign(&seller_key)
            .build(),
        vec![seller_owner_token.clone()],
    );
    let account_nft_component = result.finalize.execution_results[0].decode::<ComponentAddress>().unwrap();

    let mut nft_metadata = Metadata::new();
    nft_metadata.insert("name".to_string(), "my_custom_nft".to_string());

    let result = test.execute_expect_success(
        Transaction::builder()
            .call_method(account_nft_component, "mint", args![nft_metadata])
            .put_last_instruction_output_on_workspace("nft_bucket")
            .call_method(seller_account, "deposit", args![Workspace("nft_bucket")])
            .sign(&seller_key)
            .build(),
        vec![seller_owner_token.clone()],
    );
    let output = test.get_previous_output_address(SubstateType::NonFungible);
    let seller_nft_address = output.as_non_fungible_address().unwrap().clone();

    TestSetup {
        test,
        marketplace_component,
        seller_account,
        seller_owner_token,
        seller_key,
        seller_badge_resource,
        seller_nft_address,
    }
}