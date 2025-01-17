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
    impl_sw_curve_serializer,
    impl_sw_from_random_bytes,
    traits::{AffineCurve, Group, ProjectiveCurve, SWModelParameters as Parameters},
};
use snarkvm_fields::{impl_additive_ops_from_ref, Field, One, PrimeField, SquareRootField, Zero};
use snarkvm_utilities::{
    bititerator::BitIteratorBE,
    bytes::{FromBytes, ToBytes},
    rand::UniformRand,
    serialize::*,
};

use rand::{
    distributions::{Distribution, Standard},
    Rng,
};
use serde::{Deserialize, Serialize};
use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    io::{Error, ErrorKind, Read, Result as IoResult, Write},
    marker::PhantomData,
    ops::{Add, AddAssign, Mul, MulAssign, Neg, Sub, SubAssign},
};

#[derive(Derivative, Serialize, Deserialize)]
#[derivative(
    Copy(bound = "P: Parameters"),
    Clone(bound = "P: Parameters"),
    PartialEq(bound = "P: Parameters"),
    Eq(bound = "P: Parameters"),
    Debug(bound = "P: Parameters"),
    Hash(bound = "P: Parameters")
)]
pub struct GroupAffine<P: Parameters> {
    pub x: P::BaseField,
    pub y: P::BaseField,
    pub infinity: bool,
    #[derivative(Debug = "ignore")]
    _params: PhantomData<P>,
}

impl<P: Parameters> Display for GroupAffine<P> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        if self.infinity {
            write!(f, "GroupAffine(Infinity)")
        } else {
            write!(f, "GroupAffine(x={}, y={})", self.x, self.y)
        }
    }
}

impl<P: Parameters> GroupAffine<P> {
    pub fn new(x: P::BaseField, y: P::BaseField, infinity: bool) -> Self {
        Self {
            x,
            y,
            infinity,
            _params: PhantomData,
        }
    }

    pub fn scale_by_cofactor(&self) -> GroupProjective<P> {
        self.mul_bits(BitIteratorBE::new(P::COFACTOR))
    }

    pub fn is_on_curve(&self) -> bool {
        if self.is_zero() {
            true
        } else {
            // Check that the point is on the curve
            let y2 = self.y.square();
            let x3b = P::add_b(&((self.x.square() * self.x) + P::mul_by_a(&self.x)));
            y2 == x3b
        }
    }
}

impl<P: Parameters> Zero for GroupAffine<P> {
    #[inline]
    fn zero() -> Self {
        Self::new(P::BaseField::zero(), P::BaseField::one(), true)
    }

    #[inline]
    fn is_zero(&self) -> bool {
        self.infinity
    }
}

impl<P: Parameters> AffineCurve for GroupAffine<P> {
    type BaseField = P::BaseField;
    type Projective = GroupProjective<P>;

    impl_sw_from_random_bytes!();

    #[inline]
    fn prime_subgroup_generator() -> Self {
        Self::new(P::AFFINE_GENERATOR_COEFFS.0, P::AFFINE_GENERATOR_COEFFS.1, false)
    }

    /// Attempts to construct an affine point given an x-coordinate. The
    /// point is not guaranteed to be in the prime order subgroup.
    ///
    /// If and only if `greatest` is set will the lexicographically
    /// largest y-coordinate be selected.
    fn from_x_coordinate(x: Self::BaseField, greatest: bool) -> Option<Self> {
        // Compute x^3 + ax + b
        let x3b = P::add_b(&((x.square() * x) + P::mul_by_a(&x)));

        x3b.sqrt().map(|y| {
            let negy = -y;

            let y = if (y < negy) ^ greatest { y } else { negy };
            Self::new(x, y, false)
        })
    }

    /// Attempts to construct an affine point given a y-coordinate. The
    /// point is not guaranteed to be in the prime order subgroup.
    ///
    /// If and only if `greatest` is set will the lexicographically
    /// largest y-coordinate be selected.
    fn from_y_coordinate(_y: Self::BaseField, _greatest: bool) -> Option<Self> {
        unimplemented!()
    }

