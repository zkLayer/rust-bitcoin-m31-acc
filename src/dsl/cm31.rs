use crate::cm31::{CM31Mult, CM31MultGadget};
use crate::dsl::m31::m31_to_limbs_gadget;
use crate::treepp::*;
use crate::utils::convert_cm31_from_limbs;
use anyhow::Error;
use anyhow::Result;
use bitcoin_script_dsl::dsl::{Element, MemoryEntry, DSL};
use bitcoin_script_dsl::functions::{FunctionMetadata, FunctionOutput};
use stwo_prover::core::fields::cm31::CM31;

pub fn cm31_to_limbs(dsl: &mut DSL, inputs: &[usize]) -> Result<FunctionOutput> {
    let num = dsl.get_many_num(inputs[0])?;

    let limbs = vec![
        num[1] & 0xff,
        (num[1] >> 8) & 0xff,
        (num[1] >> 16) & 0xff,
        (num[1] >> 24) & 0xff,
        num[0] & 0xff,
        (num[0] >> 8) & 0xff,
        (num[0] >> 16) & 0xff,
        (num[0] >> 24) & 0xff,
    ];

    let new_entry = MemoryEntry::new("cm31_limbs", Element::ManyNum(limbs));

    Ok(FunctionOutput {
        new_elements: vec![new_entry.clone()],
        new_hints: vec![new_entry],
    })
}

pub fn cm31_to_limbs_gadget(_: &[usize]) -> Result<Script> {
    // Hint: eight limbs
    // Input: cm31
    // Output: eight limbs
    Ok(script! {
        { m31_to_limbs_gadget(&[])? }
        4 OP_ROLL
        { m31_to_limbs_gadget(&[])? }
    })
}

pub fn cm31_limbs_equalverify(dsl: &mut DSL, inputs: &[usize]) -> Result<FunctionOutput> {
    let a = dsl.get_many_num(inputs[0])?.to_vec();
    let b = dsl.get_many_num(inputs[1])?.to_vec();

    if a != b {
        Err(Error::msg("Equalverify fails"))
    } else {
        Ok(FunctionOutput {
            new_elements: vec![],
            new_hints: vec![],
        })
    }
}

pub fn cm31_limbs_equalverify_gadget(_: &[usize]) -> Result<Script> {
    Ok(script! {
        for i in (3..=8).rev() {
            { i } OP_ROLL OP_EQUALVERIFY
        }
        OP_ROT OP_EQUALVERIFY
        OP_EQUALVERIFY
    })
}

pub fn cm31_limbs_mul(dsl: &mut DSL, inputs: &[usize]) -> Result<FunctionOutput> {
    let a = dsl.get_many_num(inputs[1])?.to_vec();
    let b = dsl.get_many_num(inputs[2])?.to_vec();

    let hint = CM31Mult::compute_hint_from_limbs(&a[0..4], &a[4..8], &b[0..4], &b[4..8])?;

    let a_cm31 = convert_cm31_from_limbs(&a);
    let b_cm31 = convert_cm31_from_limbs(&b);

    let expected = a_cm31 * b_cm31;

    Ok(FunctionOutput {
        new_elements: vec![MemoryEntry::new(
            "cm31",
            Element::ManyNum(reformat_cm31_to_dsl_element(expected)),
        )],
        new_hints: vec![
            MemoryEntry::new("m31", Element::Num(hint.q1)),
            MemoryEntry::new("m31", Element::Num(hint.q2)),
            MemoryEntry::new("m31", Element::Num(hint.q3)),
        ],
    })
}

pub fn cm31_limbs_mul_gadget(r: &[usize]) -> Result<Script> {
    Ok(CM31MultGadget::mult(r[0] - 512))
}

pub(crate) fn reformat_cm31_to_dsl_element(v: CM31) -> Vec<i32> {
    vec![v.1 .0 as i32, v.0 .0 as i32]
}

