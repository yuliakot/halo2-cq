use group::{Curve, Group};
use rand::{Rng, SeedableRng};
use std::marker::PhantomData;

use ff::{Field, PrimeField};
use halo2_proofs::{
    circuit::SimpleFloorPlanner,
    plonk::{
        create_proof, keygen_pk, keygen_vk,
        static_lookup::{StaticCommittedTable, StaticTable, StaticTableId, StaticTableValues},
        verify_proof, Advice, Circuit, Column,
    },
    poly::{
        commitment::ParamsProver,
        kzg::{
            commitment::{KZGCommitmentScheme, ParamsKZG},
            multiopen::{ProverGWC, VerifierGWC},
            strategy::AccumulatorStrategy,
        },
        Rotation, VerificationStrategy,
    },
    transcript::{
        Blake2bRead, Blake2bWrite, Challenge255, TranscriptReadBuffer, TranscriptWriterBuffer,
    },
};
use halo2curves::{
    bn256::{Bn256, Fq2Bytes},
    pairing::{Engine, MillerLoopResult, MultiMillerLoop},
    serde::SerdeObject,
    CurveAffine, FieldExt,
};
use rand_core::{OsRng, RngCore};

#[derive(Clone)]
struct MyCircuit<E: MultiMillerLoop> {
    table: StaticTable<E>,
}

impl<E: MultiMillerLoop<Scalar = F>, F: Field> Circuit<E> for MyCircuit<E> {
    type Config = Column<Advice>;

    type FloorPlanner = SimpleFloorPlanner<E>;

    fn without_witnesses(&self) -> Self {
        self.clone()
    }

    fn configure(meta: &mut halo2_proofs::plonk::ConstraintSystem<F>) -> Self::Config {
        let advice = meta.advice_column();
        meta.lookup_static("lookup_bits", |meta| {
            (
                meta.query_advice(advice, Rotation::cur()),
                StaticTableId(String::from("bits_table")),
            )
        });

        advice
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl halo2_proofs::circuit::Layouter<F, E = E>,
    ) -> Result<(), halo2_proofs::plonk::Error> {
        layouter.register_static_table(
            StaticTableId(String::from("bits_table")),
            self.table.clone(),
        );

        Ok(())
    }
}

// ascii of cq
static SEED: [u8; 32] = [
    99, 113, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0,
];

// helper test for constructing a table: TODO: make this into a bin file
#[test]
fn static_table() {
    use halo2curves::bn256::Fr;
    const K: u32 = 6;

    let mut rng = rand_chacha::ChaCha8Rng::from_seed(SEED);
    let params = ParamsKZG::<Bn256>::new(K, &mut rng);

    let opened_ = StaticTableValues::<Bn256> { x: Fr::from(5) };
    let committed = opened_.commit(params.g2());

    let x_inv = Fr::from(5).invert().unwrap();
    let lhs = <Bn256 as Engine>::G1Affine::generator() * x_inv;
    let lhs = lhs.to_affine();

    assert_eq!(
        Bn256::pairing(&lhs, &committed.x),
        Bn256::pairing(
            &<Bn256 as Engine>::G1Affine::generator(),
            &<Bn256 as Engine>::G2Affine::generator()
        )
    );
    println!("opened: {:?}", opened_);
    println!(
        "committed x: {:?}",
        committed.x.coordinates().unwrap().x().to_bytes()
    );
    println!(
        "committed y: {:?}",
        committed.x.coordinates().unwrap().y().to_bytes()
    );
}

