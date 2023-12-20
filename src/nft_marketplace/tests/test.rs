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
use tari_template_test_tooling::support::assert_error::assert_reject_reason;

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
    let seller_balance = get_account_tari_balance(&mut test, &seller);

    // place a bid
    let bidder1 = create_account(&mut test);
    let bid1 = BidRequest {
        marketplace: marketplace_component,
        bidder: bidder1.clone(),
        nft: seller_nft_address.clone(),
        bid: Amount(100),
    };
    bid(&mut test, &bid1);
    let bidder1_balance = get_account_tari_balance(&mut test, &bidder1);

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
    let bidder1_balance_after_refund = get_account_tari_balance(&mut test, &bidder1);
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
    let seller_balance_after_sell = get_account_tari_balance(&mut test, &seller);
    assert_eq!(seller_balance_after_sell, seller_balance + bid2.bid);
}

#[test]
fn auction_period_ends_with_no_winning_bid() {
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

    // the NFT is no longer in the seller's account
    let seller_nft_balance = get_account_balance(&mut test, &seller, &seller_nft_address.resource_address());
    assert_eq!(seller_nft_balance, Amount(0));

    // advance the epoch so the auction period expires
    set_epoch(&mut test, auction.epoch_period + 1);

    // the seller withdraws the NFT
    let finish = FinishRequest {
        marketplace: marketplace_component,
        account: seller.clone(),
        nft: seller_nft_address.clone(),
    };
    finish_auction(&mut test, &finish);

    // the nft has been deposited into the seller again
    let seller_nft_balance = get_account_balance(&mut test, &seller, &seller_nft_address.resource_address());
    assert_eq!(seller_nft_balance, Amount(1));
}

#[test]
fn auction_finishes_by_buying_price_bid() {
    let TestSetup {
        mut test,
        marketplace_component,
        seller,
        seller_nft_address,
        ..
    } = setup();

    // create an auction for the NFT
    let buy_price = Amount(100);
    let auction = AuctionRequest {
        marketplace: marketplace_component,
        seller: seller.clone(),
        nft: seller_nft_address.clone(),
        min_price: None,
        buy_price: Some(buy_price),
        epoch_period: 10,
    };
    let _seller_badge = create_auction(&mut test, &auction);

    // store the seller account balance for later checks
    let seller_balance = get_account_tari_balance(&mut test, &seller);

    // place a bid that matches the buying price of the NFT
    let bidder1 = create_account(&mut test);
    let bid1 = BidRequest {
        marketplace: marketplace_component,
        bidder: bidder1.clone(),
        nft: seller_nft_address.clone(),
        bid: buy_price,
    };
    bid(&mut test, &bid1);

    // Notice that we DON'T advace the epoch period
    // so the Auction has not expired

    // the bidder received the NFT, because he paid the buy price

    // the seller received the bid payment
    let seller_balance_after_sell = get_account_tari_balance(&mut test, &seller);
    assert_eq!(seller_balance_after_sell, seller_balance + buy_price);
}

#[test]
fn auction_cancelled_by_seller() {
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
    let seller_badge = create_auction(&mut test, &auction);

    // store the seller account balance for later checks
    let seller_balance = get_account_tari_balance(&mut test, &seller);

    // place a bid that matches the buying price of the NFT
    let bidder1 = create_account(&mut test);
    let bid1 = BidRequest {
        marketplace: marketplace_component,
        bidder: bidder1.clone(),
        nft: seller_nft_address.clone(),
        bid: Amount(100),
    };
    bid(&mut test, &bid1);
    let bidder1_balance = get_account_tari_balance(&mut test, &bidder1);

    // Notice that we DON'T advance the epoch period
    // so the Auction has not expired

    // the seller cancels the NFT
    let finish = CancelRequest {
        marketplace: marketplace_component,
        account: seller.clone(),
        nft: seller_nft_address.clone(),
        seller_badge: seller_badge.clone()
    };
    cancel_auction(&mut test, &finish);

    // the nft has been deposited into the seller again
    let seller_nft_balance = get_account_balance(&mut test, &seller, &seller_nft_address.resource_address());
    assert_eq!(seller_nft_balance, Amount(1));

    // the existing bid has been refunded
    let bidder1_balance_after_cancel = get_account_tari_balance(&mut test, &bidder1);
    assert_eq!(bidder1_balance_after_cancel, bidder1_balance + bid1.bid);
}

