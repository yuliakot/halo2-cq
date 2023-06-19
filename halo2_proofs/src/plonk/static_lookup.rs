use ff::Field;
use group::prime::PrimeCurveAffine;
use halo2curves::{
    pairing::{Engine, MultiMillerLoop},
    FieldExt,
};
use rand_core::OsRng;

pub(crate) mod prover;
pub(crate) mod verifier;

use std::{collections::BTreeMap, io};

use crate::{
    arithmetic::{best_multiexp, kate_division},
    helpers::SerdePrimeField,
    poly::{kzg::commitment::ParamsKZG, EvaluationDomain},
    SerdeFormat,
};

use super::Expression;

pub fn is_pow_2(x: usize) -> bool {
    (x & (x - 1)) == 0
}

pub fn log2(x: usize) -> u32 {
    (usize::BITS - 1) - x.leading_zeros()
}

#[derive(Debug, Clone)]
pub struct StaticTable<E: MultiMillerLoop> {
    pub opened: Option<StaticTableValues<E>>,
    pub committed: Option<StaticCommittedTable<E>>,
}

/// Abstract type that allows to store MAP(table_id => static_table) in proving(verifying) key
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct StaticTableId<T: Clone + Ord>(pub T);

impl<T: Clone + Ord> StaticTableId<T> {
    pub fn id(&self) -> &T {
        &self.0
    }
}

#[derive(Debug, Clone)]
pub struct StaticTableConfig<E: MultiMillerLoop> {
    size: usize,
    g1_lagrange: Vec<E::G1Affine>,
    g_lagrange_opening_at_0: Vec<E::G1Affine>,
}

impl<E: MultiMillerLoop> StaticTableConfig<E> {
    pub fn new(
        size: usize,
        g1_lagrange: Vec<E::G1Affine>,
        g_lagrange_opening_at_0: Vec<E::G1Affine>,
    ) -> Self {
        Self {
            size,
            g1_lagrange,
            g_lagrange_opening_at_0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct StaticTableValues<E: MultiMillerLoop> {
    size: usize,
    /// Mapping from value to its index in the table
    value_index_mapping: BTreeMap<E::Scalar, usize>,
    // lagrange commitments will exist in params
    // quotient commitments
    qs: Vec<E::G1>,
}

impl<E: MultiMillerLoop> StaticTableValues<E> {
    pub fn new(values: &[E::Scalar], srs_g1: &[E::G1Affine]) -> Self {
        let size = values.len();
        assert!(is_pow_2(size));

        let value_index_mapping: BTreeMap<E::Scalar, usize> =
            values.iter().enumerate().map(|(i, &f)| (f, i)).collect();
        let keys_len: usize = value_index_mapping.keys().len();
        assert_eq!(size, keys_len); // check that table is all unique values

        // compute all qs
        let domain = EvaluationDomain::<E::Scalar>::new(2, log2(size));
        let n = E::Scalar::from(size as u64);
        let n_inv = n.invert().unwrap();

        let w = domain.get_omega();

        let roots_of_unity: Vec<E::Scalar> =
            std::iter::successors(Some(E::Scalar::one()), |p| Some(*p * w))
                .take(size)
                .collect();

        let mut table_coeffs: Vec<E::Scalar> = values.to_vec();
        EvaluationDomain::<E::Scalar>::ifft(
            table_coeffs.as_mut_slice(),
            domain.get_omega_inv(),
            log2(size),
            domain.ifft_divisor(),
        );

        // TODO: THIS SHOULD BE DONE WITH FK METHOD
        let qs: Vec<E::G1> = roots_of_unity
            .iter()
            .map(|&g_i| {
                let quotient = kate_division(&table_coeffs, g_i);
                let quotient = quotient
                    .iter()
                    .map(|&v| v * g_i * n_inv)
                    .collect::<Vec<_>>();

                best_multiexp(&quotient, &srs_g1[..quotient.len()])
            })
            .collect();

        Self {
            size,
            value_index_mapping,
            qs,
        }
    }

    pub fn commit(
        &self,
        srs_g1_len: usize,
        srs_g2: &[E::G2Affine],
        circuit_domain: usize,
    ) -> StaticCommittedTable<E> {
        let domain = EvaluationDomain::<E::Scalar>::new(2, log2(self.size));
        // zv = x^n - 1
        assert!(is_pow_2(self.size));
        let zv = srs_g2[self.size] - srs_g2[0];

        let mut table_coeffs: Vec<E::Scalar> = self.value_index_mapping.keys().cloned().collect();
        EvaluationDomain::<E::Scalar>::ifft(
            table_coeffs.as_mut_slice(),
            domain.get_omega_inv(),
            log2(self.size),
            domain.ifft_divisor(),
        );
        let t = best_multiexp(&table_coeffs, &srs_g2[..table_coeffs.len()]);
        // NOTE: B0 bound is computed generically based on srs size instead of just table size SRS
        // this allows using longer srs or just having multiple tables with different lengths
        let b0_bound_index = srs_g1_len - 1 - (circuit_domain - 2);

        StaticCommittedTable {
            zv: zv.into(),
            t: t.into(),
            x_b0_bound: srs_g2[b0_bound_index],
            size: srs_g1_len,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StaticCommittedTable<E: MultiMillerLoop> {
    pub zv: E::G2Affine,
    pub t: E::G2Affine,
    pub x_b0_bound: E::G2Affine,
    pub size: usize,
}

#[derive(Debug, Clone)]
pub struct Argument<F: Field> {
    input: Expression<F>,
    table_id: StaticTableId<String>,
}

impl<F: Field> Argument<F> {
    pub fn new(name: &'static str, input: Expression<F>, table_id: StaticTableId<String>) -> Self {
        Self { input, table_id }
    }

    pub(crate) fn required_degree(&self) -> usize {
        /*
            B(X)(q(X) * f(X) - \beta) - 1
        */
        std::cmp::max(3, 2 + self.input.degree())
    }
}

// #[test]
// fn test_table() {
//     use halo2curves::bn256::{Bn256, Fr};
//     const N: u32 = 8;
//     let params = ParamsKZG::<Bn256>::setup(N - 1, N, OsRng);

//     let table = StaticTableValues::<Bn256> {
//         size: 8,
//         value_index_mapping: (0..N).map(|i| (Fr::random(OsRng), i as usize)).collect(),
//         qs: vec![],
//     };

//     let _ = table.commit(params.g.len(), &params.g2_srs, 4);
// }
