use std::{marker::PhantomData};

use halo2_proofs::{
    arithmetic::FieldExt,
    circuit::*,
    plonk::*,
    poly::Rotation
};

#[derive(Debug, Clone)]
pub struct MerkleTreeV1Config { 
    pub advice: [ Column<Advice>; 3],
    pub bool_selector: Selector,
    pub swap_selector: Selector,
    pub hash_selector: Selector,
    pub instance: Column<Instance>,
}

pub struct MerkleTreeV1Chip<F: FieldExt>  {
    config: MerkleTreeV1Config,
    _marker: PhantomData<F>,
}

impl<F: FieldExt> MerkleTreeV1Chip<F> {

    pub fn construct(config:MerkleTreeV1Config) -> Self {
        Self {
            config,
            _marker: PhantomData
        }
    }

    pub fn configure(
        meta: &mut ConstraintSystem<F>,
        advice: [Column<Advice>; 3],
        bool_selector: Selector,
        swap_selector: Selector,
        hash_selector: Selector,
        instance: Column<Instance>,
    ) -> MerkleTreeV1Config {

        let col_a = advice[0];
        let col_b = advice[1];
        let col_c = advice[2];

        // Enable equality on the advice column c and instance column to enable permutation check 
        // between the last hash digest and the root hash passed inside the instance column
        meta.enable_equality(col_c);
        meta.enable_equality(instance);

        // Enable equality on the advice column a. This is need to carry digest from one level to the other
        // and perform copy_advice
        meta.enable_equality(col_a);

        // Enforces that c is either a 0 or 1 when the bool selector is enabled
        // s * c * (1 - c) = 0
        meta.create_gate("bool constraint", |meta| {
            let s = meta.query_selector(bool_selector);
            let c = meta.query_advice(col_c, Rotation::cur());
            vec![s * c.clone() * (Expression::Constant(F::from(1)) - c.clone())]
        });

        // Enforces that if the swap bit (c) is on, l=b and r=a. Otherwise, l=a and r=b.
        // s * (c * 2 * (b - a) - (l - a) - (b - r)) = 0
        // This applies only when the swap selector is enabled
        meta.create_gate("swap constraint", |meta| {
            let s = meta.query_selector(swap_selector);
            let a = meta.query_advice(col_a, Rotation::cur());
            let b = meta.query_advice(col_b, Rotation::cur());
            let c = meta.query_advice(col_c, Rotation::cur());
            let l = meta.query_advice(col_a, Rotation::next());
            let r = meta.query_advice(col_b, Rotation::next());
            vec![
                s * (c * Expression::Constant(F::from(2)) * (b.clone() - a.clone())
                    - (l - a.clone())
                    - (b.clone() - r)),
            ]
        });

        // enforce dummy hash function when hash selector is enabled
        // enforce a + b = c, namely a + b - c = 0
        meta.create_gate("hash constraint", |meta| {
            let s = meta.query_selector(hash_selector);
            let a = meta.query_advice(col_a, Rotation::cur());
            let b = meta.query_advice(col_b, Rotation::cur());
            let c = meta.query_advice(col_c, Rotation::cur());
       
            vec![s * (a + b - c)]
        });

        MerkleTreeV1Config {
            advice: [col_a, col_b, col_c],
            bool_selector,
            hash_selector,
            swap_selector,
            instance
        }
    }

    pub fn assign_hashing_region(   
        &self,
        mut layouter: impl Layouter<F>,
        leaf: Value<F>,
        path_element: Value<F>,
        index: Value<F>,
        prev_digest: Option<&AssignedCell<F, F>>,
        tree_level: usize,
    ) -> Result<AssignedCell<F, F>, Error> {

        layouter.assign_region(
            || format!("layer {}", tree_level),
            |mut region| {

            // Enabled Selectors at offset 0: Bool, Swap
            self.config.bool_selector.enable(&mut region, 0)?;
            self.config.swap_selector.enable(&mut region, 0)?;

            let to_hash;

            if tree_level == 0 {
                // Row 0: | Leaf | Path | Bit |
                region.assign_advice(|| "leaf", self.config.advice[0], 0, || leaf)?;
                to_hash = leaf;
            } else {
                // Row 0: | prev_digest | Path | Bit |
                prev_digest.unwrap().copy_advice(
                    || "prev digest",
                    &mut region,
                    self.config.advice[0],
                    0,
                )?;
                to_hash = prev_digest.unwrap().value().map(|x| x.to_owned());
            }

            region.assign_advice(|| "path", self.config.advice[1], 0, || path_element)?;
            region.assign_advice(|| "bit", self.config.advice[2], 0, || index)?;

            // Row 1: | InputLeft | InputRight | Digest |
            // Enabled Selectors: Hash

            let mut input_l = to_hash;
            let mut input_r = path_element;
            index.map(|index| {
                if index != F::zero() {
                    (input_l, input_r) = (path_element, to_hash);
                }
            });

            region.assign_advice(|| "input left", self.config.advice[0], 1, || input_l)?;
            region.assign_advice(|| "input right", self.config.advice[1], 1, || input_r)?;

            let digest_cell = region.assign_advice(
            || "digest",
            self.config.advice[2],
            1,
            || input_l + input_r
            )?;

            Ok(digest_cell)
        })

    }

    // Enforce permutation check between b cell and instance column
    pub fn expose_public(
        &self,
        mut layouter: impl Layouter<F>,
        b_cell: &AssignedCell<F, F>,
        row: usize,
    ) -> Result<(), Error> {
        layouter.constrain_instance(b_cell.cell(), self.config.instance, row)
    }

}