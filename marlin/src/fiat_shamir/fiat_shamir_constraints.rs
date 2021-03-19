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

use crate::fiat_shamir::{
    traits::{AlgebraicSpongeVar, FiatShamirRngVar},
    AlgebraicSponge,
    FiatShamirAlgebraicSpongeRng,
};

use snarkvm_fields::PrimeField;
use snarkvm_gadgets::{
    fields::{AllocatedFp, FpGadget},
    traits::fields::FieldGadget,
    utilities::{alloc::AllocGadget, boolean::Boolean, integer::Integer, uint::UInt8, ToBitsBEGadget},
};
use snarkvm_nonnative::{
    overhead,
    params::{get_params, OptimizationType},
    AllocatedNonNativeFieldVar,
    NonNativeFieldVar,
};
use snarkvm_r1cs::{ConstraintSystem, ConstraintVariable, LinearCombination, SynthesisError};

use std::marker::PhantomData;

/// Building the Fiat-Shamir sponge's gadget from any algebraic sponge's gadget.
#[derive(Clone)]
pub struct FiatShamirAlgebraicSpongeRngVar<
    F: PrimeField,
    CF: PrimeField,
    PS: AlgebraicSponge<CF>,
    S: AlgebraicSpongeVar<CF, PS>,
> {
    /// Algebraic sponge gadget.
    pub s: S,
    #[doc(hidden)]
    f_phantom: PhantomData<F>,
    cf_phantom: PhantomData<CF>,
    ps_phantom: PhantomData<PS>,
}

