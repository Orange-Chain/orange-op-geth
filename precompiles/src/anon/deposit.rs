use ethabi::ParamType;
use ethereum_types::U256;
use hypr_algebra::{bn254::BN254Scalar, serialization::FromToBytes};
use hypr_api::{
    anon_xfr::ar_to_abar::{verify_ar_to_abar_note, ArToAbarNote},
    parameters::VerifierParams,
    structs::{AnonAssetRecord, AxfrOwnerMemo},
};
use lazy_static::lazy_static;
use rayon::prelude::{IndexedParallelIterator, IntoParallelIterator, ParallelIterator};
use sha3::{Digest, Sha3_512};

use crate::{
    utils::{self, bytes_asset},
    Error, Result,
};

pub struct Deposit {
    outputs: Vec<[u8; 32]>,
    assets: Vec<[u8; 32]>,
    amounts: Vec<U256>,
    proofs: Vec<Vec<u8>>,
    memos: Vec<Vec<u8>>,
    hash: Vec<[u8; 32]>,
}

lazy_static! {
    static ref PARAMS: VerifierParams = VerifierParams::get_ar_to_abar().unwrap();
}

impl Deposit {
    // abi "bytes32[]", "bytes32[]", "uint256[]", "bytes[]", "bytes[]", "bytes32[]"
    fn params_type() -> [ParamType; 6] {
        let outputs = ParamType::Array(Box::new(ParamType::FixedBytes(32)));
        let assets = ParamType::Array(Box::new(ParamType::FixedBytes(32)));
        let amount = ParamType::Array(Box::new(ParamType::Uint(256)));
        let proof = ParamType::Array(Box::new(ParamType::Bytes));
        let memos = ParamType::Array(Box::new(ParamType::Bytes));
        let hash = ParamType::Array(Box::new(ParamType::FixedBytes(32)));
        [outputs, assets, amount, proof, memos, hash]
    }

    fn require(&self) -> Result<()> {
        let len = self.outputs.len();

        if len == self.assets.len()
            && len == self.amounts.len()
            && len == self.proofs.len()
            && len == self.memos.len()
            && len == self.hash.len()
        {
            Ok(())
        } else {
            Err(Error::WrongLengthOfArguments)
        }
    }

    pub fn new(data: &[u8]) -> Result<Self> {
        let res = ethabi::decode(&Self::params_type(), data).map_err(|_| Error::ParseDataFailed)?;

        let outputs = utils::into_bytes32_array(res.get(0).cloned())?;
        let assets = utils::into_bytes32_array(res.get(1).cloned())?;
        let amounts = utils::into_uint256_array(res.get(2).cloned())?;
        let proofs = utils::into_bytes_array(res.get(3).cloned())?;
        let memos = utils::into_bytes_array(res.get(4).cloned())?;
        let hash = utils::into_bytes32_array(res.get(5).cloned())?;

        let r = Self {
            outputs,
            assets,
            amounts,
            proofs,
            memos,
            hash,
        };

        r.require()?;

        Ok(r)
    }

    pub fn check(self) -> Result<()> {
        let res: Vec<_> = self
            .outputs
            .into_par_iter()
            .zip(self.assets)
            .zip(self.amounts)
            .zip(self.proofs)
            .zip(self.hash)
            .map(|((((output, asset), amount), proof), hash)| {
                verify_ttoa(&PARAMS, asset, amount.as_u128(), &output, &proof, hash)
            })
            .collect();

        for r in res {
            r?
        }
        Ok(())
    }

    pub fn gas(self) -> u64 {
        DEPOSIT_VERIFY_PER_GAS * self.assets.len() as u64
    }
}

pub const DEPOSIT_VERIFY_PER_GAS: u64 = 50000;

