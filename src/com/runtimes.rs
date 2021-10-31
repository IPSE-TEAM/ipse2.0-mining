use sp_runtime::{
    generic::Header,
    traits::{
        BlakeTwo256,
        IdentifyAccount,
        Verify,
    },
    MultiSignature,
    OpaqueExtrinsic,
};

use codec::{
    Decode,
    Encode,
};

use std::{
    collections::BTreeMap,
    fmt::Debug,
    marker::PhantomData,
};

// use sub_runtime::PoC;

use frame_support::Parameter;
use sub_runtime::poc::{Difficulty, MiningInfo};

use substrate_subxt::{
    balances::{
        AccountData,
        Balances,
    },
    extrinsic::{DefaultExtra},
    Runtime,
    contracts::Contracts,
    system::System,
};
use sp_runtime::traits::{AtLeast32Bit, Scale};

// use node_primitives::{AccountIndex, AccountId};

// pub trait Timestamp: System {
//     type Moment: Parameter + Default + AtLeast32Bit
//     + Scale<Self::BlockNumber, Output=Self::Moment> + Copy;
// }



#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PocRuntime;

use substrate_subxt::balances::BalancesEventsDecoder;
use substrate_subxt::system::SystemEventsDecoder;

use crate::com::timestamp::{Timestamp, TimestampEventsDecoder};
use crate::com::poc_staking::{PocStaking, PocStakingEventsDecoder};

#[module]
pub trait PoC: System + Balances + Timestamp + PocStaking{}

impl PoC for PocRuntime {

}

// #[derive(Clone, Debug, Eq, PartialEq, Event, Decode)]
// pub struct MiningEvent {
//     /// Account balance was transfered from.
//     pub account_id: u64,
//     pub height: u64,
//     pub sig: u64,
//     pub nonce: u64,
//     pub deadline: u64,
//
// }


#[derive(Clone, Debug, Eq, PartialEq, Store, Encode)]
pub struct TargetInfoStore<T: PoC> {
    #[store(returns = Option<Vec<Difficulty>>)]
    pub _runtime: PhantomData<T>,
}

#[derive(Clone, Debug, Eq, PartialEq, Store, Encode)]
pub struct LastMiningTsStore<T: PoC> {
    #[store(returns = Option<u64>)]
    pub _runtime: PhantomData<T>,
}

// #[derive(Clone, Debug, Eq, PartialEq, Store, Encode)]
// pub struct NumberStore<T: System> {
//     #[store(returns = <T as System>::BlockNumber)]
//     pub _runtime: PhantomData<T>,
// }

#[derive(Clone, Debug, Eq, PartialEq, Event, Decode)]
pub struct MiningEvent<T: PoC> {
    /// Account balance was transfered from.
    pub miner: <T as System>::AccountId,

    pub is_ok: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Store, Encode)]
pub struct DlInfoStore<T: PoC> {
    #[store(returns = Option<Vec<MiningInfo<T::AccountId>>>)]
    pub _runtime: PhantomData<T>,
}

#[derive(Clone, Encode, Decode, Debug, Call)]
pub struct MiningCall<T: PoC> {
    pub account_id: u64,
    pub height: u64,
    pub sig: [u8; 32],
    pub nonce: u64,
    pub deadline: u64,
    pub _runtime: PhantomData<T>,

}



impl System for PocRuntime {
    type Index = u32;
    type BlockNumber = u32;
    type Hash = sp_core::H256;
    type Hashing = BlakeTwo256;
    type AccountId = <<MultiSignature as Verify>::Signer as IdentifyAccount>::AccountId;
    type Address = pallet_indices::address::Address<Self::AccountId, u32>;
//     type Address = sp_runtime::MultiAddress<AccountId, AccountIndex>;

    type Header = Header<Self::BlockNumber, BlakeTwo256>;

    type Extrinsic = OpaqueExtrinsic;
    type AccountData = AccountData<<Self as Balances>::Balance>;
}

impl Timestamp for PocRuntime {
//     type Moment = u128;
}

impl PocStaking for PocRuntime {

}

impl Balances for PocRuntime {
    type Balance = u128;
}

impl Runtime for PocRuntime {
    type Signature = MultiSignature;
    type Extra = DefaultExtra<Self>;
}