impl<F: PrimeField, CF: PrimeField, PS: AlgebraicSponge<CF>, S: AlgebraicSpongeVar<CF, PS>>
    FiatShamirAlgebraicSpongeRngVar<F, CF, PS, S>
{
    /// Compress every two elements if possible. Provides a vector of (limb, num_of_additions),
    /// both of which are CF.
    pub fn compress_gadgets<CS: ConstraintSystem<CF>>(
        mut cs: CS,
        src_limbs: &[(FpGadget<CF>, CF)],
        ty: OptimizationType,
    ) -> Result<Vec<FpGadget<CF>>, SynthesisError> {
        let capacity = CF::size_in_bits() - 1;
        let mut dest_limbs = Vec::<FpGadget<CF>>::new();

        if src_limbs.is_empty() {
            return Ok(vec![]);
        }

        let params = get_params(F::size_in_bits(), CF::size_in_bits(), ty);

        let adjustment_factor_lookup_table = {
            let mut table = Vec::<CF>::new();

            let mut cur = CF::one();
            for _ in 1..=capacity {
                table.push(cur);
                cur.double_in_place();
            }

            table
        };

        let mut i: usize = 0;
        let src_len = src_limbs.len();
        while i < src_len {
            let first = &src_limbs[i];
            let second = if i + 1 < src_len { Some(&src_limbs[i + 1]) } else { None };

            let first_max_bits_per_limb = params.bits_per_limb + overhead!(first.1 + &CF::one());
            let second_max_bits_per_limb = if second.is_some() {
                params.bits_per_limb + overhead!(second.unwrap().1 + &CF::one())
            } else {
                0
            };

            if second.is_some() && first_max_bits_per_limb + second_max_bits_per_limb <= capacity {
                let adjustment_factor = &adjustment_factor_lookup_table[second_max_bits_per_limb];

                let mut value = first.0.mul_by_constant(
                    cs.ns(|| format!("first_mul_by_constant_adjustment_factor_{}", i)),
                    adjustment_factor,
                )?;

                value = value.add(cs.ns(|| format!("value_add_second_{}", i)), &second.unwrap().0)?;

                dest_limbs.push(value);
                i += 2;
            } else {
                dest_limbs.push(first.0.clone());
                i += 1;
            }
        }

        Ok(dest_limbs)
    }

    /// Push gadgets to sponge.
    pub fn push_gadgets_to_sponge<CS: ConstraintSystem<CF>>(
        mut cs: CS,
        sponge: &mut S,
        src: &[NonNativeFieldVar<F, CF>],
        ty: OptimizationType,
    ) -> Result<(), SynthesisError> {
        let mut src_limbs: Vec<(FpGadget<CF>, CF)> = Vec::new();

        for (i, elem) in src.iter().enumerate() {
            match elem {
                NonNativeFieldVar::Constant(c) => {
                    let v = AllocatedNonNativeFieldVar::<F, CF>::alloc_constant(
                        cs.ns(|| format!("alloc_constant_{}", i)),
                        || Ok(c),
                    )?;

                    for limb in v.limbs.iter() {
                        let num_of_additions_over_normal_form = if v.num_of_additions_over_normal_form == CF::zero() {
                            CF::one()
                        } else {
                            v.num_of_additions_over_normal_form
                        };
                        src_limbs.push((limb.clone(), num_of_additions_over_normal_form));
                    }
                }
                NonNativeFieldVar::Var(v) => {
                    for limb in v.limbs.iter() {
                        let num_of_additions_over_normal_form = if v.num_of_additions_over_normal_form == CF::zero() {
                            CF::one()
                        } else {
                            v.num_of_additions_over_normal_form
                        };
                        src_limbs.push((limb.clone(), num_of_additions_over_normal_form));
                    }
                }
            }
        }

        let dest_limbs = Self::compress_gadgets(cs.ns(|| "compress_gadgets"), &src_limbs, ty)?;
        sponge.absorb(cs.ns(|| "absorb"), &dest_limbs)?;
        Ok(())
    }

    /// Obtain random bits from hashchain gadget. (Not guaranteed to be uniformly distributed,
    /// should only be used in certain situations.)
    pub fn get_booleans_from_sponge<CS: ConstraintSystem<CF>>(
        mut cs: CS,
        sponge: &mut S,
        num_bits: usize,
    ) -> Result<Vec<Boolean>, SynthesisError> {
        let bits_per_element = CF::size_in_bits() - 1;
        let num_elements = (num_bits + bits_per_element - 1) / bits_per_element;

        let src_elements = sponge.squeeze(cs.ns(|| "squeeze"), num_elements)?;
        let mut dest_bits = Vec::<Boolean>::new();

        for (i, elem) in src_elements.iter().enumerate() {
            let elem_bits = elem.to_bits_be(cs.ns(|| format!("elem_to_bits_{}", i)))?;
            dest_bits.extend_from_slice(&elem_bits[1..]); // discard the highest bit
        }

        Ok(dest_bits)
    }

    /// Obtain random elements from hashchain gadget. (Not guaranteed to be uniformly distributed,
    /// should only be used in certain situations.)
    pub fn get_gadgets_from_sponge<CS: ConstraintSystem<CF>>(
        cs: CS,
        sponge: &mut S,
        num_elements: usize,
        outputs_short_elements: bool,
    ) -> Result<Vec<NonNativeFieldVar<F, CF>>, SynthesisError> {
        let (dest_gadgets, _) =
            Self::get_gadgets_and_bits_from_sponge(cs, sponge, num_elements, outputs_short_elements)?;

        Ok(dest_gadgets)
    }

    /// Obtain random elements, and the corresponding bits, from hashchain gadget. (Not guaranteed
    /// to be uniformly distributed, should only be used in certain situations.)
    #[allow(clippy::type_complexity)]
    pub fn get_gadgets_and_bits_from_sponge<CS: ConstraintSystem<CF>>(
        mut cs: CS,
        sponge: &mut S,
        num_elements: usize,
        outputs_short_elements: bool,
    ) -> Result<(Vec<NonNativeFieldVar<F, CF>>, Vec<Vec<Boolean>>), SynthesisError> {
        let optimization_type = OptimizationType::Constraints;

        let params = get_params(F::size_in_bits(), CF::size_in_bits(), optimization_type);

        let num_bits_per_nonnative = if outputs_short_elements {
            128
        } else {
            F::size_in_bits() - 1 // also omit the highest bit
        };
        let bits = Self::get_booleans_from_sponge(
            cs.ns(|| "get_booleans_from_sponge"),
            sponge,
            num_bits_per_nonnative * num_elements,
        )?;

        let mut lookup_table = Vec::<Vec<CF>>::new();
        let mut cur = F::one();
        for _ in 0..num_bits_per_nonnative {
            let repr = AllocatedNonNativeFieldVar::<F, CF>::get_limbs_representations(&cur, optimization_type)?;
            lookup_table.push(repr);
            cur.double_in_place();
        }

        let mut dest_gadgets = Vec::<NonNativeFieldVar<F, CF>>::new();
        let mut dest_bits = Vec::<Vec<Boolean>>::new();
        bits.chunks_exact(num_bits_per_nonnative)
            .enumerate()
            .for_each(|(i, per_nonnative_bits)| {
                let mut val = vec![CF::zero(); params.num_limbs];
                let mut lc = vec![LinearCombination::<CF>::zero(); params.num_limbs];

                let mut per_nonnative_bits_le = per_nonnative_bits.to_vec();
                per_nonnative_bits_le.reverse();

                dest_bits.push(per_nonnative_bits_le.clone());

                for (j, bit) in per_nonnative_bits_le.iter().enumerate() {
                    if bit.get_value().unwrap_or_default() {
                        for (k, val) in val.iter_mut().enumerate().take(params.num_limbs) {
                            *val += &lookup_table[j][k];
                        }
                    }

                    #[allow(clippy::needless_range_loop)]
                    for k in 0..params.num_limbs {
                        // TODO (raychu86): Confirm linear combination is correct:
                        // lc[k] = &lc[k] + bit.lc() * lookup_table[j][k];

                        lc[k] = &lc[k] + bit.lc(CS::one(), CF::one()) * lookup_table[j][k];
                    }
                }

                let mut limbs = Vec::new();
                for k in 0..params.num_limbs {
                    let gadget =
                        AllocatedFp::alloc_input(cs.ns(|| format!("alloc_input_{}_{}", i, k)), || Ok(val[k])).unwrap();

                    // TODO (raychu86): Confirm linear combination subtraction is equivalent:
                    // lc[k] = lc[k] - (CF::one(), &gadget.variable);
                    match &gadget.variable {
                        ConstraintVariable::Var(var) => {
                            lc[k] = lc[k].clone() - (CF::one(), *var);
                        }
                        ConstraintVariable::LC(linear_combination) => {
                            lc[k] = &lc[k] - (CF::one(), linear_combination);
                        }
                    }

                    // TODO (raychu86): Confirm CS enforcement is equivalent:
                    // cs.enforce_constraint(lc!(), lc!(), lc[k].clone()).unwrap();
                    cs.enforce(
                        || format!("enforce_constraint_{}_{}", i, k),
                        |lc| lc,
                        |lc| lc,
                        |_| lc[k].clone(),
                    );

                    limbs.push(FpGadget::<CF>::from(gadget));
                }

                dest_gadgets.push(NonNativeFieldVar::<F, CF>::Var(AllocatedNonNativeFieldVar::<F, CF> {
                    limbs,
                    num_of_additions_over_normal_form: CF::zero(),
                    is_in_the_normal_form: true,
                    target_phantom: Default::default(),
                }));
            });

        Ok((dest_gadgets, dest_bits))
    }
}