    fn mul_bits<S: AsRef<[u64]>>(&self, bits: BitIteratorBE<S>) -> GroupProjective<P> {
        let mut res = GroupProjective::zero();
        for i in bits {
            res.double_in_place();
            if i {
                res.add_assign_mixed(self)
            }
        }
        res
    }

    fn mul_by_cofactor_to_projective(&self) -> Self::Projective {
        self.scale_by_cofactor()
    }

    fn mul_by_cofactor_inv(&self) -> Self {
        self.mul(P::COFACTOR_INV).into()
    }

    #[inline]
    fn into_projective(&self) -> GroupProjective<P> {
        (*self).into()
    }

    fn is_in_correct_subgroup_assuming_on_curve(&self) -> bool {
        self.mul_bits(BitIteratorBE::new(P::ScalarField::characteristic()))
            .is_zero()
    }

    fn to_x_coordinate(&self) -> Self::BaseField {
        self.x
    }

    fn to_y_coordinate(&self) -> Self::BaseField {
        self.y
    }

    /// Checks that the current point is on the elliptic curve.
    fn is_on_curve(&self) -> bool {
        if self.is_zero() {
            true
        } else {
            // Check that the point is on the curve
            let y2 = self.y.square();
            let x3b = P::add_b(&((self.x.square() * self.x) + P::mul_by_a(&self.x)));
            y2 == x3b
        }
    }
}

impl<P: Parameters> Group for GroupAffine<P> {
    type ScalarField = P::ScalarField;

    #[inline]
    #[must_use]
    fn double(&self) -> Self {
        let mut tmp = *self;
        tmp += self;
        tmp
    }

    #[inline]
    fn double_in_place(&mut self) {
        let tmp = *self;
        *self = tmp.double();
    }
}

impl<P: Parameters> Neg for GroupAffine<P> {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self {
        if !self.is_zero() {
            Self::new(self.x, -self.y, false)
        } else {
            self
        }
    }
}

impl_additive_ops_from_ref!(GroupAffine, Parameters);

impl<'a, P: Parameters> Add<&'a Self> for GroupAffine<P> {
    type Output = Self;

    fn add(self, other: &'a Self) -> Self {
        let mut copy = self;
        copy += other;
        copy
    }
}

impl<'a, P: Parameters> AddAssign<&'a Self> for GroupAffine<P> {
    fn add_assign(&mut self, other: &'a Self) {
        let mut projective = GroupProjective::from(*self);
        projective.add_assign_mixed(other);
        *self = projective.into();
    }
}

impl<'a, P: Parameters> Sub<&'a Self> for GroupAffine<P> {
    type Output = Self;

    fn sub(self, other: &'a Self) -> Self {
        let mut copy = self;
        copy -= other;
        copy
    }
}

impl<'a, P: Parameters> SubAssign<&'a Self> for GroupAffine<P> {
    fn sub_assign(&mut self, other: &'a Self) {
        *self += &(-(*other));
    }
}

impl<P: Parameters> Mul<P::ScalarField> for GroupAffine<P> {
    type Output = Self;

    fn mul(self, other: P::ScalarField) -> Self {
        self.mul_bits(BitIteratorBE::new(other.into_repr())).into()
    }
}

impl<P: Parameters> MulAssign<P::ScalarField> for GroupAffine<P> {
    fn mul_assign(&mut self, other: P::ScalarField) {
        *self = self.mul(other).into()
    }
}

impl<P: Parameters> ToBytes for GroupAffine<P> {
    #[inline]
    fn write<W: Write>(&self, mut writer: W) -> IoResult<()> {
        self.x.write(&mut writer)?;
        self.y.write(&mut writer)?;
        self.infinity.write(writer)
    }
}

