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

use crate::curves::{Field, PrimeField};
use crate::gadgets::r1cs::ConstraintSystem;
use crate::gadgets::utilities::bits::FullAdder;
use crate::gadgets::utilities::boolean::Boolean;
use crate::gadgets::utilities::int::*;
use snarkvm_r1cs::errors::SynthesisError;

/// Returns the bitwise sum of a n-bit number with carry bit
pub trait RippleCarryAdder<F: Field, Rhs = Self>
where
    Self: std::marker::Sized,
{
    fn add_bits<CS: ConstraintSystem<F>>(&self, cs: CS, other: &Self) -> Result<Vec<Boolean>, SynthesisError>;
}

// Generic impl
impl<F: Field> RippleCarryAdder<F> for Vec<Boolean> {
    fn add_bits<CS: ConstraintSystem<F>>(&self, mut cs: CS, other: &Self) -> Result<Vec<Boolean>, SynthesisError> {
        let mut result = Vec::with_capacity(self.len() + 1);
        let mut carry = Boolean::constant(false);
        for (i, (a, b)) in self.iter().zip(other.iter()).enumerate() {
            let (sum, next) = Boolean::add(cs.ns(|| format!("rpc {}", i)), a, b, &carry)?;

            carry = next;
            result.push(sum);
        }

        // append the carry bit to the end
        result.push(carry);

        Ok(result)
    }
}

macro_rules! rpc_impl {
    ($($gadget: ident)*) => ($(
        impl<F: Field + PrimeField> RippleCarryAdder<F> for $gadget {
            fn add_bits<CS: ConstraintSystem<F>>(&self, cs: CS, other: &Self) -> Result<Vec<Boolean>, SynthesisError> {
                self.bits.add_bits(cs, &other.bits)
            }
        }
    )*)
}

rpc_impl!(Int8 Int16 Int32 Int64 Int128);