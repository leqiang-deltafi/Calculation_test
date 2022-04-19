//! Calculation functions

use crate::{
    math::{Decimal, TryAdd, TryDiv, TryMul, TrySub},
};
use solana_program::program_error::ProgramError;


/// test simple power with float
pub fn simple_powf(
    market_price: Decimal,
    target_reserve_a: Decimal, target_reserve_b: Decimal, 
    current_reserve_a: Decimal, current_reserve_b: Decimal,
    input_a_amount: Decimal
) ->Result<u64, ProgramError> {
    let core: Decimal = current_reserve_a.try_div(current_reserve_a.try_add(input_a_amount)?)?;
    let exp: Decimal = market_price.try_mul(target_reserve_a)?.try_div(target_reserve_b)?;
    // let mut core: f64 = (current_reserve_a as f64) / (current_reserve_a as f64 + input_a_amount as f64);
    // let exp: f64 = ((market_price*target_reserve_a) as f64) / (target_reserve_b as f64);

    let core_float64: f64 = core.to_float64()?;
    let exp_float64: f64 = exp.to_float64()?;

    let core_exp_float64: f64 = core_float64.powf(exp_float64);
    let core_exp: Decimal = Decimal::from_float64(core_exp_float64);

    let result: Decimal = current_reserve_b.try_mul(Decimal::one().try_sub(core_exp)?)?;

    Ok(result.try_floor_u64()?)
}


#[cfg(test)]
mod tests {}
