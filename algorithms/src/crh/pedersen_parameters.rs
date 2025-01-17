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

use crate::traits::crh::CRHParameters;
use snarkvm_curves::Group;
use snarkvm_fields::{ConstraintFieldError, Field, ToConstraintField};
use snarkvm_utilities::bytes::{FromBytes, ToBytes};

use rand::Rng;
use std::{
    fmt::Debug,
    io::{Read, Result as IoResult, Write},
    marker::PhantomData,
};

pub trait PedersenSize: Clone + Debug + Eq {
    const NUM_WINDOWS: usize;
    const WINDOW_SIZE: usize;
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PedersenCRHParameters<G: Group, S: PedersenSize> {
    pub bases: Vec<Vec<G>>,
    _size: PhantomData<S>,
}

impl<G: Group, S: PedersenSize> CRHParameters for PedersenCRHParameters<G, S> {
    fn setup<R: Rng>(rng: &mut R) -> Self {
        let bases = (0..S::NUM_WINDOWS).map(|_| Self::base(S::WINDOW_SIZE, rng)).collect();
        Self {
            bases,
            _size: PhantomData,
        }
    }
}

impl<G: Group, S: PedersenSize> PedersenCRHParameters<G, S> {
    pub fn from(bases: Vec<Vec<G>>) -> Self {
        Self {
            bases,
            _size: PhantomData,
        }
    }

    fn base<R: Rng>(num_powers: usize, rng: &mut R) -> Vec<G> {
        let mut powers = Vec::with_capacity(num_powers);
        let mut base = G::rand(rng);
        for _ in 0..num_powers {
            powers.push(base);
            base.double_in_place();
        }
        powers
    }
}

impl<G: Group, S: PedersenSize> ToBytes for PedersenCRHParameters<G, S> {
    fn write<W: Write>(&self, mut writer: W) -> IoResult<()> {
        (self.bases.len() as u32).write(&mut writer)?;
        for base in &self.bases {
            (base.len() as u32).write(&mut writer)?;
            for g in base {
                g.write(&mut writer)?;
            }
        }

        Ok(())
    }
}

impl<G: Group, S: PedersenSize> FromBytes for PedersenCRHParameters<G, S> {
    #[inline]
    fn read<R: Read>(mut reader: R) -> IoResult<Self> {
        let num_bases: u32 = FromBytes::read(&mut reader)?;
        let mut bases = Vec::with_capacity(num_bases as usize);

        for _ in 0..num_bases {
            let base_len: u32 = FromBytes::read(&mut reader)?;
            let mut base = Vec::with_capacity(base_len as usize);

            for _ in 0..base_len {
                let g: G = FromBytes::read(&mut reader)?;
                base.push(g);
            }
            bases.push(base);
        }

        Ok(Self {
            bases,
            _size: PhantomData,
        })
    }
}

impl<F: Field, G: Group + ToConstraintField<F>, S: PedersenSize> ToConstraintField<F> for PedersenCRHParameters<G, S> {
    #[inline]
    fn to_field_elements(&self) -> Result<Vec<F>, ConstraintFieldError> {
        Ok(Vec::new())
    }
}
