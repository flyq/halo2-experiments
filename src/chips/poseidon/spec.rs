use halo2_proofs::{halo2curves::pasta::Fp, arithmetic::Field};
use halo2_gadgets::poseidon::primitives::*;

// P128Pow5T3 is the default Spec provided by the Halo2 Gadget => https://github.com/privacy-scaling-explorations/halo2/blob/main/halo2_gadgets/src/poseidon/primitives/p128pow5t3.rs#L13
// This spec hardcodes the WIDTH and RATE parameters of the hash function to 3 and 2 respectively
// This is problematic because to perform an hash of a input array of length 4, we need the WIDTH parameter to be higher than 3
// Since the WIDTH parameter is used to define the number of hash_inputs column in the PoseidonChip.
// Because of that we need to define a new Spec
// MySpec struct allows us to define the parameters of the Poseidon hash function WIDTH and RATE
#[derive(Debug, Clone, Copy)]
pub struct MySpec<const WIDTH: usize, const RATE: usize>;

impl<const WIDTH: usize, const RATE: usize> Spec<Fp, WIDTH, RATE> for MySpec<WIDTH, RATE> {
    fn full_rounds() -> usize {
        8
    }

    fn partial_rounds() -> usize {
        56
    }

    fn sbox(val: Fp) -> Fp {
        val.pow_vartime(&[5])
    }

    fn secure_mds() -> usize {
        0
    }
}