#[test]
fn it_rejects_invalid_auctions() {
    let TestSetup {
        mut test,
        marketplace_component,
        account_nft_component,
        seller,
        seller_nft_address,
        ..
    } = setup();

    // reject if resource is not an nft
    // we test it by trying to auction a Tari fungible token
    let reason = test.execute_expect_failure(
        Transaction::builder()
        .call_method(seller.component, "withdraw", args![XTR2, Amount(1)]) // invalid resource
        .put_last_instruction_output_on_workspace("nft_bucket")
        .call_method(marketplace_component, "start_auction", args![
            Workspace("nft_bucket"),
            seller.component,
            None::<Amount>,
            None::<Amount>,
            10])
        .put_last_instruction_output_on_workspace("seller_badge")
        .call_method(seller.component, "deposit", args![Workspace("seller_badge")])
        .sign(&seller.key)
        .build(),
        vec![seller.owner_token.clone()],
    );
    assert_reject_reason(reason, "The resource is not a NFT");

    // reject if multiple nfts in the bucket
    mint_account_nft(&mut test, &seller, &account_nft_component);
    let reason = test.execute_expect_failure(
        Transaction::builder()
        .call_method(seller.component, "withdraw", args![seller_nft_address.resource_address(), Amount(2)]) // invalid bucket
        .put_last_instruction_output_on_workspace("nft_bucket")
        .call_method(marketplace_component, "start_auction", args![
            Workspace("nft_bucket"),
            seller.component,
            None::<Amount>,
            None::<Amount>,
            10])
        .put_last_instruction_output_on_workspace("seller_badge")
        .call_method(seller.component, "deposit", args![Workspace("seller_badge")])
        .sign(&seller.key)
        .build(),
        vec![seller.owner_token.clone()],
    );
    assert_reject_reason(reason, "Can only start an auction of a single NFT");

    // reject if the auction period is invalid
    let reason = test.execute_expect_failure(
        Transaction::builder()
        .call_method(seller.component, "withdraw", args![seller_nft_address.resource_address(), Amount(1)])
        .put_last_instruction_output_on_workspace("nft_bucket")
        .call_method(marketplace_component, "start_auction", args![
            Workspace("nft_bucket"),
            seller.component,
            None::<Amount>,
            None::<Amount>,
            0]) // invalid period
        .put_last_instruction_output_on_workspace("seller_badge")
        .call_method(seller.component, "deposit", args![Workspace("seller_badge")])
        .sign(&seller.key)
        .build(),
        vec![seller.owner_token.clone()],
    );
    assert_reject_reason(reason, "Invalid auction period");

    // reject if the seller account is not an account component
    let reason = test.execute_expect_failure(
        Transaction::builder()
        .call_method(seller.component, "withdraw", args![seller_nft_address.resource_address(), Amount(1)])
        .put_last_instruction_output_on_workspace("nft_bucket")
        .call_method(marketplace_component, "start_auction", args![
            Workspace("nft_bucket"),
            account_nft_component, // invalid component, it's not an account
            None::<Amount>,
            None::<Amount>,
            10])
        .put_last_instruction_output_on_workspace("seller_badge")
        .call_method(seller.component, "deposit", args![Workspace("seller_badge")])
        .sign(&seller.key)
        .build(),
        vec![seller.owner_token.clone()],
    );
    assert_reject_reason(reason, "Invalid bidder account");
}