impl<P: Parameters> FromBytes for GroupAffine<P> {
    #[inline]
    fn read<R: Read>(mut reader: R) -> IoResult<Self> {
        let x = P::BaseField::read(&mut reader)?;
        let y = P::BaseField::read(&mut reader)?;
        let infinity = bool::read(&mut reader)?;

        if infinity != x.is_zero() && y.is_one() {
            return Err(Error::new(ErrorKind::InvalidData, "Infinity flag is not valid"));
        }
        Ok(Self::new(x, y, infinity))
    }
}

impl<P: Parameters> Default for GroupAffine<P> {
    #[inline]
    fn default() -> Self {
        Self::zero()
    }
}

impl<P: Parameters> Distribution<GroupAffine<P>> for Standard {
    #[inline]
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> GroupAffine<P> {
        loop {
            let x = P::BaseField::rand(rng);
            let greatest = rng.gen();

            if let Some(p) = GroupAffine::from_x_coordinate(x, greatest) {
                return p.scale_by_cofactor().into();
            }
        }
    }
}

#[derive(Derivative)]
#[derivative(
    Copy(bound = "P: Parameters"),
    Clone(bound = "P: Parameters"),
    Eq(bound = "P: Parameters"),
    Debug(bound = "P: Parameters"),
    Hash(bound = "P: Parameters")
)]
pub struct GroupProjective<P: Parameters> {
    pub x: P::BaseField,
    pub y: P::BaseField,
    pub z: P::BaseField,
    _params: PhantomData<P>,
}

impl<P: Parameters> Display for GroupProjective<P> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}", self.into_affine())
    }
}

impl<P: Parameters> PartialEq for GroupProjective<P> {
    fn eq(&self, other: &Self) -> bool {
        if self.is_zero() {
            return other.is_zero();
        }

        if other.is_zero() {
            return false;
        }

        // The points (X, Y, Z) and (X', Y', Z')
        // are equal when (X * Z^2) = (X' * Z'^2)
        // and (Y * Z^3) = (Y' * Z'^3).
        let z1 = self.z.square();
        let z2 = other.z.square();

        !(self.x * z2 != other.x * z1 || self.y * (z2 * other.z) != other.y * (z1 * self.z))
    }
}

impl<P: Parameters> Distribution<GroupProjective<P>> for Standard {
    #[inline]
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> GroupProjective<P> {
        let res = GroupProjective::prime_subgroup_generator() * P::ScalarField::rand(rng);
        debug_assert!(res.into_affine().is_in_correct_subgroup_assuming_on_curve());
        res
    }
}

impl<P: Parameters> ToBytes for GroupProjective<P> {
    #[inline]
    fn write<W: Write>(&self, mut writer: W) -> IoResult<()> {
        self.x.write(&mut writer)?;
        self.y.write(&mut writer)?;
        self.z.write(writer)
    }
}

impl<P: Parameters> FromBytes for GroupProjective<P> {
    #[inline]
    fn read<R: Read>(mut reader: R) -> IoResult<Self> {
        let x = P::BaseField::read(&mut reader)?;
        let y = P::BaseField::read(&mut reader)?;
        let z = P::BaseField::read(reader)?;
        Ok(Self::new(x, y, z))
    }
}

impl<P: Parameters> Default for GroupProjective<P> {
    #[inline]
    fn default() -> Self {
        Self::zero()
    }
}

impl<P: Parameters> GroupProjective<P> {
    pub fn new(x: P::BaseField, y: P::BaseField, z: P::BaseField) -> Self {
        Self {
            x,
            y,
            z,
            _params: PhantomData,
        }
    }
}

impl<P: Parameters> Zero for GroupProjective<P> {
    // The point at infinity is always represented by
    // Z = 0.
    #[inline]
    fn zero() -> Self {
        Self::new(P::BaseField::zero(), P::BaseField::one(), P::BaseField::zero())
    }

    // The point at infinity is always represented by
    // Z = 0.
    #[inline]
    fn is_zero(&self) -> bool {
        self.z.is_zero()
    }
}

impl<P: Parameters> ProjectiveCurve for GroupProjective<P> {
    type Affine = GroupAffine<P>;
    type BaseField = P::BaseField;