use halo2curves::bn256::{Fq2, Fr, G2Affine};
// #[macro_use]
// extern crate lazy_static;
// lazy_static! {
//     static ref OPENED: StaticTableValues<Bn256> = StaticTableValues { x: Fr::from(5) };
//     static ref TABLE: StaticTable<Bn256> = StaticTable {
//         opened: Some(&OPENED),
//         committed: Some(StaticCommittedTable {
//             x: <Bn256 as Engine>::G2Affine::from_xy(
//                 Fq2::from_bytes(&[
//                     216, 16, 100, 160, 144, 131, 112, 19, 145, 154, 138, 174, 248, 93, 219, 245,
//                     234, 72, 57, 96, 60, 119, 229, 244, 19, 45, 48, 59, 66, 156, 83, 46, 161, 178,
//                     40, 228, 16, 229, 113, 6, 213, 89, 55, 175, 197, 122, 181, 87, 36, 22, 225,
//                     222, 8, 18, 28, 157, 217, 95, 181, 97, 245, 204, 9, 10
//                 ])
//                 .unwrap(),
//                 Fq2::from_bytes(&[
//                     246, 107, 111, 251, 148, 211, 70, 157, 212, 90, 83, 207, 76, 4, 35, 235, 229,
//                     182, 183, 60, 6, 236, 47, 122, 199, 39, 55, 184, 154, 159, 141, 47, 99, 199,
//                     252, 28, 1, 20, 20, 210, 87, 64, 33, 228, 254, 87, 214, 193, 27, 28, 201, 120,
//                     13, 189, 238, 228, 54, 167, 36, 57, 81, 99, 183, 25
//                 ])
//                 .unwrap()
//             )
//             .unwrap()
//         })
//     };
// }

fn generate_table<E: MultiMillerLoop>(x: E::Scalar) -> StaticTable<E> {
    use group::prime::PrimeCurveAffine;
    // let mut rng = rand_chacha::ChaCha8Rng::from_seed(SEED);
    // let params = ParamsKZG::<Bn256>::new(K, &mut rng);

    let opened = StaticTableValues::<E> { x };
    let committed = opened.commit(<E as Engine>::G2Affine::generator());

    StaticTable {
        opened: Some(opened),
        committed: Some(committed),
    }

    // let x_inv = Fr::from(5).invert().unwrap();
    // let lhs = <Bn256 as Engine>::G1Affine::generator() * x_inv;
    // let lhs = lhs.to_affine();
    // todo!()
}

// #[test]
// fn table_sanity() {
//     let x_inv = Fr::from(5).invert().unwrap();
//     let lhs = <Bn256 as Engine>::G1Affine::generator() * x_inv;
//     let lhs = lhs.to_affine();

//     assert_eq!(
//         Bn256::pairing(&lhs, &TABLE.clone().committed.unwrap().x),
//         Bn256::pairing(
//             &<Bn256 as Engine>::G1Affine::generator(),
//             &<Bn256 as Engine>::G2Affine::generator()
//         )
//     );
// }

#[test]
fn my_test_e2e() {
    const K: u32 = 6;
    let mut rng = rand_chacha::ChaCha8Rng::from_seed(SEED);

    let table = generate_table(<Bn256 as Engine>::Scalar::from(5));

    let params = ParamsKZG::<Bn256>::new(K, &mut rng);
    let circuit = MyCircuit { table };

    // Initialize the proving key
    let vk = keygen_vk(&params, &circuit).expect("keygen_vk should not fail");

    let pk = keygen_pk(&params, vk, &circuit).expect("keygen_pk should not fail");

    // Create proof
    let proof = {
        let mut transcript = Blake2bWrite::<_, _, Challenge255<_>>::init(vec![]);
        // Create a proof
        create_proof::<Bn256, ProverGWC<_>, _, _, _, _>(
            &params,
            &pk,
            &[circuit],
            &[],
            OsRng,
            &mut transcript,
        )
        .unwrap();

        transcript.finalize()
    };

    let mut transcript = Blake2bRead::<_, _, Challenge255<_>>::init(&proof[..]);

    let verifier_params = params.verifier_params();
    let strategy = VerificationStrategy::<Bn256, VerifierGWC<_>>::new(verifier_params);

    let p_batcher = verify_proof::<
        Bn256,
        VerifierGWC<_>,
        _,
        Blake2bRead<_, _, Challenge255<_>>,
        AccumulatorStrategy<_>,
    >(verifier_params, pk.get_vk(), strategy, &[], &mut transcript)
    .unwrap();

    let batched_tuples = p_batcher.finalize();
    let result = Bn256::multi_miller_loop(
        &batched_tuples
            .iter()
            .map(|(g1, g2)| (g1, g2))
            .collect::<Vec<_>>(),
    );

    let pairing_result = result.final_exponentiation();
    assert!(bool::from(pairing_result.is_identity()));
}