pub(crate) fn load_functions(dsl: &mut DSL) {
    dsl.add_function(
        "cm31_to_limbs",
        FunctionMetadata {
            trace_generator: cm31_to_limbs,
            script_generator: cm31_to_limbs_gadget,
            input: vec!["cm31"],
            output: vec!["cm31_limbs"],
        },
    );
    dsl.add_function(
        "cm31_limbs_equalverify",
        FunctionMetadata {
            trace_generator: cm31_limbs_equalverify,
            script_generator: cm31_limbs_equalverify_gadget,
            input: vec!["cm31_limbs", "cm31_limbs"],
            output: vec![],
        },
    );
    dsl.add_function(
        "cm31_limbs_mul",
        FunctionMetadata {
            trace_generator: cm31_limbs_mul,
            script_generator: cm31_limbs_mul_gadget,
            input: vec!["table", "cm31_limbs", "cm31_limbs"],
            output: vec!["cm31"],
        },
    );
}

#[cfg(test)]
mod test {
    use crate::dsl::{load_data_types, load_functions};
    use crate::utils::convert_cm31_to_limbs;
    use bitcoin_script_dsl::dsl::{Element, DSL};
    use bitcoin_script_dsl::test_program;
    use rand::{Rng, SeedableRng};
    use rand_chacha::ChaCha20Rng;
    use stwo_prover::core::fields::cm31::CM31;

    #[test]
    fn test_cm31_to_limbs() {
        let mut prng = ChaCha20Rng::seed_from_u64(0);

        let a_real = prng.gen_range(0u32..((1 << 31) - 1));
        let a_imag = prng.gen_range(0u32..((1 << 31) - 1));

        let mut dsl = DSL::new();

        load_data_types(&mut dsl);
        load_functions(&mut dsl);

        let a = dsl
            .alloc_constant("cm31", Element::ManyNum(vec![a_imag as i32, a_real as i32]))
            .unwrap();
        let res = dsl.execute("cm31_to_limbs", &[a]).unwrap();

        assert_eq!(res.len(), 1);
        assert_eq!(
            dsl.get_many_num(res[0]).unwrap(),
            convert_cm31_to_limbs(a_real, a_imag)
        );

        let expected = dsl
            .alloc_constant(
                "cm31_limbs",
                Element::ManyNum(convert_cm31_to_limbs(a_real, a_imag).to_vec()),
            )
            .unwrap();
        let _ = dsl
            .execute("cm31_limbs_equalverify", &[res[0], expected])
            .unwrap();

        test_program(dsl).unwrap();
    }

    #[test]
    fn test_cm31_limbs_mul() {
        let mut prng = ChaCha20Rng::seed_from_u64(0);

        let a_real = prng.gen_range(0u32..((1 << 31) - 1));
        let a_imag = prng.gen_range(0u32..((1 << 31) - 1));
        let b_real = prng.gen_range(0u32..((1 << 31) - 1));
        let b_imag = prng.gen_range(0u32..((1 << 31) - 1));

        let a_cm31 = CM31::from_u32_unchecked(a_real, a_imag);
        let b_cm31 = CM31::from_u32_unchecked(b_real, b_imag);

        let expected = a_cm31 * b_cm31;

        let a_limbs = convert_cm31_to_limbs(a_real, a_imag);
        let b_limbs = convert_cm31_to_limbs(b_real, b_imag);

        let mut dsl = DSL::new();

        load_data_types(&mut dsl);
        load_functions(&mut dsl);

        let a = dsl
            .alloc_input("cm31_limbs", Element::ManyNum(a_limbs.to_vec()))
            .unwrap();
        let b = dsl
            .alloc_input("cm31_limbs", Element::ManyNum(b_limbs.to_vec()))
            .unwrap();

        let table = dsl.execute("push_table", &[]).unwrap()[0];
        let res = dsl.execute("cm31_limbs_mul", &[table, a, b]).unwrap();

        assert_eq!(res.len(), 1);
        assert_eq!(
            dsl.get_many_num(res[0]).unwrap(),
            &[expected.1 .0 as i32, expected.0 .0 as i32]
        );

        test_program(dsl).unwrap();
    }
}