#[test]
fn it_rejects_invalid_bids() {
    let TestSetup {
        mut test,
        marketplace_component,
        account_nft_component,
        seller,
        seller_nft_address,
        ..
    } = setup();

    // reject if the auction does not exist
    let bidder = create_account(&mut test);
    let reason = test.execute_expect_failure(
        Transaction::builder()
            .call_method(bidder.component, "withdraw", args![XTR2, Amount(200)])
            .put_last_instruction_output_on_workspace("payment")
            .call_method(marketplace_component, "bid", args![bidder.component, seller_nft_address, Workspace("payment")])
            .sign(&bidder.key)
            .build(),
        vec![bidder.owner_token.clone()],
    );
    assert_reject_reason(reason, "Auction does not exist");

    // lets publish a valid auction for the rest of the asserts
    let min_price = Amount(100);
    let buy_price = Amount(500);
    let auction_period = 10;
    let auction = AuctionRequest {
        marketplace: marketplace_component,
        seller: seller.clone(),
        nft: seller_nft_address.clone(),
        min_price: Some(min_price),
        buy_price: Some(buy_price),
        epoch_period: auction_period,
    };
    let seller_badge = create_auction(&mut test, &auction);

    // reject if the payment is not in Tari
    let bidder_nft_component = create_account_nft_component(&mut test, &bidder);
    let bidder_nft_address = mint_account_nft(&mut test, &bidder, &bidder_nft_component);
    let reason = test.execute_expect_failure(
        Transaction::builder()
            .call_method(bidder.component, "withdraw", args![bidder_nft_address.resource_address(), Amount(1)])
            .put_last_instruction_output_on_workspace("payment")
            .call_method(marketplace_component, "bid", args![bidder.component, seller_nft_address, Workspace("payment")])
            .sign(&bidder.key)
            .build(),
        vec![bidder.owner_token.clone()],
    );
    assert_reject_reason(reason, "Invalid payment resource, the marketplace only accepts Tari (XTR2) tokens");

    // reject if buy price is too low
    let reason = test.execute_expect_failure(
        Transaction::builder()
            .call_method(bidder.component, "withdraw", args![XTR2, min_price-1])
            .put_last_instruction_output_on_workspace("payment")
            .call_method(marketplace_component, "bid", args![bidder.component, seller_nft_address, Workspace("payment")])
            .sign(&bidder.key)
            .build(),
        vec![bidder.owner_token.clone()],
    );
    assert_reject_reason(reason, "Minimum price not met");

    // reject if buy price is too high (higher than the buy price)
    let reason = test.execute_expect_failure(
        Transaction::builder()
            .call_method(bidder.component, "withdraw", args![XTR2, buy_price+1])
            .put_last_instruction_output_on_workspace("payment")
            .call_method(marketplace_component, "bid", args![bidder.component, seller_nft_address, Workspace("payment")])
            .sign(&bidder.key)
            .build(),
        vec![bidder.owner_token.clone()],
    );
    assert_reject_reason(reason, "Payment exceeds the buying price");

    // reject if the bidder account is not an account component
    let reason = test.execute_expect_failure(
        Transaction::builder()
            .call_method(bidder.component, "withdraw", args![XTR2, Amount(1)])
            .put_last_instruction_output_on_workspace("payment")
            // using the bidder's nft component address instead of its account
            .call_method(marketplace_component, "bid", args![bidder_nft_component, seller_nft_address, Workspace("payment")])
            .sign(&bidder.key)
            .build(),
        vec![bidder.owner_token.clone()],
    );
    assert_reject_reason(reason, "Invalid bidder account");

    // reject if the auction has expired
    set_epoch(&mut test, auction_period + 1);
    let reason = test.execute_expect_failure(
        Transaction::builder()
            .call_method(bidder.component, "withdraw", args![XTR2, min_price+1])
            .put_last_instruction_output_on_workspace("payment")
            .call_method(marketplace_component, "bid", args![bidder.component, seller_nft_address, Workspace("payment")])
            .sign(&bidder.key)
            .build(),
        vec![bidder.owner_token.clone()],
    );
    assert_reject_reason(reason, "Auction has expired");
}

#[test]
fn it_rejects_invalid_auction_finish() {
    let TestSetup {
        mut test,
        marketplace_component,
        account_nft_component,
        seller,
        seller_nft_address,
        ..
    } = setup();

    // reject if the auction does not exist
    let bidder = create_account(&mut test);
    let reason = test.execute_expect_failure(
        Transaction::builder()
            .call_method(marketplace_component, "finish_auction", args![seller_nft_address])
            .sign(&bidder.key)
            .build(),
        vec![bidder.owner_token.clone()],
    );
    assert_reject_reason(reason, "Auction does not exist");

    // let's publish a valid auction for the rest of the asserts
    let min_price = Amount(100);
    let buy_price = Amount(500);
    let auction_period = 10;
    let auction = AuctionRequest {
        marketplace: marketplace_component,
        seller: seller.clone(),
        nft: seller_nft_address.clone(),
        min_price: Some(min_price),
        buy_price: Some(buy_price),
        epoch_period: auction_period,
    };
    let seller_badge = create_auction(&mut test, &auction);

    // let's make the bidder win the auction
    let bid_req = BidRequest {
        marketplace: marketplace_component,
        bidder: bidder.clone(),
        nft: seller_nft_address.clone(),
        bid: min_price + 1,
    };
    bid(&mut test, &bid_req);

    // the auction period has not ended yet, so the bidder should not be able to finish
    let reason = test.execute_expect_failure(
        Transaction::builder()
            .call_method(marketplace_component, "finish_auction", args![seller_nft_address])
            .sign(&bidder.key)
            .build(),
        vec![bidder.owner_token.clone()],
    );
    assert_reject_reason(reason, "Auction is still in progress");
}

