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
use std::collections::BTreeMap;

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
    address: ComponentAddress,
    bid: Vault,
}

#[template]
mod nft_marketplace {
    use super::*;

    pub struct NftMarketplace {
        auctions: BTreeMap<NonFungibleAddress, Auction>,
        seller_badge_resource: ResourceAddress,
    }

    impl NftMarketplace {
        pub fn new() -> Self {
            Self {
                auctions: BTreeMap::new(),
                seller_badge_resource: ResourceBuilder::non_fungible()
                    // TODO: proper access control. Is it possible to allow only this component to mint&burn? 
                    .mintable(AccessRule::AllowAll)
                    .burnable(AccessRule::DenyAll)
                    .build(),
            }
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
            immutable_data.insert("nft_address", nft_address.to_string());
            ResourceManager::get(self.seller_badge_resource)
                .mint_non_fungible(badge_id, &immutable_data, &())
        }
    }
}
