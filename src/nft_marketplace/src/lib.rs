//   Copyright 2023. The Tari Project
//
//   Redistribution and use in source and binary forms, with or without modification, are permitted provided that the
//   following conditions are met:
//
//   1. Redistributions of source code must retain the above copyright notice, this list of conditions and the following
//   disclaimer.
//
//   2. Redistributions in binary form must reproduce the above copyright notice, this list of conditions and the
//   following disclaimer in the documentation and/or other materials provided with the distribution.
//
//   3. Neither the name of the copyright holder nor the names of its contributors may be used to endorse or promote
//   products derived from this software without specific prior written permission.
//
//   THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES,
//   INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
//   DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
//   SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
//   SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY,
//   WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
//   USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

use tari_template_lib::prelude::*;
use tari_template_lib::Hash;

use std::collections::BTreeMap;
use std::str::FromStr;

/// TODO: create constant in template_lib for account template address (and other builtin templates)
pub const ACCOUNT_TEMPLATE_ADDRESS: Hash = Hash::from_array([0u8; 32]);

// the immutable field name on the seller badges used to reference which nft is being sold
// used to allow the seller the option to cancel
// TODO: there should be only one metadata field needed (for the whole NonFungibleAddress)
//       but there is no easy way to parse back the whole address string into a NonFungibleAddress
//       so here we workaround by storing the resource and the id separately
pub const SELLER_BADGE_RESOURCE_FIELD: &str = "resource";
pub const SELLER_BADGE_ID_FIELD: &str = "id";