    #[inline]
    fn prime_subgroup_generator() -> Self {
        GroupAffine::prime_subgroup_generator().into()
    }

    #[inline]
    fn is_normalized(&self) -> bool {
        self.is_zero() || self.z.is_one()
    }

    #[inline]
    fn batch_normalization(v: &mut [Self]) {
        // Montgomery’s Trick and Fast Implementation of Masked AES
        // Genelle, Prouff and Quisquater
        // Section 3.2

        // First pass: compute [a, ab, abc, ...]
        let mut prod = Vec::with_capacity(v.len());
        let mut tmp = P::BaseField::one();
        for g in v
            .iter_mut()
            // Ignore normalized elements
            .filter(|g| !g.is_normalized())
        {
            tmp.mul_assign(&g.z);
            prod.push(tmp);
        }

        // Invert `tmp`.
        tmp = tmp.inverse().unwrap(); // Guaranteed to be nonzero.

        // Second pass: iterate backwards to compute inverses
        for (g, s) in v
            .iter_mut()
            // Backwards
            .rev()
            // Ignore normalized elements
            .filter(|g| !g.is_normalized())
            // Backwards, skip last element, fill in one for last term.
            .zip(
                prod.into_iter()
                    .rev()
                    .skip(1)
                    .chain(Some(P::BaseField::one())),
            )
        {
            // tmp := tmp * g.z; g.z := tmp * s = 1/z
            let newtmp = tmp * g.z;
            g.z = tmp * s;
            tmp = newtmp;
        }
        #[cfg(not(feature = "parallel"))]
        {
            // Perform affine transformations
            for g in v.iter_mut().filter(|g| !g.is_normalized()) {
                let z2 = g.z.square(); // 1/z
                g.x *= &z2; // x/z^2
                g.y *= &(z2 * g.z); // y/z^3
                g.z = P::BaseField::one(); // z = 1
            }
        }

        #[cfg(feature = "parallel")]
        {
            use rayon::prelude::*;
            // Perform affine transformations
            v.par_iter_mut().filter(|g| !g.is_normalized()).for_each(|g| {
                let z2 = g.z.square(); // 1/z
                g.x *= &z2; // x/z^2
                g.y *= &(z2 * g.z); // y/z^3
                g.z = P::BaseField::one(); // z = 1
            });
        }
    }

    #[allow(clippy::many_single_char_names)]
    fn add_assign_mixed(&mut self, other: &Self::Affine) {
        if other.is_zero() {
            return;
        }

        if self.is_zero() {
            self.x = other.x;
            self.y = other.y;
            self.z = P::BaseField::one();
            return;
        }

        // http://www.hyperelliptic.org/EFD/g1p/auto-shortw-jacobian-0.html#addition-madd-2007-bl
        // Works for all curves.

        // Z1Z1 = Z1^2
        let z1z1 = self.z.square();

        // U2 = X2*Z1Z1
        let u2 = other.x * z1z1;

        // S2 = Y2*Z1*Z1Z1
        let s2 = (other.y * self.z) * z1z1;

        if self.x == u2 && self.y == s2 {
            // The two points are equal, so we double.
            self.double_in_place();
        } else {
            // If we're adding -a and a together, self.z becomes zero as H becomes zero.

            // H = U2-X1
            let h = u2 - self.x;

            // HH = H^2
            let hh = h.square();

            // I = 4*HH
            let mut i = hh;
            i.double_in_place();
            i.double_in_place();

            // J = H*I
            let mut j = h * i;

            // r = 2*(S2-Y1)
            let r = (s2 - self.y).double();

            // V = X1*I
            let v = self.x * i;

            // X3 = r^2 - J - 2*V
            self.x = r.square();
            self.x -= &j;
            self.x -= &v;
            self.x -= &v;

            // Y3 = r*(V-X3)-2*Y1*J
            j *= &self.y; // J = 2*Y1*J
            j.double_in_place();
            self.y = v - self.x;
            self.y *= &r;
            self.y -= &j;

            // Z3 = (Z1+H)^2-Z1Z1-HH
            self.z += &h;
            self.z.square_in_place();
            self.z -= &z1z1;
            self.z -= &hh;
        }
    }

