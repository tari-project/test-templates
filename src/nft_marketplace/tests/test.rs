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
        seller,
        seller_nft_address,
        ..
    } = setup();

    // create an auction for the NFT
    let auction = AuctionRequest {
        marketplace: marketplace_component,
        seller: seller.clone(),
        nft: seller_nft_address.clone(),
        min_price: None,
        buy_price: None,
        epoch_period: 10,
    };
    let _seller_badge = create_auction(&mut test, &auction);

    // store the seller account balance for later checks
    let seller_balance = get_account_balance(&mut test, &seller);

    // place a bid
    let bidder1 = create_account(&mut test);
    let bid1 = BidRequest {
        marketplace: marketplace_component,
        bidder: bidder1.clone(),
        nft: seller_nft_address.clone(),
        bid: Amount(100),
    };
    bid(&mut test, &bid1);
    let bidder1_balance = get_account_balance(&mut test, &bidder1);

    // place a higher bid 
    let bidder2 = create_account(&mut test);
    let bid2 = BidRequest {
        marketplace: marketplace_component,
        bidder: bidder2.clone(),
        nft: seller_nft_address.clone(),
        bid: Amount(200),
    };
    bid(&mut test, &bid2);

    // bidder2 is now the highest bidder, so the previous bid must have been refunded to bidder1
    let bidder1_balance_after_refund = get_account_balance(&mut test, &bidder1);
    assert_eq!(bidder1_balance_after_refund, bidder1_balance + bid1.bid);

    // advance the epoch so the auction period expires
    set_epoch(&mut test, auction.epoch_period + 1);

    // the winning bidder (bidder2) withdraws the NFT
    let finish = FinishRequest {
        marketplace: marketplace_component,
        account: bidder2.clone(),
        nft: seller_nft_address.clone(),
    };
    finish_auction(&mut test, &finish);

    // the seller received the bid payment
    let seller_balance_after_sell = get_account_balance(&mut test, &seller);
    assert_eq!(seller_balance_after_sell, seller_balance + bid2.bid);
}

// TODO: auction_period_ends_with_no_winning_bid
// TODO: auction_finishes_by_buying_price_bid
// TODO: auction_cancelled_by_seller
// TODO: it_rejects_invalid_auctions
// TODO: it_rejects_invalid_bids
// TODO: it_rejects_invalid_auction_withdrawals
// TODO: it_rejects_invalid_auction_cancellations

#[derive(Clone, Debug)]
struct Account {
    pub component: ComponentAddress,
    pub owner_token: NonFungibleAddress,
    pub key: RistrettoSecretKey,
}

struct TestSetup {
    test: TemplateTest,
    marketplace_component: ComponentAddress,
    seller: Account,
    seller_badge_resource: ResourceAddress,
    seller_nft_address: NonFungibleAddress,
}

fn setup() -> TestSetup {
    let mut test = TemplateTest::new(["./"]);

    // create the seller account
    let (seller_account, seller_owner_token, seller_key) = test.create_owned_account();
    let seller = Account {
        component: seller_account,
        owner_token: seller_owner_token,
        key: seller_key
    };
    
    // create the NFT marketplace component
    let template = test.get_template_address("NftMarketplace");
    let result = test.execute_expect_success(
        Transaction::builder()
            .call_function(template, "new", args![])
            .sign(&seller.key)
            .build(),
        vec![seller.owner_token.clone()],
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
            .call_function(account_nft_template, "create", args![seller.owner_token])
            .sign(&seller.key)
            .build(),
        vec![seller.owner_token.clone()],
    );
    let account_nft_component = result.finalize.execution_results[0].decode::<ComponentAddress>().unwrap();

    let mut nft_metadata = Metadata::new();
    nft_metadata.insert("name".to_string(), "my_custom_nft".to_string());

    test.execute_expect_success(
        Transaction::builder()
            .call_method(account_nft_component, "mint", args![nft_metadata])
            .put_last_instruction_output_on_workspace("nft_bucket")
            .call_method(seller.component, "deposit", args![Workspace("nft_bucket")])
            .sign(&seller.key)
            .build(),
        vec![seller.owner_token.clone()],
    );
    let output = test.get_previous_output_address(SubstateType::NonFungible);
    let seller_nft_address = output.as_non_fungible_address().unwrap().clone();

    TestSetup {
        test,
        marketplace_component,
        seller,
        seller_badge_resource,
        seller_nft_address,
    }
}

fn create_account(test: &mut TemplateTest) -> Account {
    let (component, owner_token, key) = test.create_owned_account();
    Account { component, owner_token, key }
}

fn get_account_balance(test: &mut TemplateTest, account: &Account) -> Amount {
    let result = test.execute_expect_success(
        Transaction::builder()
            .call_method(account.component, "balance", args![XTR2])
            .sign(&account.key)
            .build(),
        vec![account.owner_token.clone()],
    );
    let balance = result.finalize.execution_results[0].decode::<Amount>().unwrap();
    balance
}

#[derive(Clone, Debug)]
struct AuctionRequest {
    marketplace: ComponentAddress,
    seller: Account,
    nft: NonFungibleAddress,
    min_price: Option<Amount>,
    buy_price: Option<Amount>,
    epoch_period: u64,
}

// returns the seller badge
fn create_auction(test: &mut TemplateTest, req: &AuctionRequest) -> NonFungibleAddress {
    test.execute_expect_success(
        Transaction::builder()
            .call_method(req.seller.component, "withdraw", args![req.nft.resource_address(), Amount(1)])
            .put_last_instruction_output_on_workspace("nft_bucket")
            .call_method(req.marketplace, "start_auction", args![
                Workspace("nft_bucket"),
                req.seller.component,
                req.min_price,
                req.buy_price,
                req.epoch_period])
            .put_last_instruction_output_on_workspace("seller_badge")
            .call_method(req.seller.component, "deposit", args![Workspace("seller_badge")])
            .sign(&req.seller.key)
            .build(),
        vec![req.seller.owner_token.clone()],
    );
    let output = test.get_previous_output_address(SubstateType::NonFungible);
    let seller_badge = output.as_non_fungible_address().unwrap().clone();
    seller_badge
}

#[derive(Clone, Debug)]
struct BidRequest {
    marketplace: ComponentAddress,
    bidder: Account,
    nft: NonFungibleAddress,
    bid: Amount,
}

fn bid(test: &mut TemplateTest, req: &BidRequest) {
    test.execute_expect_success(
        Transaction::builder()
            .call_method(req.bidder.component, "withdraw", args![XTR2, req.bid])
            .put_last_instruction_output_on_workspace("payment")
            .call_method(req.marketplace, "bid", args![req.bidder.component, req.nft, Workspace("payment")])
            .sign(&req.bidder.key)
            .build(),
        vec![req.bidder.owner_token.clone()],
    );
}

fn set_epoch(test: &mut TemplateTest, new_epoch: u64) {
    test.set_virtual_substate(VirtualSubstateAddress::CurrentEpoch, VirtualSubstate::CurrentEpoch(new_epoch));
}

#[derive(Clone, Debug)]
struct FinishRequest {
    marketplace: ComponentAddress,
    account: Account,
    nft: NonFungibleAddress,
}

fn finish_auction(test: &mut TemplateTest, req: &FinishRequest) {
    test.execute_expect_success(
        Transaction::builder()
            .call_method(req.marketplace, "finish_auction", args![req.nft])
            .sign(&req.account.key)
            .build(),
        vec![req.account.owner_token.clone()],
    );
}