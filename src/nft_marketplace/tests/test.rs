use tari_template_lib::args;
use tari_template_lib::models::{
    ComponentAddress, NonFungibleAddress, ResourceAddress,
};
use tari_template_test_tooling::crypto::RistrettoSecretKey;
use tari_template_test_tooling::TemplateTest;
use tari_transaction::Transaction;
use tari_template_lib::prelude::Metadata;
use tari_template_test_tooling::SubstateType;

#[test]
fn auction_period_ends_with_winning_bid() {
    let TestSetup {
        mut test,
        marketplace_component,
        seller_account,
        ..
    } = setup();

    // create the NFT that is going to be sold


    // create an auction for the NFT

    // place a valid bid

    // place a higher bid (the previous bid will be refunded to the bidder)

    // advance the epoch so the auction period expires

    // the winning bidder withdraws the NFT

    // the seller received the bid payment


    /*
    let result = test.execute_expect_success(
        Transaction::builder()
            .call_method(marketplace_component, "take_free_coins", args![])
            .put_last_instruction_output_on_workspace("bucket")
            .call_method(admin_account, "deposit", args![Workspace("bucket")])
            .call_method(marketplace_component, "total_supply", args![])
            .sign(&admin_key)
            .build(),
        vec![admin_proof.clone()],
    );

    let total_supply = result.finalize.execution_results[3]
        .decode::<Amount>()
        .unwrap();

    assert_eq!(total_supply, Amount(1_000_000_000));
    */
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