    #[inline]
    fn into_affine(&self) -> GroupAffine<P> {
        (*self).into()
    }

    #[inline]
    fn recommended_wnaf_for_scalar(scalar: <Self::ScalarField as PrimeField>::BigInteger) -> usize {
        P::empirical_recommended_wnaf_for_scalar(scalar)
    }

    #[inline]
    fn recommended_wnaf_for_num_scalars(num_scalars: usize) -> usize {
        P::empirical_recommended_wnaf_for_num_scalars(num_scalars)
    }
}

impl<P: Parameters> Group for GroupProjective<P> {
    type ScalarField = P::ScalarField;

    #[inline]
    #[must_use]
    fn double(&self) -> Self {
        let mut tmp = *self;
        tmp.double_in_place();
        tmp
    }

    #[inline]
    fn double_in_place(&mut self) {
        if self.is_zero() {
            return;
        }

        if P::COEFF_A.is_zero() {
            // A = X1^2
            let mut a = self.x.square();

            // B = Y1^2
            let b = self.y.square();

            // C = B^2
            let mut c = b.square();

            // D = 2*((X1+B)2-A-C)
            let d = ((self.x + b).square() - a - c).double();

            // E = 3*A
            let old_a = a;
            a.double_in_place();
            let e = old_a + a;

            // F = E^2
            let f = e.square();

            // Z3 = 2*Y1*Z1
            self.z *= &self.y;
            self.z.double_in_place();

            // X3 = F-2*D
            self.x = f - d - d;

            // Y3 = E*(D-X3)-8*C
            c.double_in_place();
            c.double_in_place();
            c.double_in_place();
            self.y = (d - self.x) * e - c;
        } else {
            // http://www.hyperelliptic.org/EFD/g1p/auto-shortw-jacobian-0.html#doubling-dbl-2009-l
            // XX = X1^2
            let xx = self.x.square();

            // YY = Y1^2
            let yy = self.y.square();

            // YYYY = YY^2
            let mut yyyy = yy.square();

            // ZZ = Z1^2
            let zz = self.z.square();

            // S = 2*((X1+YY)^2-XX-YYYY)
            let s = ((self.x + yy).square() - xx - yyyy).double();

            // M = 3*XX+a*ZZ^2
            let m = xx + xx + xx + P::mul_by_a(&zz.square());

            // T = M^2-2*S
            let t = m.square() - s.double();

            // X3 = T
            self.x = t;
            // Y3 = M*(S-T)-8*YYYY
            let old_y = self.y;
            yyyy.double_in_place();
            yyyy.double_in_place();
            yyyy.double_in_place();
            self.y = m * (s - t) - yyyy;
            // Z3 = (Y1+Z1)^2-YY-ZZ
            self.z = (old_y + self.z).square() - yy - zz;
        }
    }
}

impl<P: Parameters> Neg for GroupProjective<P> {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self {
        if !self.is_zero() {
            Self::new(self.x, -self.y, self.z)
        } else {
            self
        }
    }
}

impl_additive_ops_from_ref!(GroupProjective, Parameters);

impl<'a, P: Parameters> Add<&'a Self> for GroupProjective<P> {
    type Output = Self;

    #[inline]
    fn add(self, other: &'a Self) -> Self {
        let mut copy = self;
        copy += other;
        copy
    }
}

