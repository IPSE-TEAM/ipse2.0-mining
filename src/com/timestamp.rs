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
use substrate_subxt::system::SystemEventsDecoder;
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TimeStampRuntime;

#[module]
pub trait Timestamp: System {
//     type Moment: Parameter + Default + AtLeast32Bit
//     + Scale<Self::BlockNumber, Output=Self::Moment> + Copy;
}

#[derive(Clone, Debug, Eq, PartialEq, Store, Encode)]
pub struct NowStore<T: Timestamp> {
    #[store(returns = Option<u64>)]
    pub _runtime: PhantomData<T>,
}

impl Runtime for TimeStampRuntime {
    type Signature = MultiSignature;
    type Extra = DefaultExtra<Self>;
}

impl System for TimeStampRuntime {
    type Index = u32;
    type BlockNumber = u32;
    type Hash = sp_core::H256;
    type Hashing = BlakeTwo256;
    type AccountId = <<MultiSignature as Verify>::Signer as IdentifyAccount>::AccountId;
    type Address = pallet_indices::address::Address<Self::AccountId, u32>;
    type Header = Header<Self::BlockNumber, BlakeTwo256>;
    type Extrinsic = OpaqueExtrinsic;
    type AccountData = AccountData<<Self as Balances>::Balance>;
}

impl Balances for TimeStampRuntime {
    type Balance = u128;
}