impl<F: PrimeField, CF: PrimeField, PS: AlgebraicSponge<CF>, S: AlgebraicSpongeVar<CF, PS>>
    FiatShamirRngVar<F, CF, FiatShamirAlgebraicSpongeRng<F, CF, PS>> for FiatShamirAlgebraicSpongeRngVar<F, CF, PS, S>
{
    fn new<CS: ConstraintSystem<CF>>(cs: CS) -> Self {
        Self {
            s: S::new(cs),
            f_phantom: PhantomData,
            cf_phantom: PhantomData,
            ps_phantom: PhantomData,
        }
    }

    fn constant<CS: ConstraintSystem<CF>>(cs: CS, pfs: &FiatShamirAlgebraicSpongeRng<F, CF, PS>) -> Self {
        Self {
            s: S::constant(cs, &pfs.s.clone()),
            f_phantom: PhantomData,
            cf_phantom: PhantomData,
            ps_phantom: PhantomData,
        }
    }

    fn absorb_nonnative_field_elements<CS: ConstraintSystem<CF>>(
        &mut self,
        cs: CS,
        elems: &[NonNativeFieldVar<F, CF>],
        ty: OptimizationType,
    ) -> Result<(), SynthesisError> {
        Self::push_gadgets_to_sponge(cs, &mut self.s, &elems.to_vec(), ty)
    }

    fn absorb_native_field_elements<CS: ConstraintSystem<CF>>(
        &mut self,
        cs: CS,
        elems: &[FpGadget<CF>],
    ) -> Result<(), SynthesisError> {
        self.s.absorb(cs, elems)?;
        Ok(())
    }

    fn absorb_bytes<CS: ConstraintSystem<CF>>(&mut self, mut cs: CS, elems: &[UInt8]) -> Result<(), SynthesisError> {
        let capacity = CF::size_in_bits() - 1;
        let mut bits = Vec::<Boolean>::new();
        for elem in elems.iter() {
            let mut bits_le = elem.to_bits_le(); // UInt8's to_bits is le, which is an exception in Zexe.
            bits_le.reverse();
            bits.extend_from_slice(&bits_le);
        }

        let mut adjustment_factors = Vec::<CF>::new();
        let mut cur = CF::one();
        for _ in 0..capacity {
            adjustment_factors.push(cur);
            cur.double_in_place();
        }

        let mut gadgets = Vec::<FpGadget<CF>>::new();
        for (i, elem_bits) in bits.chunks(capacity).enumerate() {
            let mut elem = CF::zero();
            let mut lc = LinearCombination::zero();
            for (bit, adjustment_factor) in elem_bits.iter().rev().zip(adjustment_factors.iter()) {
                if bit.get_value().unwrap_or_default() {
                    elem += adjustment_factor;
                }
                // TODO (raychu86): Confirm linear combination is correct:
                // lc = &lc + bit.lc() * *adjustment_factor;

                lc = &lc + bit.lc(CS::one(), CF::one()) * *adjustment_factor;
            }

            let gadget = AllocatedFp::alloc_input(cs.ns(|| format!("alloc_input_{}", i)), || Ok(elem))?;

            // TODO (raychu86): Confirm linear combination subtraction is equivalent:
            // lc = lc.clone() - (CF::one(), gadget.variable);
            match &gadget.variable {
                ConstraintVariable::Var(var) => {
                    lc = lc.clone() - (CF::one(), *var);
                }
                ConstraintVariable::LC(linear_combination) => {
                    lc = &lc - (CF::one(), linear_combination);
                }
            }

            gadgets.push(FpGadget::from(gadget));

            // TODO (raychu86): Confirm CS enforcement is equivalent:
            // ccs.enforce_constraint(lc!(), lc!(), lc)?;
            cs.enforce(|| format!("enforce_constraint_{}", i), |lc| lc, |lc| lc, |_| lc);
        }

        self.s.absorb(cs.ns(|| "absorb"), &gadgets)
    }

    fn squeeze_native_field_elements<CS: ConstraintSystem<CF>>(
        &mut self,
        cs: CS,
        num: usize,
    ) -> Result<Vec<FpGadget<CF>>, SynthesisError> {
        self.s.squeeze(cs, num)
    }

    fn squeeze_field_elements<CS: ConstraintSystem<CF>>(
        &mut self,
        cs: CS,
        num: usize,
    ) -> Result<Vec<NonNativeFieldVar<F, CF>>, SynthesisError> {
        Self::get_gadgets_from_sponge(cs, &mut self.s, num, false)
    }

    #[allow(clippy::type_complexity)]
    fn squeeze_field_elements_and_bits<CS: ConstraintSystem<CF>>(
        &mut self,
        cs: CS,
        num: usize,
    ) -> Result<(Vec<NonNativeFieldVar<F, CF>>, Vec<Vec<Boolean>>), SynthesisError> {
        Self::get_gadgets_and_bits_from_sponge(cs, &mut self.s, num, false)
    }

    fn squeeze_128_bits_field_elements<CS: ConstraintSystem<CF>>(
        &mut self,
        cs: CS,
        num: usize,
    ) -> Result<Vec<NonNativeFieldVar<F, CF>>, SynthesisError> {
        Self::get_gadgets_from_sponge(cs, &mut self.s, num, true)
    }

    #[allow(clippy::type_complexity)]
    fn squeeze_128_bits_field_elements_and_bits<CS: ConstraintSystem<CF>>(
        &mut self,
        cs: CS,
        num: usize,
    ) -> Result<(Vec<NonNativeFieldVar<F, CF>>, Vec<Vec<Boolean>>), SynthesisError> {
        Self::get_gadgets_and_bits_from_sponge(cs, &mut self.s, num, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fiat_shamir::{
        poseidon::{constraints::PoseidonSpongeVar, PoseidonSponge},
        traits::FiatShamirRng,
    };

    use snarkvm_curves::bls12_377::Fr;
    use snarkvm_fields::One;
    use snarkvm_gadgets::utilities::eq::EqGadget;
    use snarkvm_r1cs::TestConstraintSystem;
    use snarkvm_utilities::rand::UniformRand;

    use rand::{Rng, SeedableRng};
    use rand_chacha::ChaChaRng;

    type PS = PoseidonSponge<Fr>;
    type PSGadget = PoseidonSpongeVar<Fr>;
    type FS = FiatShamirAlgebraicSpongeRng<Fr, Fr, PS>;
    type FSGadget = FiatShamirAlgebraicSpongeRngVar<Fr, Fr, PS, PSGadget>;

    const MAX_ELEMENTS: usize = 500;
    const MAX_ELEMENT_SIZE: usize = 100;
    const ITERATIONS: usize = 100;

    #[test]
    fn test_fiat_shamir_algebraic_sponge_rng_constant() {
        let mut rng = ChaChaRng::seed_from_u64(123456789u64);

        for i in 0..ITERATIONS {
            let mut cs = TestConstraintSystem::<Fr>::new();

            // Generate random elements to absorb.
            let num_bytes: usize = rng.gen_range(0..MAX_ELEMENT_SIZE);
            let element: Vec<u8> = (0..num_bytes).map(|_| u8::rand(&mut rng)).collect();

            // Create a new FS rng.
            let mut fs_rng = FS::new();
            fs_rng.absorb_bytes(&element);

            // Allocate a new fs_rng gadget from the existing `fs_rng`.
            let mut fs_rng_gadget = FSGadget::constant(cs.ns(|| format!("fs_rng_gadget_constant_{}", i)), &fs_rng);

            // Get bits from the `fs_rng` and `fs_rng_gadget`.
            let num_bits = num_bytes * 8;
            let bits = FS::get_bits_from_sponge(&mut fs_rng.s, num_bits);
            let bit_gadgets = FSGadget::get_booleans_from_sponge(
                cs.ns(|| format!("get_booleans_from_sponge_{}", i)),
                &mut fs_rng_gadget.s,
                num_bits,
            )
            .unwrap();

            // Check that the bit results are equivalent.
            for (j, (bit_gadget, bit)) in bit_gadgets.iter().zip(bits).enumerate() {
                // Allocate a boolean from the native bit.
                let alloc_boolean = Boolean::alloc(cs.ns(|| format!("alloc_boolean_{}_{}", i, j)), || Ok(bit)).unwrap();

                // Check that the boolean gadgets are equivalent.
                bit_gadget
                    .enforce_equal(cs.ns(|| format!("enforce_equal_bit_{}_{}", i, j)), &alloc_boolean)
                    .unwrap();
            }
        }
    }

    #[test]
    fn test_compress_gadgets_weight_optimized() {
        let mut rng = ChaChaRng::seed_from_u64(123456789u64);

        for i in 0..ITERATIONS {
            let mut cs = TestConstraintSystem::<Fr>::new();

            // Generate random elements.
            let num_elements: usize = rng.gen_range(0..MAX_ELEMENT_SIZE);
            let elements: Vec<_> = (0..num_elements).map(|_| Fr::rand(&mut rng)).collect();

            // Construct elements limb representations
            let mut element_limbs = Vec::<(Fr, Fr)>::new();
            let mut element_limb_gadgets = Vec::<(FpGadget<Fr>, Fr)>::new();

            for (j, elem) in elements.iter().enumerate() {
                let limbs =
                    AllocatedNonNativeFieldVar::<Fr, Fr>::get_limbs_representations(elem, OptimizationType::Weight)
                        .unwrap();
                for (k, limb) in limbs.iter().enumerate() {
                    let allocated_limb =
                        FpGadget::alloc(cs.ns(|| format!("alloc_limb_{}_{}_{}", i, j, k)), || Ok(limb)).unwrap();

                    element_limbs.push((*limb, Fr::one()));
                    element_limb_gadgets.push((allocated_limb, Fr::one()));
                    // Specifically set to one, since most gadgets in the constraint world would not have zero noise (due to the relatively weak normal form testing in `alloc`)
                }
            }

            // Compress the elements.
            let compressed_elements = FS::compress_elements(&element_limbs, OptimizationType::Weight);
            let compressed_element_gadgets = FSGadget::compress_gadgets(
                cs.ns(|| "compress_elements"),
                &element_limb_gadgets,
                OptimizationType::Weight,
            )
            .unwrap();

            // Check that the compressed results are equivalent.
            for (j, (gadget, element)) in compressed_element_gadgets.iter().zip(compressed_elements).enumerate() {
                // Allocate the field gadget from the base element.
                let alloc_element =
                    FpGadget::alloc(cs.ns(|| format!("alloc_field_{}_{}", i, j)), || Ok(element)).unwrap();

                // Check that the elements are equivalent.
                gadget
                    .enforce_equal(cs.ns(|| format!("enforce_equal_element_{}_{}", i, j)), &alloc_element)
                    .unwrap();
            }
        }
    }

    #[test]
    fn test_compress_gadgets_constraint_optimized() {
        let mut rng = ChaChaRng::seed_from_u64(123456789u64);

        for i in 0..ITERATIONS {
            let mut cs = TestConstraintSystem::<Fr>::new();

            // Generate random elements.
            let num_elements: usize = rng.gen_range(0..MAX_ELEMENT_SIZE);
            let elements: Vec<_> = (0..num_elements).map(|_| Fr::rand(&mut rng)).collect();

            // Construct elements limb representations
            let mut element_limbs = Vec::<(Fr, Fr)>::new();
            let mut element_limb_gadgets = Vec::<(FpGadget<Fr>, Fr)>::new();

            for (j, elem) in elements.iter().enumerate() {
                let limbs = AllocatedNonNativeFieldVar::<Fr, Fr>::get_limbs_representations(
                    elem,
                    OptimizationType::Constraints,
                )
                .unwrap();
                for (k, limb) in limbs.iter().enumerate() {
                    let allocated_limb =
                        FpGadget::alloc(cs.ns(|| format!("alloc_limb_{}_{}_{}", i, j, k)), || Ok(limb)).unwrap();

                    element_limbs.push((*limb, Fr::one()));
                    element_limb_gadgets.push((allocated_limb, Fr::one()));
                    // Specifically set to one, since most gadgets in the constraint world would not have zero noise (due to the relatively weak normal form testing in `alloc`)
                }
            }

            // Compress the elements.
            let compressed_elements = FS::compress_elements(&element_limbs, OptimizationType::Constraints);
            let compressed_element_gadgets = FSGadget::compress_gadgets(
                cs.ns(|| "compress_elements"),
                &element_limb_gadgets,
                OptimizationType::Constraints,
            )
            .unwrap();

            // Check that the compressed results are equivalent.
            for (j, (gadget, element)) in compressed_element_gadgets.iter().zip(compressed_elements).enumerate() {
                // Allocate the field gadget from the base element.
                let alloc_element =
                    FpGadget::alloc(cs.ns(|| format!("alloc_field_{}_{}", i, j)), || Ok(element)).unwrap();

                // Check that the elements are equivalent.
                gadget
                    .enforce_equal(cs.ns(|| format!("enforce_equal_element_{}_{}", i, j)), &alloc_element)
                    .unwrap();
            }
        }
    }

    #[test]
    fn test_push_gadgets_to_sponge() {}

    #[test]
    fn test_get_gadgets_from_sponge() {}

    #[test]
    fn test_get_booleans_from_sponge() {}

    #[test]
    fn test_squeeze_native_field_elements() {}

    #[test]
    fn test_squeeze_field_elements() {}

    #[test]
    fn test_squeeze_field_elements_and_bits() {}

    #[test]
    fn test_squeeze_128_bits_field_elements() {}

    #[test]
    fn test_squeeze_128_bits_field_elements_and_bits() {}
}