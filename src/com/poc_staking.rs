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

use sp_core::crypto::{AccountId32, Ss58Codec};

use sub_runtime::poc_staking::{MachineInfo};

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
pub struct PocStakingRuntime;

#[module]
pub trait PocStaking: System {
//     type Moment: Parameter + Default + AtLeast32Bit
//     + Scale<Self::BlockNumber, Output=Self::Moment> + Copy;
}

#[derive(Clone, Encode, Decode, Debug, Call)]
pub struct RegisterCall<T: PocStaking> {
    pub plot_size: u64,
    pub numeric_id: u128,
    pub miner_proportion: u32,
    pub miner_reward_dest: Option<AccountId32>,
    pub _runtime: PhantomData<T>,

}

#[derive(Clone, Debug, Eq, PartialEq, Store, Encode)]
pub struct DiskOfStore<T: PocStaking> {
    #[store(returns = Option<MachineInfo<T::BlockNumber, T::AccountId>>)]
    pub account_id: T::AccountId,
    pub _runtime: PhantomData<T>,
}

impl Runtime for PocStakingRuntime {
    type Signature = MultiSignature;
    type Extra = DefaultExtra<Self>;
}

impl System for PocStakingRuntime {
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

impl Balances for PocStakingRuntime {
    type Balance = u128;
}