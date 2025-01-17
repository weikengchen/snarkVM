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

pub use crate::crh::pedersen_parameters::PedersenSize;

use crate::{
    crh::{PedersenCRH, PedersenCRHParameters},
    traits::CRH,
};
use snarkvm_curves::traits::Group;
use snarkvm_fields::{ConstraintFieldError, Field, ToConstraintField};
use snarkvm_utilities::bytes::{FromBytes, ToBytes};

use rand::Rng;
use std::io::{Read, Result as IoResult, Write};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PedersenCommitmentParameters<G: Group, S: PedersenSize> {
    pub bases: Vec<Vec<G>>,
    pub random_base: Vec<G>,
    pub crh: PedersenCRH<G, S>,
}

impl<G: Group, S: PedersenSize> PedersenCommitmentParameters<G, S> {
    pub fn setup<R: Rng>(rng: &mut R) -> Self {
        let bases = (0..S::NUM_WINDOWS)
            .map(|_| Self::base(S::WINDOW_SIZE, rng))
            .collect::<Vec<Vec<G>>>();
        let random_base = Self::base(S::WINDOW_SIZE, rng);
        let crh_parameters = PedersenCRHParameters::from(bases.clone());
        let crh = PedersenCRH::from(crh_parameters);
        Self {
            bases,
            random_base,
            crh,
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

impl<F: Field, G: Group + ToConstraintField<F>, S: PedersenSize> ToConstraintField<F>
    for PedersenCommitmentParameters<G, S>
{
    #[inline]
    fn to_field_elements(&self) -> Result<Vec<F>, ConstraintFieldError> {
        Ok(Vec::new())
    }
}

impl<G: Group, S: PedersenSize> ToBytes for PedersenCommitmentParameters<G, S> {
    fn write<W: Write>(&self, mut writer: W) -> IoResult<()> {
        (self.bases.len() as u32).write(&mut writer)?;
        for base in &self.bases {
            (base.len() as u32).write(&mut writer)?;
            for g in base {
                g.write(&mut writer)?;
            }
        }

        (self.random_base.len() as u32).write(&mut writer)?;
        for g in &self.random_base {
            g.write(&mut writer)?;
        }

        self.crh.parameters().write(&mut writer)?;

        Ok(())
    }
}

impl<G: Group, S: PedersenSize> FromBytes for PedersenCommitmentParameters<G, S> {
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

        let random_base_len: u32 = FromBytes::read(&mut reader)?;
        let mut random_base = Vec::with_capacity(random_base_len as usize);

        for _ in 0..random_base_len {
            let g: G = FromBytes::read(&mut reader)?;
            random_base.push(g);
        }

        let crh_parameters: <PedersenCRH<G, S> as CRH>::Parameters = FromBytes::read(&mut reader)?;
        let crh = PedersenCRH::<G, S>::from(crh_parameters);

        Ok(Self {
            bases,
            random_base,
            crh,
        })
    }
}
