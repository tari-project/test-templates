//   Copyright 2024. The Tari Project
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

#[template]
mod tariswap_index {
    use super::*;

    type PoolKey = (ResourceAddress, ResourceAddress);

    pub struct TariswapIndex {
        pool_template: TemplateAddress,
        pools: BTreeMap<PoolKey, ComponentAddress>,
        // All pools in the index will have the same swap market fee, which is represented as a per thousand amount
        market_fee: u16,
    }

    impl TariswapIndex {
        pub fn new(pool_template: TemplateAddress, market_fee: u16) -> Component<Self> {
            Component::new(Self {
                pool_template,
                pools: BTreeMap::new(),
                market_fee
            })
            .with_access_rules(AccessRules::allow_all())
            .create()
        }

        // convenience method for external APIs and interfaces
        pub fn get_pools(&self) -> BTreeMap<(ResourceAddress, ResourceAddress), ComponentAddress> {
            self.pools.clone()
        }

        pub fn create_pool(
            &mut self,
            a_addr: ResourceAddress,
            b_addr: ResourceAddress,
        ) -> ComponentAddress {
            let pool_key = Self::build_pool_key(a_addr, b_addr);

            // check that the pool does not alredy exists
            assert!(
                !self.pools.contains_key(&pool_key),
                "A pool already exists for the input resources"
            );

            // init the pool component
            let pool_component: ComponentAddress = TemplateManager::get(self.pool_template)
                .call("new".to_string(), args![
                    pool_key.0,
                    pool_key.1,
                    self.market_fee
                ]);

            // add the new pool component to the index
            self.pools.insert(pool_key, pool_component);
            
            pool_component
        }

        // create a consistent resource pair by sorting them
        fn build_pool_key(
            a_addr: ResourceAddress,
            b_addr: ResourceAddress
        ) -> PoolKey {
            let mut addr_vector = [a_addr, b_addr];
            addr_vector.sort();
            (addr_vector[0], addr_vector[1])
        }
    }
}