fn verify_ttoa(
    params: &VerifierParams,
    asset: [u8; 32],
    amount: u128,
    commitment: &[u8; 32],
    proof: &[u8],
    hash: [u8; 32],
) -> Result<()> {
    let output = AnonAssetRecord {
        commitment: BN254Scalar::from_bytes(commitment).map_err(|_| Error::ParseDataFailed)?,
    };
    let proof = bincode::deserialize(proof).map_err(|_| Error::ProofDecodeFailed)?;

    let note = ArToAbarNote {
        asset: bytes_asset(&asset)?,
        amount,
        output,
        proof,
        memo: AxfrOwnerMemo::from_bytes(&Vec::new()),
    };

    let mut hasher = Sha3_512::new();
    hasher.update(hash);

    verify_ar_to_abar_note(params, &note, hasher).map_err(|_| Error::ProofVerificationFailed)
}

#[cfg(test)]
mod test {
    use super::Deposit;

    #[test]
    fn test_len_1() {
        let encode = hex::decode("00000000000000000000000000000000000000000000000000000000000000c00000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000014000000000000000000000000000000000000000000000000000000000000001800000000000000000000000000000000000000000000000000000000000000680000000000000000000000000000000000000000000000000000000000000078000000000000000000000000000000000000000000000000000000000000000012e5cee2ca3c56caf722797738332415647acb7cdc28db468c20f40f422c53927000000000000000000000000000000000000000000000000000000000000000100000000000000000000000064d09e26eca6c9bf3779dbe856dad76d5184034000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000004563918244f4000000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000048805000000000000002000000000000000a03a599a6f184a2c0a9075d63e06e98512e2e8b53674d583e642c2617c42dc132000000000000000e078c3b10f981b23c4a2fb5a74206172283c50c62c5aff96a76a4694380c45a82000000000000000f52401a6de1710bf054d29f8910595723240dee4e3e10882ce879ca1dec4419c20000000000000009080ce2b1abf88d99ff7eaca9bb2d15620dbe7a8c1a3d84a590c9f2c182c5fa9200000000000000076efba58d0edbe4cd68fca5ee5a07cc8327b0f63b8f091f707f5a025fab4a801050000000000000020000000000000006eeec7bd487b29d8ce94b3e0096fede1fd4718da799aa9f6e2c149cfe4a8f3902000000000000000b173e20007d72895bc6dcbe07016d88a9b4591e847d391c9fd160587ad42f09c200000000000000070ae9cff76c25c04cdf07eb9ac371528fb3b90a38a2ad05e3bb8237722787b2f2000000000000000edd6ea1018d01206191bfb7b2f55fbcaa9d837e0f299ff7897de3bc7789f480b20000000000000005f838d1dbab41abbf0324a0cdfd37d0f8fe0622a411a2721f318f034c3ab5b90200000000000000039f883c25110a2732e266a68824042655caf1683ec2b9255248648656c763b89200000000000000077520f8c9456044f1acd4cc63a7a6c29bdfeb0c2cac9d8aff1731ebf2ac131142000000000000000a6d928f19176c91f1601a27ddf1b0e9af0ab4e2a2e99095ec4ce184480c48c2405000000000000002000000000000000bb3a951ec1c019daef06b214c50e48c413cc85663c8594c716e9de368474832e200000000000000057214f686c42693ff0ad0c968ebbad92bd8f251595c61407d6731b9837999d1920000000000000007ce13bb74b2b006cdd226c3dc0cc60d4e0c83a9a81625afb9161a3b8bcb0c62c2000000000000000d4560ec12b74895e602e98f0d7e1d698e45f98170b260c12d2bc9f0c2bb8ff222000000000000000b278d666ed0447c461281a4795a77575e85588941d789a1c9c4b388c4eb47e0c03000000000000002000000000000000d5970f026d16519144d91a6ef32ef875ef729b45192313ed4999721668b3f02420000000000000008d08e45184a548d6a362533fc49e16931e741e7bcb9f55b6633408245b3ab62d2000000000000000b8ae8df67d8e81157e9377191041ff043ab22511d22e7298c367c5fa50b963042000000000000000cf616e557755aff09cc0824429bbea1d3b7c0ad2d2b679291d462a40ba14590204000000000000002000000000000000ad555915b8f6851df3c13197c084e8da4dd9273e5add80feb262dfc71b24db012000000000000000c801b08dd5486aeb7cbb4315930e9f601efb70885cad7d32246097c70314d01b2000000000000000228b2b2065b92721cf1845a3935cd966433d044c3b7887f3703f208681ac462220000000000000005ea149614ac39f39759fa868c05dc604f1cde2dc77ee7dcb8b1b44a883a28d05200000000000000013734d031356384e77085f0b5ce08383cbc974902d53b180f00fa6c5b5063e8e20000000000000007ff7b70ea9a69d9bd3e982f25037cc1bb8a5ed2572cc389bc0bbac4abb1fb626000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000081e6e9d2acb8271083b9d7ed2f741515e30df0dc6145f1e9ae574b7c429ecdb166809d105b0db9140f5a2bc500324f7cc44e08c292a4d08e725a9091345e24511b1a4c8a7178fb43f029fa8b6062a4441fdf06195bea4581050a1bd8838d9b0439763bf69fa8e08e4afa1728dffc225580fff806ede6ae018d3b1c8a02431a907202000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001ad210a311d4e33e4df3536e1463bde91a7c9867b76d1e9b69ae3e93297016cbc").unwrap();

        let res = Deposit::new(&encode);
        match res {
            Err(v) => {
                println!("err {}", v.code());
            }
            Ok(dep) => {
                assert_eq!(50000, dep.gas());
            }
        }
    }