// TODO: it_rejects_invalid_auction_cancellations

#[derive(Clone, Debug)]
struct Account {
    pub component: ComponentAddress,
    pub owner_token: NonFungibleAddress,
    pub key: RistrettoSecretKey,
}

struct TestSetup {
    test: TemplateTest,
    account_nft_component: ComponentAddress,
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
    let account_nft_component = create_account_nft_component(&mut test, &seller);
    let seller_nft_address = mint_account_nft(&mut test, &seller, &account_nft_component);

    TestSetup {
        test,
        marketplace_component,
        account_nft_component,
        seller,
        seller_badge_resource,
        seller_nft_address,
    }
}

fn create_account(test: &mut TemplateTest) -> Account {
    let (component, owner_token, key) = test.create_owned_account();
    Account { component, owner_token, key }
}

fn get_account_balance(test: &mut TemplateTest, account: &Account, resource: &ResourceAddress) -> Amount {
    let result = test.execute_expect_success(
        Transaction::builder()
            .call_method(account.component, "balance", args![resource])
            .sign(&account.key)
            .build(),
        vec![account.owner_token.clone()],
    );
    let balance = result.finalize.execution_results[0].decode::<Amount>().unwrap();
    balance
}

fn get_account_tari_balance(test: &mut TemplateTest, account: &Account) -> Amount {
    return get_account_balance(test, account, &XTR2);
}

fn create_account_nft_component(test: &mut TemplateTest, account: &Account) -> ComponentAddress {
    let account_nft_template = test.get_template_address("AccountNonFungible");
    let result = test.execute_expect_success(
        Transaction::builder()
            .call_function(account_nft_template, "create", args![account.owner_token])
            .sign(&account.key)
            .build(),
        vec![account.owner_token.clone()],
    );
    let account_nft_component = result.finalize.execution_results[0].decode::<ComponentAddress>().unwrap();
    account_nft_component
}

fn mint_account_nft(test: &mut TemplateTest, account: &Account, account_nft_component: &ComponentAddress) -> NonFungibleAddress {
    let mut nft_metadata = Metadata::new();
    nft_metadata.insert("name".to_string(), "my_custom_nft".to_string());

    test.execute_expect_success(
        Transaction::builder()
            .call_method(*account_nft_component, "mint", args![nft_metadata])
            .put_last_instruction_output_on_workspace("nft_bucket")
            .call_method(account.component, "deposit", args![Workspace("nft_bucket")])
            .sign(&account.key)
            .build(),
        vec![account.owner_token.clone()],
    );
    let output = test.get_previous_output_address(SubstateType::NonFungible);
    let minted_nft_address = output.as_non_fungible_address().unwrap().clone();
    minted_nft_address
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

#[derive(Clone, Debug)]
struct CancelRequest {
    marketplace: ComponentAddress,
    account: Account,
    nft: NonFungibleAddress,
    seller_badge: NonFungibleAddress,
}

fn cancel_auction(test: &mut TemplateTest, req: &CancelRequest) {
    test.execute_expect_success(
        Transaction::builder()
            .call_method(req.account.component, "withdraw_non_fungible", args![
                req.seller_badge.resource_address(),
                req.seller_badge.id()
            ])
            .put_last_instruction_output_on_workspace("seller_badge")
            .call_method(req.marketplace, "cancel_auction", args![Workspace("seller_badge")])
            .sign(&req.account.key)
            .build(),
        vec![req.account.owner_token.clone()],
    );
}