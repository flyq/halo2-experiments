use crate::chips::hash_v1::{Hash1Chip, Hash1Config};
use eth_types::Field;
use gadgets::less_than::{LtChip, LtConfig, LtInstruction};
use std::marker::PhantomData;

use halo2_proofs::{circuit::*, plonk::*, poly::Rotation};

#[derive(Default)]
// define circuit struct using array of usernames and balances
struct MyCircuit<F> {
    pub value_l: u64,
    pub value_r: u64,
    pub check: bool,
    _marker: PhantomData<F>,
}

#[derive(Clone, Debug)]
struct TestCircuitConfig<F> {
    q_enable: Selector,
    value_l: Column<Advice>,
    value_r: Column<Advice>,
    check: Column<Advice>,
    lt: LtConfig<F, 8>,
    hash_config: Hash1Config,
}

impl<F: Field> Circuit<F> for MyCircuit<F> {
    type Config = TestCircuitConfig<F>;
    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        let q_enable = meta.complex_selector();
        let value_l = meta.advice_column();
        let value_r = meta.advice_column();
        let check = meta.advice_column();
        let instance = meta.instance_column();

        let lt = LtChip::configure(
            meta,
            |meta| meta.query_selector(q_enable),
            |meta| meta.query_advice(value_l, Rotation::cur()),
            |meta| meta.query_advice(value_r, Rotation::cur()),
        );

        let hash_config = Hash1Chip::configure(meta, [value_l, value_r], instance);

        let config = Self::Config {
            q_enable,
            value_l,
            value_r,
            check,
            lt,
            hash_config,
        };

        meta.create_gate(
            "verifies that `check` current confif = is_lt from LtChip ",
            |meta| {
                let q_enable = meta.query_selector(q_enable);

                // This verifies lt(value_l::cur, value_r::cur) is calculated correctly
                let check = meta.query_advice(config.check, Rotation::cur());

                // verifies that check is equal to lt in the child chip
                vec![q_enable * (config.lt.is_lt(meta, None) - check)]
            },
        );

        config
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        let lt_chip = LtChip::construct(config.lt);
        lt_chip.load(&mut layouter)?;
        let hash_chip: Hash1Chip<F> = Hash1Chip::construct(config.hash_config);

        let _ = layouter.assign_region(
            || "witness",
            |mut region| {
                region.assign_advice(
                    || "value left",
                    config.value_l,
                    0,
                    || Value::known(F::from(self.value_l)),
                )?;

                region.assign_advice(
                    || "value right",
                    config.value_r,
                    0,
                    || Value::known(F::from(self.value_r)),
                )?;

                region.assign_advice(|| "check", config.check, 0, || Value::known(F::from(1)))?;

                config.q_enable.enable(&mut region, 0)?;

                lt_chip.assign(&mut region, 0, F::from(self.value_l), F::from(self.value_r))?;

                Ok(())
            },
        );

        let b = hash_chip.assign_advice_row(
            layouter.namespace(|| "load row"),
            Value::known(F::from(self.value_l)),
        )?;
        hash_chip.expose_public(layouter.namespace(|| "hash output check"), &b, 0)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::MyCircuit;
    use halo2_proofs::{dev::MockProver, halo2curves::bn256::Fr as Fp};
    use std::marker::PhantomData;

    #[test]
    fn test_less_than_3() {
        let k = 9;

        // initate usernames and balances array
        let value_l: u64 = 5;
        let value_r: u64 = 10;
        let check = true;

        let mut circuit = MyCircuit::<Fp> {
            value_l,
            value_r,
            check,
            _marker: PhantomData,
        };

        let public_input_1 = vec![Fp::from(10)];

        // Test 1 - should be valid
        let prover = MockProver::run(k, &circuit, vec![public_input_1.clone()]).unwrap();
        prover.assert_satisfied();

        // switch value_l and value_r
        circuit.value_l = 10;
        circuit.value_r = 5;

        // Test 2 - should be invalid
        let prover = MockProver::run(k, &circuit, vec![public_input_1.clone()]).unwrap();
        assert!(prover.verify().is_err());

        // let check to be false
        circuit.check = false;

        // Test 3 - should be invalid! as we are now forcing the check to be true
        let prover = MockProver::run(k, &circuit, vec![public_input_1]).unwrap();
        assert!(prover.verify().is_err());
    }
}