impl<'a, P: Parameters> AddAssign<&'a Self> for GroupProjective<P> {
    #[allow(clippy::many_single_char_names)]
    #[allow(clippy::suspicious_op_assign_impl)]
    fn add_assign(&mut self, other: &'a Self) {
        if self.is_zero() {
            *self = *other;
            return;
        }

        if other.is_zero() {
            return;
        }

        // http://www.hyperelliptic.org/EFD/g1p/auto-shortw-jacobian-0.html#addition-add-2007-bl
        // Works for all curves.

        // Z1Z1 = Z1^2
        let z1z1 = self.z.square();

        // Z2Z2 = Z2^2
        let z2z2 = other.z.square();

        // U1 = X1*Z2Z2
        let u1 = self.x * z2z2;

        // U2 = X2*Z1Z1
        let u2 = other.x * z1z1;

        // S1 = Y1*Z2*Z2Z2
        let s1 = self.y * other.z * z2z2;

        // S2 = Y2*Z1*Z1Z1
        let s2 = other.y * self.z * z1z1;

        if u1 == u2 && s1 == s2 {
            // The two points are equal, so we double.
            self.double_in_place();
        } else {
            // If we're adding -a and a together, self.z becomes zero as H becomes zero.

            // H = U2-U1
            let h = u2 - u1;

            // I = (2*H)^2
            let i = (h.double()).square();

            // J = H*I
            let j = h * i;

            // r = 2*(S2-S1)
            let r = (s2 - s1).double();

            // V = U1*I
            let v = u1 * i;

            // X3 = r^2 - J - 2*V
            self.x = r.square() - j - (v.double());

            // Y3 = r*(V - X3) - 2*S1*J
            self.y = r * (v - self.x) - (s1 * j).double();

            // Z3 = ((Z1+Z2)^2 - Z1Z1 - Z2Z2)*H
            self.z = ((self.z + other.z).square() - z1z1 - z2z2) * h;
        }
    }
}

impl<'a, P: Parameters> Sub<&'a Self> for GroupProjective<P> {
    type Output = Self;

    #[inline]
    fn sub(self, other: &'a Self) -> Self {
        let mut copy = self;
        copy -= other;
        copy
    }
}

impl<'a, P: Parameters> SubAssign<&'a Self> for GroupProjective<P> {
    fn sub_assign(&mut self, other: &'a Self) {
        *self += &(-(*other));
    }
}

impl<P: Parameters> Mul<P::ScalarField> for GroupProjective<P> {
    type Output = Self;

    /// Performs scalar multiplication of this element.
    #[allow(clippy::suspicious_arithmetic_impl)]
    #[inline]
    fn mul(self, other: P::ScalarField) -> Self {
        let mut res = Self::zero();

        let mut found_one = false;

        for i in BitIteratorBE::new(other.into_repr()) {
            if found_one {
                res.double_in_place();
            } else {
                found_one = i;
            }

            if i {
                res += self;
            }
        }

        res
    }
}

impl<P: Parameters> MulAssign<P::ScalarField> for GroupProjective<P> {
    /// Performs scalar multiplication of this element.
    fn mul_assign(&mut self, other: P::ScalarField) {
        *self = *self * other
    }
}

// The affine point X, Y is represented in the Jacobian
// coordinates with Z = 1.
impl<P: Parameters> From<GroupAffine<P>> for GroupProjective<P> {
    #[inline]
    fn from(p: GroupAffine<P>) -> GroupProjective<P> {
        if p.is_zero() {
            Self::zero()
        } else {
            Self::new(p.x, p.y, P::BaseField::one())
        }
    }
}

// The projective point X, Y, Z is represented in the affine
// coordinates as X/Z^2, Y/Z^3.
impl<P: Parameters> From<GroupProjective<P>> for GroupAffine<P> {
    #[inline]
    fn from(p: GroupProjective<P>) -> GroupAffine<P> {
        if p.is_zero() {
            GroupAffine::zero()
        } else if p.z.is_one() {
            // If Z is one, the point is already normalized.
            GroupAffine::new(p.x, p.y, false)
        } else {
            // Z is nonzero, so it must have an inverse in a field.
            let zinv = p.z.inverse().unwrap();
            let zinv_squared = zinv.square();

            // X/Z^2
            let x = p.x * zinv_squared;

            // Y/Z^3
            let y = p.y * (zinv_squared * zinv);

            GroupAffine::new(x, y, false)
        }
    }
}

impl_sw_curve_serializer!(Parameters);