/// Simple English-like auctions
/// The winner needs to claim the nft after the bidding period finishes. For simplicity, no marketplace fees are
/// considered. There exist a lot more approaches to auctions, we can highlight:
///     - Price descending, dutch-like auctions. The first bidder gets the nft right away, no need to wait or claim
///       afterwards
///     - Blind auctions, were bids are not known until the end. This requires cryptography support, and implies that
///       all bidder's funds will be locked until the end of the auction
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Auction {
    // The NFT will be locked, so the user gives away control to the marketplace
    // There are other approaches to this, like just allowing the seller to complete and confirm the bid at the end
    vault: Vault,

    // address of the account component of the seller
    seller_address: ComponentAddress,

    // minimum required price for a bid
    min_price: Option<Amount>,

    // price at which the NFT will be sold automatically
    buy_price: Option<Amount>,

    // Holds the current highest bidder, it's replaced when a new highest bidder appears
    highest_bid: Option<Bid>,

    // Time sensitive logic is a big issue, we need custom support for it. I see two options:
    //      1. Ad hoc protocol in the second layer to agree on timestamps (inside of a commitee? globally?)
    //      2. Leverage the base layer block number (~3 minute intervals)
    //      3. Use the current epoch (~30 min intervals)
    // We are going with (3) here. But either way this means custom utils and that some external state influences
    // execution
    ending_epoch: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Bid {
    bidder_account: ComponentAddress,
    vault: Vault,
}

#[template]
mod nft_marketplace {
    use super::*;

    pub struct NftMarketplace {
        auctions: BTreeMap<NonFungibleAddress, Auction>,
        seller_badge_resource: ResourceAddress,
    }

    impl NftMarketplace {
        pub fn new() -> Component<NftMarketplace> {
            let component_access_rules = AccessRules::new()
                .default(AccessRule::AllowAll);
            let auctions = BTreeMap::new();
            let seller_badge_resource = ResourceBuilder::non_fungible()
                    // TODO: proper access control. Is it possible to allow only this component to mint&burn? 
                    .mintable(AccessRule::AllowAll)
                    .burnable(AccessRule::AllowAll)
                    .build();

            Component::new(Self {
                auctions,
                seller_badge_resource
            })
                .with_access_rules(component_access_rules)
                .create()
        }

        pub fn get_auction(&self, nft_address: NonFungibleAddress) -> Option<Auction> {
            self.auctions.get(&nft_address).cloned()
        }

        // convenience method for external APIs and interfaces
        // TODO: support for advanced filtering (price ranges, auctions about to end, etc.) could be desirable
        pub fn get_auctions(&self) -> BTreeMap<NonFungibleAddress, Auction> {
            self.auctions.clone()
        }  

        // returns a badge used to cancel the sell order in the future
        // the badge will contain immutable metadata referencing the nft being sold
        pub fn start_auction(
            &mut self,
            nft_bucket: Bucket,
            seller_address: ComponentAddress,
            min_price: Option<Amount>,
            buy_price: Option<Amount>,
            epoch_period: u64,
        ) -> Bucket {
            assert!(
                nft_bucket.resource_type() == ResourceType::NonFungible,
                "The resource is not a NFT"
            );

            assert!(
                nft_bucket.amount() == Amount(1),
                "Can only start an auction of a single NFT"
            );

            assert!(epoch_period > 0, "Invalid auction period");

            // needed to ensure that we can process the auction when it ends
            Self::assert_component_is_account(seller_address);

            let auction = Auction {
                vault: Vault::from_bucket(nft_bucket),
                seller_address,
                min_price,
                buy_price,
                highest_bid: None,
                ending_epoch: Consensus::current_epoch() + epoch_period,
            };

            // TODO: we need a "get_non_fungible_address" method in the template_lib
            let nft_resource = auction.vault.resource_address();
            let nft_id = &auction.vault.get_non_fungible_ids()[0];
            let nft_address = NonFungibleAddress::new(nft_resource, nft_id.clone());

            self.auctions.insert(nft_address.clone(), auction);

            // mint and return a badge to be used later for (optionally) canceling the auction by the seller
            let badge_id = NonFungibleId::random();
            // the data MUST be immutable, to avoid security exploits (changing the nft which it points to afterwards)
            let mut immutable_data = Metadata::new();
            immutable_data.insert(SELLER_BADGE_RESOURCE_FIELD, nft_resource.to_string());
            immutable_data.insert(SELLER_BADGE_ID_FIELD, nft_id.to_string());
            ResourceManager::get(self.seller_badge_resource)
                .mint_non_fungible(badge_id, &immutable_data, &())
        }

        // process a new bid for an ongoing auction
        pub fn bid(&mut self, bidder_account_address: ComponentAddress, nft_address: NonFungibleAddress, payment: Bucket) {
            let auction = self.auctions.get_mut(&nft_address).expect("Auction does not exist");

            assert!(Consensus::current_epoch() < auction.ending_epoch, "Auction has expired");

            assert_eq!(
                payment.resource_address(),
                XTR2,
                "Invalid payment resource, the marketplace only accepts Tari (XTR2) tokens"
            );

            // validate that the bidder account is really an account
            // so we can deposit the refund later if a higher bidder comes
            // otherwise an attacker could block newer higher bids 
            Self::assert_component_is_account(bidder_account_address);

            // check that the minimum price (if set) is met
            if let Some(min_price) = auction.min_price {
                assert!(payment.amount() >= min_price, "Minimum price not met");
            }

            // immediatly refund the previous highest bidder if there is one
            if let Some(highest_bid) = &mut auction.highest_bid {
                assert!(
                    payment.amount() > highest_bid.vault.balance(),
                    "There is a higher bid placed"
                );
                let previous_bidder_account = ComponentManager::get(highest_bid.bidder_account);
                let refund_bucket = highest_bid.vault.withdraw_all();
                // TODO: improve call method generics when there is no return value
                previous_bidder_account.call::<_,()>("deposit".to_string(), args![refund_bucket]);

                // update the highest bidder in the auction
                highest_bid.bidder_account = bidder_account_address;
                highest_bid.vault.deposit(payment.clone());
            } else {
                // the bidder is the first one to place a bid
                let highest_bid = Bid {
                    bidder_account: bidder_account_address,
                    vault: Vault::from_bucket(payment.clone()),
                };
                auction.highest_bid = Some(highest_bid);
            }

            // if the bid meets the buying price, we process the sell immediatly
            if let Some(buy_price) = auction.buy_price {
                assert!(payment.amount() <= buy_price, "Payment exceeds the buying price");
                if payment.amount() == buy_price {
                    self.process_auction_payments(nft_address);
                }
            }
        }

        // finish the auction by sending the NFT and payment to the respective accounts
        // used by a bid seller to receive the bid payment, or by the buyer to get the NFT, whatever happens first
        pub fn finish_auction(&mut self, nft_address: NonFungibleAddress) {
            let auction = self.auctions.get_mut(&nft_address).expect("Auction does not exist");

            assert!(
                Consensus::current_epoch() >= auction.ending_epoch,
                "Auction is still in progress"
            );

            self.process_auction_payments(nft_address);
        }

        // the seller wants to cancel the auction
        pub fn cancel_auction(&mut self, seller_badge_bucket: Bucket) {
            // we check that the badge has been created by the marketplace
            assert!(
                seller_badge_bucket.resource_address() == self.seller_badge_resource,
                "Invalid seller badge resource"
            );

            // the seller badge references the corresponding nft address in its immutable data
            // TODO: we need a more convenient method in the template_lib to extract NFT data from a bucket
            let seller_badge_id = &seller_badge_bucket.get_non_fungible_ids()[0];
            let seller_badge = ResourceManager::get(self.seller_badge_resource).get_non_fungible(&seller_badge_id);
            let nft_metadata = seller_badge.get_data::<Metadata>();
            let nft_resource_str = nft_metadata.get(SELLER_BADGE_RESOURCE_FIELD)
                .expect("Invalid seller badge: No NFT resource field in metadata");
            let nft_resource = ResourceAddress::from_str(&nft_resource_str)
                .expect("Invalid seller badge: Invalid NFT resource field in metadata");
            let nft_id_str = nft_metadata.get(SELLER_BADGE_ID_FIELD)
                .expect("Invalid seller badge: No NFT id field in metadata");
            let nft_id = NonFungibleId::try_from_string(nft_id_str)
                .expect("Invalid seller badge: Invalid NFT id field in metadata");
            let nft_address = NonFungibleAddress::new(nft_resource, nft_id);
            let auction = self.auctions.get_mut(&nft_address)
                .expect("Auction does not exist");

            // an auction cannot be cancelled if it has ended
            assert!(Consensus::current_epoch() < auction.ending_epoch, "Auction has ended");

            // we are canceling the bid
            // so we need to pay back the highest bidded (if there's one)
            if let Some(highest_bid) = &mut auction.highest_bid {
                let bidder_account = ComponentManager::get(highest_bid.bidder_account);
                let refund_bucket = highest_bid.vault.withdraw_all();
                bidder_account.call::<_,()>("deposit".to_string(), args![refund_bucket]);
                auction.highest_bid = None;
            }

            // at this point there is no bidder
            // so the payment process will just send the NFT back to the seller
            self.process_auction_payments(nft_address);
        }

        fn assert_component_is_account(component_address: ComponentAddress) {
            let component = ComponentManager::get(component_address);
            assert!(component.get_template_address() == ACCOUNT_TEMPLATE_ADDRESS, "Invalid bidder account");
        }

        // this method MUST ALWAYS be private, to prevent auction cancellation by unauthorized third parties
        fn process_auction_payments(&mut self, nft_address: NonFungibleAddress) {
            let auction = self.auctions.get_mut(&nft_address).expect("Auction does not exist");

            let seller_account = ComponentManager::get(auction.seller_address);
            let nft_bucket = auction.vault.withdraw_all();

            if let Some(highest_bid) = &mut auction.highest_bid {
                // deposit the nft to the bidder
                let bidder_account = ComponentManager::get(highest_bid.bidder_account);
                bidder_account.call::<_,()>("deposit".to_string(), args![nft_bucket]);

                // deposit the funds to the seller
                let payment = highest_bid.vault.withdraw_all();
                seller_account.call::<_,()>("deposit".to_string(), args![payment]);
            } else {
                // no bidders in the auction, so just return the NFT to the seller
                seller_account.call::<_,()>("deposit".to_string(), args![nft_bucket]);
            }

            // TODO: burn the seller badge to avoid it being used again

            // TODO: we cannot remove the auction because the network does not allow to delete the auction vault (OrphanedSubstate)
            // self.auctions.remove(&nft_address);
        }
    }
}