    #[test]
    fn test_len_1_1() {
        let encode = hex::decode("00000000000000000000000000000000000000000000000000000000000000c00000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000014000000000000000000000000000000000000000000000000000000000000001800000000000000000000000000000000000000000000000000000000000000680000000000000000000000000000000000000000000000000000000000000078000000000000000000000000000000000000000000000000000000000000000012e5cee2ca3c56caf722797738332415647acb7cdc28db468c20f40f422c53927000000000000000000000000000000000000000000000000000000000000000100000000000000000000000064d09e26eca6c9bf3779dbe856dad76d5184034000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000004563918244f4000000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000048805000000000000002000000000000000a03a599a6f184a2c0a9075d63e06e98512e2e8b53674d583e642c2617c42dc132000000000000000e078c3b10f981b23c4a2fb5a74206172283c50c62c5aff96a76a4694380c45a82000000000000000f52401a6de1710bf054d29f8910595723240dee4e3e10882ce879ca1dec4419c20000000000000009080ce2b1abf88d99ff7eaca9bb2d15620dbe7a8c1a3d84a590c9f2c182c5fa9200000000000000076efba58d0edbe4cd68fca5ee5a07cc8327b0f63b8f091f707f5a025fab4a801050000000000000020000000000000006eeec7bd487b29d8ce94b3e0096fede1fd4718da799aa9f6e2c149cfe4a8f3902000000000000000b173e20007d72895bc6dcbe07016d88a9b4591e847d391c9fd160587ad42f09c200000000000000070ae9cff76c25c04cdf07eb9ac371528fb3b90a38a2ad05e3bb8237722787b2f2000000000000000edd6ea1018d01206191bfb7b2f55fbcaa9d837e0f299ff7897de3bc7789f480b20000000000000005f838d1dbab41abbf0324a0cdfd37d0f8fe0622a411a2721f318f034c3ab5b90200000000000000039f883c25110a2732e266a68824042655caf1683ec2b9255248648656c763b89200000000000000077520f8c9456044f1acd4cc63a7a6c29bdfeb0c2cac9d8aff1731ebf2ac131142000000000000000a6d928f19176c91f1601a27ddf1b0e9af0ab4e2a2e99095ec4ce184480c48c2405000000000000002000000000000000bb3a951ec1c019daef06b214c50e48c413cc85663c8594c716e9de368474832e200000000000000057214f686c42693ff0ad0c968ebbad92bd8f251595c61407d6731b9837999d1920000000000000007ce13bb74b2b006cdd226c3dc0cc60d4e0c83a9a81625afb9161a3b8bcb0c62c2000000000000000d4560ec12b74895e602e98f0d7e1d698e45f98170b260c12d2bc9f0c2bb8ff222000000000000000b278d666ed0447c461281a4795a77575e85588941d789a1c9c4b388c4eb47e0c03000000000000002000000000000000d5970f026d16519144d91a6ef32ef875ef729b45192313ed4999721668b3f02420000000000000008d08e45184a548d6a362533fc49e16931e741e7bcb9f55b6633408245b3ab62d2000000000000000b8ae8df67d8e81157e9377191041ff043ab22511d22e7298c367c5fa50b963042000000000000000cf616e557755aff09cc0824429bbea1d3b7c0ad2d2b679291d462a40ba14590204000000000000002000000000000000ad555915b8f6851df3c13197c084e8da4dd9273e5add80feb262dfc71b24db012000000000000000c801b08dd5486aeb7cbb4315930e9f601efb70885cad7d32246097c70314d01b2000000000000000228b2b2065b92721cf1845a3935cd966433d044c3b7887f3703f208681ac462220000000000000005ea149614ac39f39759fa868c05dc604f1cde2dc77ee7dcb8b1b44a883a28d05200000000000000013734d031356384e77085f0b5ce08383cbc974902d53b180f00fa6c5b5063e8e20000000000000007ff7b70ea9a69d9bd3e982f25037cc1bb8a5ed2572cc389bc0bbac4abb1fb626000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000081e6e9d2acb8271083b9d7ed2f741515e30df0dc6145f1e9ae574b7c429ecdb166809d105b0db9140f5a2bc500324f7cc44e08c292a4d08e725a9091345e24511b1a4c8a7178fb43f029fa8b6062a4441fdf06195bea4581050a1bd8838d9b0439763bf69fa8e08e4afa1728dffc225580fff806ede6ae018d3b1c8a02431a907202000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001ad210a311d4e33e4df3536e1463bde91a7c9867b76d1e9b69ae3e93297016cbc").unwrap();

        let res = Deposit::new(&encode);
        match res {
            Err(v) => {
                println!("{}", v.code());
            }
            Ok(dep) => {
                let result = dep.check();
                match  result {
                    Ok(_) => {
                        println!("check ok");
                    },
                    Err(_) => {
                        println!("check err");
                    },
                }
            }
        }
    }

