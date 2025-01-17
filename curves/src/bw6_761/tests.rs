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
    bw6_761::{
        g1::BW6_761G1Parameters,
        g2::Bls12_377G2Parameters,
        Fq,
        Fq3,
        Fq6,
        Fr,
        G1Affine,
        G1Projective,
        G2Affine,
        G2Projective,
        BW6_761,
    },
    templates::short_weierstrass::tests::sw_tests,
    traits::{tests_curve::curve_tests, tests_group::group_test, AffineCurve, PairingEngine},
};
use snarkvm_fields::{
    tests_field::{field_serialization_test, field_test, frobenius_test, primefield_test, sqrt_field_test},
    Field,
    One,
    PrimeField,
};

#[test]
fn test_bw6_761_fr() {
    let a: Fr = rand::random();
    let b: Fr = rand::random();
    field_test(a, b);
    sqrt_field_test(a);
    primefield_test::<Fr>();
}

#[test]
fn test_bw6_761_fq() {
    let a: Fq = rand::random();
    let b: Fq = rand::random();
    field_test(a, b);
    primefield_test::<Fq>();
    sqrt_field_test(a);
    field_serialization_test::<Fq>();
}

#[test]
fn test_bw6_761_fq3() {
    let a: Fq3 = rand::random();
    let b: Fq3 = rand::random();
    field_test(a, b);
    sqrt_field_test(a);
    frobenius_test::<Fq3, _>(Fq::characteristic(), 13);
}

#[test]
fn test_bw6_761_fq6() {
    let a: Fq6 = rand::random();
    let b: Fq6 = rand::random();
    field_test(a, b);
    frobenius_test::<Fq6, _>(Fq::characteristic(), 13);
}

#[test]
fn test_g1_projective_curve() {
    curve_tests::<G1Projective>();

    sw_tests::<BW6_761G1Parameters>();
}

#[test]
fn test_g1_projective_group() {
    let a: G1Projective = rand::random();
    let b: G1Projective = rand::random();
    group_test(a, b);
}

#[test]
fn test_g1_generator() {
    let generator = G1Affine::prime_subgroup_generator();
    assert!(generator.is_on_curve());
    assert!(generator.is_in_correct_subgroup_assuming_on_curve());
}

#[test]
fn test_g2_projective_curve() {
    curve_tests::<G2Projective>();

    sw_tests::<Bls12_377G2Parameters>();
}

#[test]
fn test_g2_projective_group() {
    let a: G2Projective = rand::random();
    let b: G2Projective = rand::random();
    group_test(a, b);
}

#[test]
fn test_g2_generator() {
    let generator = G2Affine::prime_subgroup_generator();
    assert!(generator.is_on_curve());
    assert!(generator.is_in_correct_subgroup_assuming_on_curve());
}

#[test]
fn test_bilinearity() {
    let a: G1Projective = rand::random();
    let b: G2Projective = rand::random();
    let s: Fr = rand::random();

    let sa = a * s;
    let sb = b * s;

    let ans1 = BW6_761::pairing(sa, b);
    let ans2 = BW6_761::pairing(a, sb);
    let ans3 = BW6_761::pairing(a, b).pow(s.into_repr());

    assert_eq!(ans1, ans2);
    assert_eq!(ans2, ans3);

    assert_ne!(ans1, Fq6::one());
    assert_ne!(ans2, Fq6::one());
    assert_ne!(ans3, Fq6::one());

    assert_eq!(ans1.pow(Fr::characteristic()), Fq6::one());
    assert_eq!(ans2.pow(Fr::characteristic()), Fq6::one());
    assert_eq!(ans3.pow(Fr::characteristic()), Fq6::one());
}
