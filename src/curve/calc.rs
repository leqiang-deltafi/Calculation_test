//! Calculation functions

use crate::{
    error::SwapError,
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


/// Get target amount given quote amount.
///
/// target_amount = market_price * quote_amount * (1 - slope
///         + slope * (target_reserve^2 / future_reserve / current_reserve))
/// where quote_amount = future_reserve - current_reserve.
///
/// # Arguments
///
/// * target_reserve - initial reserve position to track divergent loss.
/// * future_reserve - reserve position after the current quoted trade.
/// * current_reserve - current reserve position.
/// * market price - fair market price determined by internal and external oracle.
/// * slope - the higher the curve slope is, the bigger the price splippage.
///
/// # Return value
///
/// target amount determined by the pricing function.
pub fn get_target_amount(
    target_reserve: Decimal,
    future_reserve: Decimal,
    current_reserve: Decimal,
    market_price: Decimal,
    slope: Decimal,
) -> Result<Decimal, ProgramError> {
    if current_reserve <= Decimal::zero()
        || future_reserve < current_reserve
        || future_reserve > target_reserve
    {
        return Err(SwapError::CalculationFailure.into());
    }

    let fair_amount = future_reserve
        .try_sub(current_reserve)?
        .try_mul(market_price)?;

    if slope > Decimal::one() {
        return Err(SwapError::InvalidSlope.into());
    }

    if slope.is_zero() {
        return Ok(fair_amount);
    }
    let penalty_ratio = target_reserve
        .try_mul(target_reserve)?
        .try_div(future_reserve)?
        .try_div(current_reserve)?;
    let penalty = penalty_ratio.try_mul(slope)?;
    fair_amount.try_mul(penalty.try_add(Decimal::one())?.try_sub(slope)?)
}

/// Get target amount given quote amount in reserve direction.
///
/// # Arguments
///
/// * target_reserve - initial reserve position to track divergent loss.
/// * current_reserve - current reserve position.
/// * quote_amount - quote amount.
/// * market price - fair market price determined by internal and external oracle.
/// * slope - the higher the curve slope is, the bigger the price splippage.
///
/// # Return value
///
/// target amount determined by the pricing function.
pub fn get_target_amount_reverse_direction(
    target_reserve: Decimal,
    current_reserve: Decimal,
    quote_amount: Decimal,
    market_price: Decimal,
    slope: Decimal,
) -> Result<Decimal, ProgramError> {
    if target_reserve <= Decimal::zero() {
        return Err(SwapError::CalculationFailure.into());
    }

    if quote_amount.is_zero() {
        return Ok(Decimal::zero());
    }

    if slope > Decimal::one() {
        return Err(SwapError::InvalidSlope.into());
    }

    let fair_amount = quote_amount.try_mul(market_price)?;
    if slope.is_zero() {
        return Ok(fair_amount.min(current_reserve));
    }

    if slope == Decimal::one() {
        let adjusted_ratio = if fair_amount.is_zero() {
            Decimal::zero()
        } else {
            fair_amount
                .try_mul(current_reserve)?
                .try_div(target_reserve)?
                .try_div(target_reserve)?
        };

        return current_reserve
            .try_mul(adjusted_ratio)?
            .try_div(adjusted_ratio.try_add(Decimal::one())?);
    }

    let future_reserve = slope
        .try_mul(target_reserve)?
        .try_div(current_reserve)?
        .try_mul(target_reserve)?
        .try_add(fair_amount)?;
    let mut adjusted_reserve = Decimal::one().try_sub(slope)?.try_mul(current_reserve)?;

    let is_smaller = if adjusted_reserve < future_reserve {
        adjusted_reserve = future_reserve.try_sub(adjusted_reserve)?;
        true
    } else {
        adjusted_reserve = adjusted_reserve.try_sub(future_reserve)?;
        false
    };
    adjusted_reserve = Decimal::from(adjusted_reserve.try_floor_u64()?);

    let square_root = Decimal::one()
        .try_sub(slope)?
        .try_mul(4)?
        .try_mul(slope)?
        .try_mul(target_reserve)?
        .try_mul(target_reserve)?;
    let square_root = adjusted_reserve
        .try_mul(adjusted_reserve)?
        .try_add(square_root)?
        .sqrt()?;

    let denominator = Decimal::one().try_sub(slope)?.try_mul(2)?;
    let numerator = if is_smaller {
        square_root.try_sub(adjusted_reserve)?
    } else {
        adjusted_reserve.try_add(square_root)?
    };

    let candidate_reserve = numerator.try_div(denominator)?;
    if candidate_reserve > current_reserve {
        Ok(Decimal::zero())
    } else {
        current_reserve.try_sub(candidate_reserve)
    }
}

/// Get adjusted target reserve given quote amount.
///
/// # Arguments
///
/// * current_reserve - current reserve position.
/// * quote_amount - quote amount.
/// * market price - fair market price determined by internal and external oracle.
/// * slope - the higher the curve slope is, the bigger the price splippage.
///
/// # Return value
///
/// adjusted target reserve.
pub fn get_target_reserve(
    current_reserve: Decimal,
    quote_amount: Decimal,
    market_price: Decimal,
    slope: Decimal,
) -> Result<Decimal, ProgramError> {
    if current_reserve.is_zero() {
        return Ok(Decimal::zero());
    }
    if slope.is_zero() {
        return quote_amount.try_mul(market_price)?.try_add(current_reserve);
    }

    if slope > Decimal::one() {
        return Err(SwapError::InvalidSlope.into());
    }

    let price_offset = market_price.try_mul(slope)?.try_mul(4)?;
    let square_root = if price_offset.is_zero() {
        Decimal::one()
    } else {
        price_offset
            .try_mul(quote_amount)?
            .try_div(current_reserve)?
            .try_add(Decimal::one())?
            .sqrt()?
    };

    let premium = square_root
        .try_sub(Decimal::one())?
        .try_div(2)?
        .try_div(slope)?
        .try_add(Decimal::one())?;

    premium.try_mul(current_reserve)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::curve::{default_market_price, default_slope};

    #[test]
    fn test_get_target_amount_reverse_direction() {
        let target_reserve = Decimal::from(2_000_000u64);
        let current_reserve = Decimal::from(1_000_000u64);
        let quote_amount = Decimal::from(3_000u64);
        let slope: Decimal = default_slope();
        let market_price: Decimal = default_market_price();

        // Test failures on get_target_amount_reverse_direction
        assert!(get_target_amount_reverse_direction(
            target_reserve,
            Decimal::zero(),
            quote_amount,
            market_price,
            slope
        )
        .is_err());

        assert!(get_target_amount_reverse_direction(
            target_reserve,
            Decimal::zero(),
            quote_amount,
            market_price,
            Decimal::from(2u64)
        )
        .is_err());

        assert_eq!(
            get_target_amount_reverse_direction(
                target_reserve,
                current_reserve,
                Decimal::zero(),
                market_price,
                slope
            )
            .unwrap(),
            Decimal::zero()
        );

        assert_eq!(
            get_target_amount_reverse_direction(
                target_reserve,
                current_reserve,
                quote_amount,
                market_price,
                Decimal::zero()
            )
            .unwrap(),
            Decimal::from(300_000u64)
        );

        assert_eq!(
            get_target_amount_reverse_direction(
                target_reserve,
                current_reserve,
                quote_amount,
                market_price,
                Decimal::zero()
            )
            .unwrap(),
            Decimal::from(300_000u64)
        );

        // lower slope, target amount closer to fair amount
        assert_eq!(
            get_target_amount_reverse_direction(
                target_reserve,
                current_reserve,
                quote_amount,
                market_price,
                slope // 0.1
            )
            .unwrap(),
            Decimal::from_scaled_val(213026385522420035_u128)
        );

        assert_eq!(
            get_target_amount_reverse_direction(
                target_reserve,
                current_reserve,
                quote_amount,
                market_price,
                Decimal::from_scaled_val(10_000_000_000_u128) // 0.01
            )
            .unwrap(),
            Decimal::from_scaled_val(286783858337550955_u128)
        );

        assert_eq!(
            get_target_amount_reverse_direction(
                target_reserve,
                current_reserve,
                quote_amount,
                market_price,
                Decimal::from_scaled_val(1_000_000_000_u128) // 0.001
            )
            .unwrap(),
            Decimal::from_scaled_val(298595750348446284_u128)
        );

        assert_eq!(
            get_target_amount_reverse_direction(
                target_reserve,
                current_reserve,
                quote_amount,
                market_price,
                Decimal::from_scaled_val(100_000_000_u128) // 0.0001
            )
            .unwrap(),
            Decimal::from_scaled_val(299858672641819676_u128)
        );
    }

    #[test]
    fn test_get_target_amount_reverse_direction_stable() {
        let high_reserve = Decimal::from(2_000_000u64);
        let low_reserve = Decimal::from(1_000_000u64);
        let slope: Decimal = Decimal::from_scaled_val(10_000_000_u128); // 0.00001
        let market_price: Decimal = Decimal::one();

        assert_eq!(
            get_target_amount_reverse_direction(
                high_reserve,
                low_reserve,
                Decimal::from(1_000_000u64),
                market_price,
                slope
            )
            .unwrap(),
            Decimal::from_scaled_val(993700_363895515373_u128)
        );

        assert_eq!(
            get_target_amount_reverse_direction(
                high_reserve,
                low_reserve,
                Decimal::from(300_000u64),
                market_price,
                slope
            )
            .unwrap(),
            Decimal::from_scaled_val(299985_858155851637u128)
        );

        assert_eq!(
            get_target_amount_reverse_direction(
                low_reserve,
                high_reserve,
                Decimal::from(1_000_000u64),
                market_price,
                slope
            )
            .unwrap(),
            Decimal::from_scaled_val(1000004_999999999751u128)
        );

        assert_eq!(
            get_target_amount_reverse_direction(
                low_reserve,
                high_reserve,
                Decimal::from(300_000u64),
                market_price,
                slope
            )
            .unwrap(),
            Decimal::from_scaled_val(300002_117660907878u128)
        );
    }

    #[test]
    fn test_get_target_reserve() {
        let current_reserve = Decimal::from(1_000_000u64);
        let quote_amount = Decimal::from(3_000u64);
        let slope: Decimal = default_slope();
        let market_price: Decimal = Decimal::from(100u64);

        assert_eq!(
            // slope = 0.1
            get_target_reserve(Decimal::zero(), quote_amount, market_price, slope).unwrap(),
            Decimal::zero()
        );

        assert!(get_target_reserve(
            current_reserve,
            quote_amount,
            market_price,
            Decimal::from(2u64)
        )
        .is_err());

        // lower slope, target reserve closer to fair amount
        assert_eq!(
            get_target_reserve(current_reserve, quote_amount, market_price, Decimal::zero())
                .unwrap(),
            Decimal::from(1_300_000u64)
        );

        assert_eq!(
            get_target_reserve(
                current_reserve,
                quote_amount,
                market_price,
                slope // 0.1
            )
            .unwrap(),
            Decimal::from_scaled_val(1291502_622120000000_u128)
        );

        assert_eq!(
            get_target_reserve(
                current_reserve,
                quote_amount,
                market_price,
                Decimal::from_scaled_val(10_000_000_000_u128) // 0.01
            )
            .unwrap(),
            Decimal::from_scaled_val(1299105359800000000_u128)
        );

        assert_eq!(
            get_target_reserve(
                current_reserve,
                quote_amount,
                market_price,
                Decimal::from_scaled_val(1_000_000_000_u128) // 0.001
            )
            .unwrap(),
            Decimal::from_scaled_val(1299910053000000000_u128)
        );

        assert_eq!(
            get_target_reserve(
                current_reserve,
                quote_amount,
                market_price,
                Decimal::from_scaled_val(100_000_000_u128) // 0.0001
            )
            .unwrap(),
            Decimal::from_scaled_val(1299991000000000000_u128)
        );
    }

    #[test]
    fn test_get_target_amount() {
        let slope: Decimal = default_slope();
        let market_price: Decimal = default_market_price();
        let small = Decimal::from(1_000_000u64);
        let medium = Decimal::from(2_000_000u64);
        let large = Decimal::from(3_000_000u64);

        // test failure cases for get_target_amount
        assert!(get_target_amount(large, medium, Decimal::zero(), market_price, slope).is_err());

        assert!(get_target_amount(small, medium, large, market_price, slope).is_err());

        assert!(get_target_amount(Decimal::zero(), medium, large, market_price, slope).is_err());

        assert!(get_target_amount(
            Decimal::zero(),
            medium,
            large,
            market_price,
            Decimal::from(2u64)
        )
        .is_err());

        assert_eq!(
            get_target_amount(large, medium, small, market_price, Decimal::zero()).unwrap(),
            Decimal::from(100_000_000u64)
        );

        // lower slope, target amount closer to fair amount
        assert_eq!(
            get_target_amount(
                large,
                medium,
                small,
                market_price,
                slope // 0.1
            )
            .unwrap(),
            Decimal::from(135_000_000u64)
        );

        assert_eq!(
            get_target_amount(
                large,
                medium,
                small,
                market_price,
                Decimal::from_scaled_val(10_000_000_000_u128) // 0.01
            )
            .unwrap(),
            Decimal::from(103_500_000u64)
        );

        assert_eq!(
            get_target_amount(
                large,
                medium,
                small,
                market_price,
                Decimal::from_scaled_val(1_000_000_000_u128) // 0.001
            )
            .unwrap(),
            Decimal::from(100_350_000u64)
        );

        assert_eq!(
            get_target_amount(
                large,
                medium,
                small,
                market_price,
                Decimal::from_scaled_val(100_000_000_u128) // 0.0001
            )
            .unwrap(),
            Decimal::from(100_035_000u64)
        );
    }

    #[test]
    fn test_get_target_amount_stable() {
        let slope: Decimal = Decimal::from_scaled_val(10_000_000_u128); // 0.00001
        let market_price: Decimal = Decimal::one();
        let current_reserve = Decimal::from(1_000_000u64);
        let medium = Decimal::from(3_000_000u64);
        let large = Decimal::from(4_000_000u64);

        assert_eq!(
            get_target_amount(
                medium,
                current_reserve
                    .try_add(Decimal::from(1_000_000u64))
                    .unwrap(),
                current_reserve,
                market_price,
                slope
            )
            .unwrap(),
            Decimal::from(1_000_035u64)
        );

        assert_eq!(
            get_target_amount(
                large,
                current_reserve
                    .try_add(Decimal::from(1_000_000u64))
                    .unwrap(),
                current_reserve,
                market_price,
                slope
            )
            .unwrap(),
            Decimal::from(1_000_070u64)
        );

        assert_eq!(
            get_target_amount(
                large,
                current_reserve
                    .try_add(Decimal::from(3_000_000u64))
                    .unwrap(),
                current_reserve,
                market_price,
                slope
            )
            .unwrap(),
            Decimal::from(3_000_090u64)
        );
    }
}