    #[test]
    fn test_len_2() {
        let encode = hex::decode("00000000000000000000000000000000000000000000000000000000000000c00000000000000000000000000000000000000000000000000000000000000120000000000000000000000000000000000000000000000000000000000000018000000000000000000000000000000000000000000000000000000000000001e00000000000000000000000000000000000000000000000000000000000000bc00000000000000000000000000000000000000000000000000000000000000da00000000000000000000000000000000000000000000000000000000000000002a17fdb9199735f012ba3431b62491d2a7dbadd103bd3d934009aa277484da025a17fdb9199735f012ba3431b62491d2a7dbadd103bd3d934009aa277484da025000000000000000000000000000000000000000000000000000000000000000200000000000000000000000064d09e26eca6c9bf3779dbe856dad76d5184034000000000000000000000000064d09e26eca6c9bf3779dbe856dad76d5184034000000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000004563918244f400000000000000000000000000000000000000000000000000004563918244f40000000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000500000000000000000000000000000000000000000000000000000000000000048805000000000000002000000000000000e45eb814b5026e1f8e6aaa76ac03fc28f9238132f150eb5b28574c43a4180eaf200000000000000033e6a308bac8b6c283fd4eb54e6fd618ef8dca2acd478106a48db8b39203de8620000000000000009e673f9500b3a7723a1ac4e56bbbdf09157cc36a96fc910a8c148f3e62ec6d132000000000000000d2919998cb09d10598b8cb9163b5cabf862d533b540a53adfa415b20f623aa10200000000000000058c0d0f1ab88efdc6d23511f8133088e0b7d2803c87f365a6f831b41eff41c2a05000000000000002000000000000000fd1138c68b0cbf6d27adc55e3c7ea5244e279e57d3a6083e73ef24b7ad08dbaf2000000000000000506bb7c46f518ce392aa9696351dd01fa4b7cc8035de1bad2b28096616bb35ab2000000000000000618ef1a3fcb175da8a835911021d20bb444cc1176afe1ebe4a4d7f02a8aee00d20000000000000006bfe2193a8265901e88fc30910fdc752365435542d7329c504b1a259174055ab20000000000000005f7ed0272991a0f81c37ad6b48c2d6a35c1757b8a8de966a593bde8706a6fbac2000000000000000b6199695cf9fc5cfa1d3fb060b280fdef270c03bc7c4e5b2ef1764f891185f832000000000000000431f15d746b6dbc023c5dc6ea97f972e9dae6854e8261385c3fa35425f697800200000000000000048086fa5e61fc81f283d0a142718f1476f225d00372f113ca6bf4984f333d10505000000000000002000000000000000316cbd778e5c5c564da8dc140fd187da18414e6588838e169b2f6e08195f560e2000000000000000e43047535479abd22783ef14b5467d2eaa025f33bba35d6fedf0e88c9c21e81b2000000000000000e8854e686fefd338201997d2b6c6ab686f7501d06909fc9a4769c3fb6594532f2000000000000000ec7050ff04366de3bb50e76cd4e181b297f07f972594167e48d2008db062ef292000000000000000f90ab29290ac2fada53b599cc7cb56ef1c8e320079381cdfb758ccc635e26c0d030000000000000020000000000000009feb96f6c0178252aa9d8374f749ea8fb133deca13a466f64d8a5374c9c40d002000000000000000e660a1643d195ff8ea55513b393216f34272d502c6257619317a025fec79ac0e20000000000000000fbbda1e14e9152c7ce977ac4d9b4fc70c634dc9bdb7f625c7afa466e8f11e1a200000000000000088647bc564dc5f1453c1327eaf244286b0de4e5773d5f5d939c640ece3932503040000000000000020000000000000006e6ff0af9d33354cfec6401cfa1589a3d862f006700353cc3e85dc93a0697d012000000000000000bb27654a79e6a30bb77b57e74b9c45b7cafb28a546edc019d52ce63e5aa89f2a20000000000000008151bddf52b3380c281fe30c381f516b9884d42cca576055806d9d9ad00b54092000000000000000158371cd229a8d5d6497a7cf7de688faedf319d0c8125fa69e230f0b1edbf52d2000000000000000feb09b092dfcbd89b8ba1a364d39e47d4e8b331cb7bcc03c612d890fd16a2e0f200000000000000044ccdb089dc205f4b7a0f0c8337cef0241f7f10957198d1948d9387e5de11087000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000048805000000000000002000000000000000e45eb814b5026e1f8e6aaa76ac03fc28f9238132f150eb5b28574c43a4180eaf200000000000000033e6a308bac8b6c283fd4eb54e6fd618ef8dca2acd478106a48db8b39203de8620000000000000009e673f9500b3a7723a1ac4e56bbbdf09157cc36a96fc910a8c148f3e62ec6d132000000000000000d2919998cb09d10598b8cb9163b5cabf862d533b540a53adfa415b20f623aa10200000000000000058c0d0f1ab88efdc6d23511f8133088e0b7d2803c87f365a6f831b41eff41c2a05000000000000002000000000000000fd1138c68b0cbf6d27adc55e3c7ea5244e279e57d3a6083e73ef24b7ad08dbaf2000000000000000506bb7c46f518ce392aa9696351dd01fa4b7cc8035de1bad2b28096616bb35ab2000000000000000618ef1a3fcb175da8a835911021d20bb444cc1176afe1ebe4a4d7f02a8aee00d20000000000000006bfe2193a8265901e88fc30910fdc752365435542d7329c504b1a259174055ab20000000000000005f7ed0272991a0f81c37ad6b48c2d6a35c1757b8a8de966a593bde8706a6fbac2000000000000000b6199695cf9fc5cfa1d3fb060b280fdef270c03bc7c4e5b2ef1764f891185f832000000000000000431f15d746b6dbc023c5dc6ea97f972e9dae6854e8261385c3fa35425f697800200000000000000048086fa5e61fc81f283d0a142718f1476f225d00372f113ca6bf4984f333d10505000000000000002000000000000000316cbd778e5c5c564da8dc140fd187da18414e6588838e169b2f6e08195f560e2000000000000000e43047535479abd22783ef14b5467d2eaa025f33bba35d6fedf0e88c9c21e81b2000000000000000e8854e686fefd338201997d2b6c6ab686f7501d06909fc9a4769c3fb6594532f2000000000000000ec7050ff04366de3bb50e76cd4e181b297f07f972594167e48d2008db062ef292000000000000000f90ab29290ac2fada53b599cc7cb56ef1c8e320079381cdfb758ccc635e26c0d030000000000000020000000000000009feb96f6c0178252aa9d8374f749ea8fb133deca13a466f64d8a5374c9c40d002000000000000000e660a1643d195ff8ea55513b393216f34272d502c6257619317a025fec79ac0e20000000000000000fbbda1e14e9152c7ce977ac4d9b4fc70c634dc9bdb7f625c7afa466e8f11e1a200000000000000088647bc564dc5f1453c1327eaf244286b0de4e5773d5f5d939c640ece3932503040000000000000020000000000000006e6ff0af9d33354cfec6401cfa1589a3d862f006700353cc3e85dc93a0697d012000000000000000bb27654a79e6a30bb77b57e74b9c45b7cafb28a546edc019d52ce63e5aa89f2a20000000000000008151bddf52b3380c281fe30c381f516b9884d42cca576055806d9d9ad00b54092000000000000000158371cd229a8d5d6497a7cf7de688faedf319d0c8125fa69e230f0b1edbf52d2000000000000000feb09b092dfcbd89b8ba1a364d39e47d4e8b331cb7bcc03c612d890fd16a2e0f200000000000000044ccdb089dc205f4b7a0f0c8337cef0241f7f10957198d1948d9387e5de110870000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000081bafd7757534363ccbfd0aeba4ca27ed039c86a78b19b733ad3e632ba675ccb06808e46d6eda1c75751ea9acd3999a84784cf90f13d809203e1f9df28a3cea6ff6dd99f3317ebe0103b6805442512e913157e8df8dd0aa5e9241c677e4e6c5edec1946bbf2046aeead80bfcb4c31d5309ca470a4d423600bbeeec7906b5e96ee8c2000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000081bafd7757534363ccbfd0aeba4ca27ed039c86a78b19b733ad3e632ba675ccb06808e46d6eda1c75751ea9acd3999a84784cf90f13d809203e1f9df28a3cea6ff6dd99f3317ebe0103b6805442512e913157e8df8dd0aa5e9241c677e4e6c5edec1946bbf2046aeead80bfcb4c31d5309ca470a4d423600bbeeec7906b5e96ee8c2000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002ad210a311d4e33e4df3536e1463bde91a7c9867b76d1e9b69ae3e93297016cbcad210a311d4e33e4df3536e1463bde91a7c9867b76d1e9b69ae3e93297016cbc").unwrap();

        let res = Deposit::new(&encode);
        match res {
            Err(v) => {
                println!("err {}", v.code());
            }
            Ok(dep) => {
                assert_eq!(100000, dep.gas());
            }
        }
    }
}
