// Copyright (C) 2019-2021 Aleo Systems Inc.
// This file is part of the snarkVM library.

// The snarkVM library is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// The snarkVM library is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with the snarkVM library. If not, see <https://www.gnu.org/licenses/>.

use crate::{
    account::AccountAddress,
    errors::RecordError,
    testnet1::{payload::Payload, BaseDPCComponents},
    traits::RecordScheme,
};
use snarkvm_algorithms::traits::{CommitmentScheme, SignatureScheme, CRH};
use snarkvm_utilities::{
    bytes::{FromBytes, ToBytes},
    to_bytes,
    variable_length_integer::*,
};

use std::{
    fmt,
    io::{Read, Result as IoResult, Write},
    str::FromStr,
};

fn default_program_id<C: CRH>() -> Vec<u8> {
    to_bytes![C::Output::default()].unwrap()
}

#[derive(Derivative)]
#[derivative(
    Default(bound = "C: BaseDPCComponents"),
    Debug(bound = "C: BaseDPCComponents"),
    Clone(bound = "C: BaseDPCComponents"),
    PartialEq(bound = "C: BaseDPCComponents"),
    Eq(bound = "C: BaseDPCComponents")
)]
pub struct Record<C: BaseDPCComponents> {
    pub(crate) owner: AccountAddress<C>,
    pub(crate) is_dummy: bool,
    // TODO (raychu86) use AleoAmount which will guard the value range
    pub(crate) value: u64,
    pub(crate) payload: Payload,

    #[derivative(Default(value = "default_program_id::<C::ProgramVerificationKeyCRH>()"))]
    pub(crate) birth_program_id: Vec<u8>,
    #[derivative(Default(value = "default_program_id::<C::ProgramVerificationKeyCRH>()"))]
    pub(crate) death_program_id: Vec<u8>,

    pub(crate) serial_number_nonce: <C::SerialNumberNonceCRH as CRH>::Output,
    pub(crate) commitment: <C::RecordCommitment as CommitmentScheme>::Output,
    pub(crate) commitment_randomness: <C::RecordCommitment as CommitmentScheme>::Randomness,
}

impl<C: BaseDPCComponents> RecordScheme for Record<C> {
    type Commitment = <C::RecordCommitment as CommitmentScheme>::Output;
    type CommitmentRandomness = <C::RecordCommitment as CommitmentScheme>::Randomness;
    type Owner = AccountAddress<C>;
    type Payload = Payload;
    type SerialNumber = <C::AccountSignature as SignatureScheme>::PublicKey;
    type SerialNumberNonce = <C::SerialNumberNonceCRH as CRH>::Output;
    type Value = u64;

    fn owner(&self) -> &Self::Owner {
        &self.owner
    }

    fn is_dummy(&self) -> bool {
        self.is_dummy
    }

    fn value(&self) -> Self::Value {
        self.value
    }

    fn payload(&self) -> &Self::Payload {
        &self.payload
    }

    fn birth_program_id(&self) -> &[u8] {
        &self.birth_program_id
    }

    fn death_program_id(&self) -> &[u8] {
        &self.death_program_id
    }

    fn serial_number_nonce(&self) -> &Self::SerialNumberNonce {
        &self.serial_number_nonce
    }

    fn commitment(&self) -> Self::Commitment {
        self.commitment.clone()
    }

    fn commitment_randomness(&self) -> Self::CommitmentRandomness {
        self.commitment_randomness.clone()
    }
}

impl<C: BaseDPCComponents> ToBytes for Record<C> {
    #[inline]
    fn write<W: Write>(&self, mut writer: W) -> IoResult<()> {
        self.owner.write(&mut writer)?;
        self.is_dummy.write(&mut writer)?;
        self.value.write(&mut writer)?;
        self.payload.write(&mut writer)?;

        variable_length_integer(self.birth_program_id.len() as u64).write(&mut writer)?;
        self.birth_program_id.write(&mut writer)?;

        variable_length_integer(self.death_program_id.len() as u64).write(&mut writer)?;
        self.death_program_id.write(&mut writer)?;

        self.serial_number_nonce.write(&mut writer)?;
        self.commitment.write(&mut writer)?;
        self.commitment_randomness.write(&mut writer)
    }
}

impl<C: BaseDPCComponents> FromBytes for Record<C> {
    #[inline]
    fn read<R: Read>(mut reader: R) -> IoResult<Self> {
        let owner: AccountAddress<C> = FromBytes::read(&mut reader)?;
        let is_dummy: bool = FromBytes::read(&mut reader)?;
        let value: u64 = FromBytes::read(&mut reader)?;
        let payload: Payload = FromBytes::read(&mut reader)?;

        let birth_program_id_size: usize = read_variable_length_integer(&mut reader)?;

        let mut birth_program_id = Vec::with_capacity(birth_program_id_size);
        for _ in 0..birth_program_id_size {
            let byte: u8 = FromBytes::read(&mut reader)?;
            birth_program_id.push(byte);
        }

        let death_program_id_size: usize = read_variable_length_integer(&mut reader)?;

        let mut death_program_id = Vec::with_capacity(death_program_id_size);
        for _ in 0..death_program_id_size {
            let byte: u8 = FromBytes::read(&mut reader)?;
            death_program_id.push(byte);
        }

        let serial_number_nonce: <C::SerialNumberNonceCRH as CRH>::Output = FromBytes::read(&mut reader)?;

        let commitment: <C::RecordCommitment as CommitmentScheme>::Output = FromBytes::read(&mut reader)?;
        let commitment_randomness: <C::RecordCommitment as CommitmentScheme>::Randomness =
            FromBytes::read(&mut reader)?;

        Ok(Self {
            owner,
            is_dummy,
            value,
            payload,
            birth_program_id,
            death_program_id,
            serial_number_nonce,
            commitment,
            commitment_randomness,
        })
    }
}

impl<C: BaseDPCComponents> FromStr for Record<C> {
    type Err = RecordError;

    fn from_str(record: &str) -> Result<Self, Self::Err> {
        let record = hex::decode(record)?;

        Ok(Self::read(&record[..])?)
    }
}

impl<C: BaseDPCComponents> fmt::Display for Record<C> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            hex::encode(to_bytes![self].expect("serialization to bytes failed"))
        )
    }
}
