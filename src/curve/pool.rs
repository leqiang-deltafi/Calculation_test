//! Intelligent Market Maker V1
use super::*;
use crate::{
    error::SwapError,
    math::{Decimal, TryAdd, TryDiv, TryMul, TrySub},
    state::{pack_decimal, unpack_decimal},
};

use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::{
    entrypoint::ProgramResult,
    program_error::ProgramError,
    program_pack::{Pack, Sealed},
};

use std::{
    cmp::Ordering,
    convert::{TryFrom, TryInto},
};

use num::pow::checked_pow;

#[cfg(feature = "fuzz")]
use arbitrary::Arbitrary;

/// Multiplier status enum
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Debug, Hash)]
pub enum Multiplier {
    /// multiplier = 1
    One,
    /// multiplier > 1
    AboveOne,
    /// multiplier < 1
    BelowOne,
}

impl Default for Multiplier {
    fn default() -> Self {
        Self::One
    }
}

impl TryFrom<u8> for Multiplier {
    type Error = ProgramError;

    fn try_from(multiplier: u8) -> Result<Self, Self::Error> {
        match multiplier {
            0 => Ok(Multiplier::One),
            1 => Ok(Multiplier::AboveOne),
            2 => Ok(Multiplier::BelowOne),
            _ => Err(ProgramError::InvalidAccountData),
        }
    }
}

/// PoolState struct
#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PoolState {
    /// market price
    pub market_price: Decimal,
    /// slope
    pub slope: Decimal,
    /// base token reserve
    pub base_reserve: Decimal,
    /// quote token reserve
    pub quote_reserve: Decimal,
    /// base token regression target
    pub base_target: Decimal,
    /// quote token regression target
    pub quote_target: Decimal,
    /// supply
    pub total_supply: u64,
    /// Multiplier status
    pub multiplier: Multiplier,
    /// last valid market price
    pub last_market_price: Decimal,
    /// last valid market price slot
    pub last_valid_market_price_slot: u64,
}

/// Initialize pool state
pub struct InitPoolStateParams {
    /// market price
    pub market_price: Decimal,
    /// slope
    pub slope: Decimal,
    /// base token reserve
    pub base_reserve: Decimal,
    /// quote token reserve
    pub quote_reserve: Decimal,
    /// total supply
    pub total_supply: u64,
    /// last valid pyth price
    pub last_market_price: Decimal,
    /// last valid pyth price slot
    pub last_valid_market_price_slot: u64,
}

/// Swap direction
#[cfg_attr(feature = "fuzz", derive(Arbitrary))]
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SwapDirection {
    /// sell base
    SellBase,
    /// sell quote
    SellQuote,
}

impl PoolState {
    /// Create new pool state
    pub fn new(params: InitPoolStateParams) -> Self {
        let mut state = Self::default();
        Self::init(&mut state, params);
        state
    }

    /// Init pool state
    pub fn init(&mut self, params: InitPoolStateParams) {
        self.market_price = params.market_price;
        self.slope = params.slope;
        self.base_reserve = params.base_reserve;
        self.quote_reserve = params.quote_reserve;
        self.total_supply = params.total_supply;
        self.last_market_price = params.last_market_price;
        self.last_valid_market_price_slot = params.last_valid_market_price_slot;
    }

    /// Adjust pool token target.
    ///
    /// # Return value
    ///
    /// adjusted token target.
    pub fn adjust_target(&mut self) -> ProgramResult {
        if self.base_reserve == Decimal::zero() || self.quote_reserve == Decimal::zero() {
            self.multiplier = Multiplier::One;
            self.base_target = Decimal::zero();
            self.quote_target = Decimal::zero();

            return Ok(());
        }

        match self
            .quote_reserve
            .try_div(self.base_reserve)?
            .cmp(&self.market_price)
        {
            Ordering::Greater => {
                self.multiplier = Multiplier::AboveOne;
                self.quote_target = self
                    .base_reserve
                    .try_mul(self.market_price)?
                    .try_add(self.quote_reserve)?
                    .try_div(2)?;
                self.base_target = get_target_reserve(
                    self.base_reserve,
                    self.quote_reserve.try_sub(self.quote_target)?,
                    self.market_price.reciprocal()?,
                    self.slope,
                )?
            }
            Ordering::Less => {
                self.multiplier = Multiplier::BelowOne;
                self.base_target = self
                    .quote_reserve
                    .try_div(self.market_price)?
                    .try_add(self.base_reserve)?
                    .try_div(2)?;
                self.quote_target = get_target_reserve(
                    self.quote_reserve,
                    self.base_reserve.try_sub(self.base_target)?,
                    self.market_price,
                    self.slope,
                )?
            }
            Ordering::Equal => {
                self.multiplier = Multiplier::One;
                self.base_target = self.base_reserve;
                self.quote_target = self.quote_reserve;
            }
        }
        Ok(())
    }

    /// Update market price
    pub fn set_market_price(
        &mut self,
        base_decimals: u8,
        quote_decimals: u8,
        market_price: Decimal,
    ) -> ProgramResult {
        self.market_price = match base_decimals.cmp(&quote_decimals) {
            Ordering::Greater => market_price.try_div(
                checked_pow(10u64, (base_decimals - quote_decimals) as usize)
                    .ok_or(SwapError::CalculationFailure)?,
            ),
            Ordering::Less => market_price.try_mul(
                checked_pow(10u64, (quote_decimals - base_decimals) as usize)
                    .ok_or(SwapError::CalculationFailure)?,
            ),
            Ordering::Equal => Ok(market_price),
        }?;
        self.adjust_target()
    }

    /// Update pool state to sell base token
    pub fn sell_base_token(&mut self, base_input: u64, quote_input: u64) -> ProgramResult {
        self.base_reserve = self.base_reserve.try_add(Decimal::from(base_input))?;
        self.quote_reserve = self.quote_reserve.try_sub(Decimal::from(quote_input))?;
        self.adjust_target()
    }

    /// Update pool state to sell quote token
    pub fn sell_quote_token(&mut self, base_input: u64, quote_input: u64) -> ProgramResult {
        self.base_reserve = self.base_reserve.try_sub(Decimal::from(base_input))?;
        self.quote_reserve = self.quote_reserve.try_add(Decimal::from(quote_input))?;
        self.adjust_target()
    }

    /// Update pool state to swap
    pub fn swap(
        &mut self,
        amount_in: u64,
        amount_out: u64,
        swap_direction: SwapDirection,
    ) -> ProgramResult {
        match swap_direction {
            SwapDirection::SellBase => self.sell_base_token(amount_in, amount_out),
            SwapDirection::SellQuote => self.sell_quote_token(amount_out, amount_in),
        }
    }

    /// Sell base token for quote token with multiplier input.
    ///
    /// # Arguments
    ///
    /// * base_amount - base amount to sell.
    /// * multiplier - multiplier status.
    ///
    /// # Return value
    ///
    /// purchased quote token amount.
    fn sell_base_token_with_multiplier(
        &self,
        base_amount: Decimal,
        multiplier: Multiplier,
    ) -> Result<Decimal, ProgramError> {
        if self.slope > Decimal::one() {
            return Err(SwapError::InvalidSlope.into());
        }

        match multiplier {
            Multiplier::One => get_target_amount_reverse_direction(
                self.quote_target,
                self.quote_target,
                base_amount,
                self.market_price,
                self.slope,
            ),
            Multiplier::AboveOne => get_target_amount(
                self.base_target,
                self.base_reserve.try_add(base_amount)?,
                self.base_reserve,
                self.market_price,
                self.slope,
            ),
            Multiplier::BelowOne => get_target_amount_reverse_direction(
                self.quote_target,
                self.quote_reserve,
                base_amount,
                self.market_price,
                self.slope,
            ),
        }
    }

    /// Sell base token for quote token.
    ///
    /// # Arguments
    ///
    /// * base_amount - base amount to sell.
    ///
    /// # Return value
    ///
    /// purchased quote token amount, updated multiplier.
    pub fn quote_out_amount(&self, base_amount: u64) -> Result<u64, ProgramError> {
        match self.multiplier {
            Multiplier::One => {
                self.sell_base_token_with_multiplier(base_amount.into(), Multiplier::One)?
            }
            Multiplier::BelowOne => {
                self.sell_base_token_with_multiplier(base_amount.into(), Multiplier::BelowOne)?
            }
            Multiplier::AboveOne => {
                let back_to_one_pay_base = self.base_target.try_sub(self.base_reserve)?;
                let back_to_one_receive_quote = self.quote_reserve.try_sub(self.quote_target)?;

                match back_to_one_pay_base.cmp(&Decimal::from(base_amount)) {
                    Ordering::Greater => self
                        .sell_base_token_with_multiplier(base_amount.into(), Multiplier::AboveOne)?
                        .min(back_to_one_receive_quote),
                    Ordering::Equal => back_to_one_receive_quote,
                    Ordering::Less => self
                        .sell_base_token_with_multiplier(
                            Decimal::from(base_amount).try_sub(back_to_one_pay_base)?,
                            Multiplier::One,
                        )?
                        .try_add(back_to_one_receive_quote)?,
                }
            }
        }
        .try_floor_u64()
    }

    /// Sell quote token for base token with multiplier input.
    ///
    /// # Arguments
    ///
    /// * quote_amount - quote amount to sell.
    /// * multiplier - multiplier status.
    ///
    /// # Return value
    ///
    /// purchased base token amount.
    fn sell_quote_token_with_multiplier(
        &self,
        quote_amount: Decimal,
        multiplier: Multiplier,
    ) -> Result<Decimal, ProgramError> {
        if self.slope > Decimal::one() {
            return Err(SwapError::InvalidSlope.into());
        }

        match multiplier {
            Multiplier::One => get_target_amount_reverse_direction(
                self.base_target,
                self.base_target,
                quote_amount,
                self.market_price.reciprocal()?,
                self.slope,
            ),
            Multiplier::AboveOne => get_target_amount_reverse_direction(
                self.base_target,
                self.base_reserve,
                quote_amount,
                self.market_price.reciprocal()?,
                self.slope,
            ),
            Multiplier::BelowOne => get_target_amount(
                self.quote_target,
                self.quote_reserve.try_add(quote_amount)?,
                self.quote_reserve,
                self.market_price.reciprocal()?,
                self.slope,
            ),
        }
    }

    /// Sell quote token for base token.
    ///
    /// # Arguments
    ///
    /// * quote_amount - quote amount to sell.
    ///
    /// # Return value
    ///
    /// purchased base token amount, updated multiplier.
    pub fn base_out_amount(&self, quote_amount: u64) -> Result<u64, ProgramError> {
        match self.multiplier {
            Multiplier::One => {
                self.sell_quote_token_with_multiplier(quote_amount.into(), Multiplier::One)?
            }
            Multiplier::AboveOne => {
                self.sell_quote_token_with_multiplier(quote_amount.into(), Multiplier::AboveOne)?
            }
            Multiplier::BelowOne => {
                let back_to_one_pay_quote = self.quote_target.try_sub(self.quote_reserve)?;
                let back_to_one_receive_base = self.base_reserve.try_sub(self.base_target)?;

                match back_to_one_pay_quote.cmp(&Decimal::from(quote_amount)) {
                    Ordering::Greater => self
                        .sell_quote_token_with_multiplier(
                            quote_amount.into(),
                            Multiplier::BelowOne,
                        )?
                        .min(back_to_one_receive_base),
                    Ordering::Equal => back_to_one_receive_base,
                    Ordering::Less => self
                        .sell_quote_token_with_multiplier(
                            Decimal::from(quote_amount).try_sub(back_to_one_pay_quote)?,
                            Multiplier::One,
                        )?
                        .try_add(back_to_one_receive_base)?,
                }
            }
        }
        .try_floor_u64()
    }

    /// Return out amount
    pub fn get_out_amount(
        &self,
        amount_in: u64,
        swap_direction: SwapDirection,
    ) -> Result<u64, ProgramError> {
        match swap_direction {
            SwapDirection::SellBase => self.quote_out_amount(amount_in),
            SwapDirection::SellQuote => self.base_out_amount(amount_in),
        }
    }

    /// Buy shares [round down]: deposit and calculate shares.
    ///
    /// # Arguments
    ///
    /// * base_balance - base amount to sell.
    /// * quote_balance - quote amount to sell.
    /// * total_supply - total shares amount.
    ///
    /// # Return value
    ///
    /// purchased shares, base token amount, quote token amount.
    pub fn buy_shares(
        &mut self,
        base_input: u64,
        quote_input: u64,
    ) -> Result<(u64, u64, u64), ProgramError> {
        if base_input == 0 || quote_input == 0 {
            return Err(SwapError::InsufficientFunds.into());
        }

        let (shares, base_output, quote_output) = if self.total_supply == 0 {
            // Use the base input amount to determine the initial share.
            let shares = base_input;
            (shares, base_input, quote_input)
        } else if self.base_reserve > Decimal::zero() && self.quote_reserve > Decimal::zero() {
            // case 2. normal case
            let base_input_ratio = Decimal::from(base_input).try_div(self.base_reserve)?;
            let quote_input_ratio = Decimal::from(quote_input).try_div(self.quote_reserve)?;
            let mint_ratio = base_input_ratio.min(quote_input_ratio);
            let shares = mint_ratio.try_mul(self.total_supply)?;
            if base_input_ratio == mint_ratio {
                (
                    shares.try_floor_u64()?,
                    base_input,
                    mint_ratio.try_mul(self.quote_reserve)?.try_floor_u64()?,
                )
            } else {
                (
                    shares.try_floor_u64()?,
                    mint_ratio.try_mul(self.base_reserve)?.try_floor_u64()?,
                    quote_input,
                )
            }
        } else {
            return Err(SwapError::IncorrectMint.into());
        };

        // The calculated base/quote amounts shouldn't exceed the output amounts.
        if base_output > base_input || quote_output > quote_input {
            return Err(SwapError::CalculationFailure.into());
        }

        self.base_reserve = self.base_reserve.try_add(Decimal::from(base_output))?;
        self.quote_reserve = self.quote_reserve.try_add(Decimal::from(quote_output))?;
        self.total_supply = self
            .total_supply
            .checked_add(shares)
            .ok_or(SwapError::CalculationFailure)?;
        self.adjust_target()?;
        Ok((shares, base_output, quote_output))
    }

    /// Sell shares [round down]: withdraw shares and calculate the withdrawn amount.
    ///
    /// # Arguments
    ///
    /// * share_amount - share amount to sell.
    /// * base_min_amount - base min amount.
    /// * quote_min_amount - quote min amount.
    /// * total_supply - total shares amount.
    ///
    /// # Return value
    ///
    /// base amount, quote amount.
    pub fn sell_shares(
        &mut self,
        share_amount: u64,
        base_min_amount: u64,
        quote_min_amount: u64,
    ) -> Result<(u64, u64), ProgramError> {
        if self.total_supply < share_amount {
            return Err(SwapError::InsufficientFunds.into());
        }

        let base_balance = self.base_reserve;
        let quote_balance = self.quote_reserve;

        let base_amount = base_balance
            .try_mul(share_amount)?
            .try_div(self.total_supply)?
            .try_floor_u64()?;
        let quote_amount = quote_balance
            .try_mul(share_amount)?
            .try_div(self.total_supply)?
            .try_floor_u64()?;

        if base_amount < base_min_amount || quote_amount < quote_min_amount {
            return Err(SwapError::WithdrawNotEnough.into());
        }

        self.base_reserve = self.base_reserve.try_sub(Decimal::from(base_amount))?;
        self.quote_reserve = self.quote_reserve.try_sub(Decimal::from(quote_amount))?;

        self.total_supply = self
            .total_supply
            .checked_sub(share_amount)
            .ok_or(SwapError::CalculationFailure)?;
        self.adjust_target()?;
        Ok((base_amount, quote_amount))
    }

    /// Total value locked in the pool
    pub fn tvl(&self, base_price: Decimal, quote_price: Decimal) -> Result<Decimal, ProgramError> {
        self.base_reserve
            .try_mul(base_price)?
            .try_add(self.quote_reserve.try_mul(quote_price)?)
    }

    /// Check and update last market price and slot
    pub fn check_and_update_market_price_and_slot(
        &mut self,
        new_market_price: Decimal,
        new_market_last_slot: u64,
    ) -> ProgramResult {
        const SHORT_PYTH_PRICE_SLOTS: u64 = 25; // 10s

        // check if this slot is with 10s from last swap and the price diff is more than 1%
        if new_market_last_slot - self.last_valid_market_price_slot < SHORT_PYTH_PRICE_SLOTS {
            let price_diff = if new_market_price > self.last_market_price {
                new_market_price.try_sub(self.last_market_price)
            } else {
                self.last_market_price.try_sub(new_market_price)
            }?;

            if price_diff.try_mul(Decimal::from(100u64))? > self.last_market_price {
                return Err(SwapError::UnstableMarketPrice.into());
            }
        }

        self.last_market_price = new_market_price;
        self.last_valid_market_price_slot = new_market_last_slot;

        Ok(())
    }

    /// Collect trade trade fee.
    pub fn collect_trade_fee(&mut self, base_fee: u64, quote_fee: u64) -> ProgramResult {
        self.base_reserve = self.base_reserve.try_add(Decimal::from(base_fee))?;
        self.quote_reserve = self.quote_reserve.try_add(Decimal::from(quote_fee))?;
        Ok(())
    }

    /// Check the reserve amount match the token amount in the pool.
    pub fn check_reserve_amount(
        &self,
        base_token_amount: u64,
        quote_token_amount: u64,
    ) -> ProgramResult {
        if self.base_reserve > Decimal::from(base_token_amount)
            || self.quote_reserve > Decimal::from(quote_token_amount)
        {
            return Err(SwapError::InconsistentPoolState.into());
        }

        if self.base_reserve == Decimal::zero() || self.quote_reserve == Decimal::zero() {
            return Err(SwapError::InsufficientFunds.into());
        }

        Ok(())
    }

    /// check the mint supply matches the total supply in the pool.
    pub fn check_mint_supply(&self, mint_supply: u64) -> ProgramResult {
        if mint_supply > self.total_supply {
            return Err(SwapError::InvalidSupply.into());
        }
        Ok(())
    }
}

impl Sealed for PoolState {}

/// PoolState packed size
pub const POOL_STATE_SIZE: usize = 129; // 16 + 16 + 16 + 16 + 16 + 16 + 8 + 1 + 16 + 8
impl Pack for PoolState {
    const LEN: usize = POOL_STATE_SIZE;
    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, POOL_STATE_SIZE];
        let (
            market_price,
            slope,
            base_reserve,
            quote_reserve,
            base_target,
            quote_target,
            total_supply,
            multiplier,
            last_market_price,
            last_valid_market_price_slot,
        ) = mut_array_refs![output, 16, 16, 16, 16, 16, 16, 8, 1, 16, 8];
        pack_decimal(self.market_price, market_price);
        pack_decimal(self.slope, slope);
        pack_decimal(self.base_reserve, base_reserve);
        pack_decimal(self.quote_reserve, quote_reserve);
        pack_decimal(self.base_target, base_target);
        pack_decimal(self.quote_target, quote_target);
        *total_supply = self.total_supply.to_le_bytes();
        multiplier[0] = self.multiplier as u8;
        pack_decimal(self.last_market_price, last_market_price);
        *last_valid_market_price_slot = self.last_valid_market_price_slot.to_le_bytes();
    }

    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, POOL_STATE_SIZE];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            market_price,
            slope,
            base_reserve,
            quote_reserve,
            base_target,
            quote_target,
            total_supply,
            multiplier,
            last_market_price,
            last_valid_market_price_slot,
        ) = array_refs![input, 16, 16, 16, 16, 16, 16, 8, 1, 16, 8];
        Ok(Self {
            market_price: unpack_decimal(market_price),
            slope: unpack_decimal(slope),
            base_reserve: unpack_decimal(base_reserve),
            quote_reserve: unpack_decimal(quote_reserve),
            base_target: unpack_decimal(base_target),
            quote_target: unpack_decimal(quote_target),
            total_supply: u64::from_le_bytes(*total_supply),
            multiplier: multiplier[0].try_into()?,
            last_market_price: unpack_decimal(last_market_price),
            last_valid_market_price_slot: u64::from_le_bytes(*last_valid_market_price_slot),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_mint_supply() {
        let pool_state = PoolState::new(InitPoolStateParams {
            market_price: default_market_price(),
            slope: default_slope(),
            base_reserve: Decimal::from(100u64),
            quote_reserve: Decimal::from(100u64),
            total_supply: 1000,
            last_market_price: default_market_price(),
            last_valid_market_price_slot: 0,
        });

        assert_eq!(pool_state.check_mint_supply(1000), Ok(()));

        // It is ok to have mint supply less than total_supply in pool, because people can burn token by themselves.
        assert_eq!(pool_state.check_mint_supply(999), Ok(()));

        // The mint supply should not be not be larger than total_supply in pool.
        assert_eq!(
            pool_state.check_mint_supply(1001),
            Err(SwapError::InvalidSupply.into())
        );
    }

    #[test]
    fn test_check_reserve_amount() {
        let mut pool_state = PoolState::new(InitPoolStateParams {
            market_price: default_market_price(),
            slope: default_slope(),
            base_reserve: Decimal::from(100u64),
            quote_reserve: Decimal::from(100u64),
            total_supply: 0,
            last_market_price: default_market_price(),
            last_valid_market_price_slot: 0,
        });

        assert_eq!(pool_state.check_reserve_amount(100u64, 100u64), Ok(()));

        // It is ok to have more balance in token account.
        assert_eq!(pool_state.check_reserve_amount(100u64, 101u64), Ok(()));
        assert_eq!(pool_state.check_reserve_amount(101u64, 100u64), Ok(()));

        assert_eq!(
            pool_state.check_reserve_amount(100u64, 99u64),
            Err(SwapError::InconsistentPoolState.into())
        );

        pool_state.base_reserve = Decimal::zero();
        assert_eq!(
            pool_state.check_reserve_amount(0u64, 100u64),
            Err(SwapError::InsufficientFunds.into())
        );
    }

    #[test]
    fn test_swap() {
        let mut pool_state = PoolState::new(InitPoolStateParams {
            market_price: default_market_price(),
            slope: default_slope(),
            base_reserve: Decimal::zero(),
            quote_reserve: Decimal::zero(),
            total_supply: 0,
            last_market_price: default_market_price(),
            last_valid_market_price_slot: 0,
        });

        assert_eq!(
            pool_state.buy_shares(1_000_000, 100_000_000),
            Ok((1_000_000, 1_000_000, 100_000_000))
        );

        assert_eq!(pool_state.base_reserve.try_floor_u64(), Ok(1_000_000));
        assert_eq!(pool_state.quote_reserve.try_floor_u64(), Ok(100_000_000));

        // User sells base tokens. The pool should have more base and less reserve.
        assert_eq!(pool_state.swap(100, 200, SwapDirection::SellBase), Ok(()));
        assert_eq!(pool_state.base_reserve.try_floor_u64(), Ok(1_000_000 + 100));
        assert_eq!(
            pool_state.quote_reserve.try_floor_u64(),
            Ok(100_000_000 - 200)
        );

        // User sells quote tokens. The pool should have more quote and less base.
        assert_eq!(
            pool_state.swap(1000, 2000, SwapDirection::SellQuote),
            Ok(())
        );
        assert_eq!(
            pool_state.base_reserve.try_floor_u64(),
            Ok(1_000_000 + 100 - 2000)
        );
        assert_eq!(
            pool_state.quote_reserve.try_floor_u64(),
            Ok(100_000_000 - 200 + 1000)
        );
    }

    #[test]
    fn test_buy_shares() {
        let mut pool_state = PoolState::new(InitPoolStateParams {
            market_price: default_market_price(),
            slope: default_slope(),
            base_reserve: Decimal::zero(),
            quote_reserve: Decimal::zero(),
            total_supply: 0,
            last_market_price: default_market_price(),
            last_valid_market_price_slot: 0,
        });

        assert_eq!(
            pool_state.buy_shares(1_000_000, 100_000_000),
            Ok((1_000_000, 1_000_000, 100_000_000))
        );

        assert_eq!(pool_state.total_supply, 1_000_000);

        let base_input = 100;
        let quote_out = pool_state.quote_out_amount(base_input).unwrap();

        assert_eq!(quote_out, 9999);
        pool_state
            .swap(base_input, quote_out, SwapDirection::SellBase)
            .unwrap();

        let quote_input = 10000;
        let base_out = pool_state.base_out_amount(quote_input).unwrap();
        assert_eq!(base_out, 100);
        pool_state
            .swap(quote_input, base_out, SwapDirection::SellQuote)
            .unwrap();

        assert!(pool_state.buy_shares(0u64, 1000).is_err());

        assert_eq!(
            pool_state.buy_shares(10_000, 10_000_000),
            Ok((10000, 10000, 1_000_000))
        );

        assert_eq!(pool_state.total_supply, 1_010_000);
    }

    #[test]
    fn test_buy_shares_2() {
        let mut pool_state = PoolState::new(InitPoolStateParams {
            market_price: default_market_price(),
            slope: default_slope(),
            base_reserve: Decimal::zero(),
            quote_reserve: Decimal::zero(),
            total_supply: 0,
            last_market_price: default_market_price(),
            last_valid_market_price_slot: 0,
        });

        // The initial buy share should always use base token as shares.
        assert_eq!(
            pool_state.buy_shares(1_000_000_000, 100_000_000),
            Ok((1_000_000_000, 1_000_000_000, 100_000_000))
        );
    }

    #[test]
    fn test_sell_shares() {
        let mut pool_state = PoolState::new(InitPoolStateParams {
            market_price: default_market_price(),
            slope: default_slope(),
            base_reserve: Decimal::zero(),
            quote_reserve: Decimal::zero(),
            total_supply: 0,
            last_market_price: default_market_price(),
            last_valid_market_price_slot: 0,
        });

        assert!(pool_state.sell_shares(10, 1, 1).is_err());

        pool_state.buy_shares(1_000_000, 100_000_000).unwrap();

        assert!(pool_state.sell_shares(10, 100, 100).is_err());
        assert_eq!(pool_state.total_supply, 1_000_000);

        if let Ok((base_amount, quote_amount)) = pool_state.sell_shares(10, 1, 1) {
            assert_eq!(base_amount, 10);
            assert_eq!(quote_amount, 1_000);
            assert_eq!(pool_state.total_supply, 999_990);
            assert_eq!(pool_state.base_reserve, Decimal::from(999_990_u64));
            assert_eq!(pool_state.quote_reserve, Decimal::from(99_999_000_u64));
        }
    }

    #[test]
    fn test_tvl() {
        let mut pool_state = PoolState::new(InitPoolStateParams {
            market_price: default_market_price(),
            slope: default_slope(),
            base_reserve: Decimal::zero(),
            quote_reserve: Decimal::zero(),
            total_supply: 0,
            last_market_price: default_market_price(),
            last_valid_market_price_slot: 0,
        });

        pool_state.buy_shares(1_000_000, 100_000_000).unwrap();

        assert_eq!(
            pool_state
                .tvl(Decimal::from(10u64), Decimal::from(100u64))
                .unwrap(),
            Decimal::from(10_010_000_000_u64)
        );
    }

    #[test]
    fn test_check_and_update_market_price_and_slot() {
        let mut pool_state = PoolState::new(InitPoolStateParams {
            market_price: default_market_price(),
            slope: default_slope(),
            base_reserve: Decimal::zero(),
            quote_reserve: Decimal::zero(),
            total_supply: 0,
            last_market_price: Decimal::from(200u64),
            last_valid_market_price_slot: 0,
        });

        assert!(pool_state
            .check_and_update_market_price_and_slot(Decimal::from(100u64), 6u64)
            .is_err());
        assert!(pool_state
            .check_and_update_market_price_and_slot(Decimal::from(1000u64), 26u64)
            .is_ok());
        assert!(pool_state
            .check_and_update_market_price_and_slot(Decimal::from(1001u64), 50u64)
            .is_ok());
    }

    #[test]
    fn test_packing_pool() {
        let pool_state = PoolState::new(InitPoolStateParams {
            market_price: default_market_price(),
            slope: default_slope(),
            base_reserve: Decimal::from(1_000_000u64),
            quote_reserve: Decimal::from(900_000_000u64),
            total_supply: 900_000,
            last_market_price: default_market_price(),
            last_valid_market_price_slot: 0,
        });

        let mut packed = [0u8; PoolState::LEN];
        PoolState::pack_into_slice(&pool_state, &mut packed);
        let unpacked = PoolState::unpack_from_slice(&packed).unwrap();
        assert_eq!(pool_state, unpacked);
    }

    #[test]
    fn test_get_out_amount_balanced_pool() {
        let mut pool_state = PoolState::new(InitPoolStateParams {
            market_price: default_market_price(),
            slope: default_slope(), // 0.1
            base_reserve: Decimal::zero(),
            quote_reserve: Decimal::zero(),
            total_supply: 0,
            last_market_price: default_market_price(), // 100
            last_valid_market_price_slot: 0,
        });

        pool_state.buy_shares(1_000_000, 100_000_000).unwrap();

        // Sell base, below one.
        pool_state.multiplier = Multiplier::BelowOne;
        // case 1: a trade with 50% of the reserve with high price.
        let below_one_base = pool_state
            .get_out_amount(500_000u64, SwapDirection::SellBase)
            .unwrap();
        assert_eq!(below_one_base, 46_065_533_u64);

        // case 2: a trade with 5% of the reserve with lower price.
        let below_one_base = pool_state
            .get_out_amount(50_000_u64, SwapDirection::SellBase)
            .unwrap();
        assert_eq!(below_one_base, 4_973_964_u64);

        // case 3: a trade with 0.5% of the reserve with lower price.
        let above_one_base = pool_state
            .get_out_amount(5_000_u64, SwapDirection::SellBase)
            .unwrap();
        assert_eq!(above_one_base, 499_748_u64);

        // Sell base, above one.
        pool_state.multiplier = Multiplier::AboveOne;
        // case 1: a trade with 50% of the reserve with high price.
        let above_one_base = pool_state
            .get_out_amount(500_000_u64, SwapDirection::SellBase)
            .unwrap();

        assert_eq!(above_one_base, 46_065_533_u64);

        // case 2: a trade with 5% of the reserve with high price.
        let above_one_base = pool_state
            .get_out_amount(50_000_u64, SwapDirection::SellBase)
            .unwrap();

        assert_eq!(above_one_base, 4_973_964_u64);

        // case 3: a trade with 0.5% of the reserve with high price.
        let above_one_base = pool_state
            .get_out_amount(5_000_u64, SwapDirection::SellBase)
            .unwrap();

        assert_eq!(above_one_base, 499_748_u64);

        // Sell base, equal one.
        pool_state.multiplier = Multiplier::One;
        // case 1: a trade with 50% of the reserve with high price.
        let equal_one_base = pool_state
            .get_out_amount(500_000_u64, SwapDirection::SellBase)
            .unwrap();

        assert_eq!(equal_one_base, 46_065_533_u64);

        // case 2: a trade with 5% of the reserve with high price.
        let equal_one_base = pool_state
            .get_out_amount(50_000_u64, SwapDirection::SellBase)
            .unwrap();

        assert_eq!(equal_one_base, 4_973_964_u64);

        // case 3: a trade with 0.5% of the reserve with high price.
        let equal_one_base = pool_state
            .get_out_amount(5_000_u64, SwapDirection::SellBase)
            .unwrap();

        assert_eq!(equal_one_base, 499_748_u64);

        // Sell quote, below one.
        pool_state.multiplier = Multiplier::BelowOne;
        // case 1: a trade with 50% of the reserve with high price.
        let below_one_quote = pool_state
            .get_out_amount(50_000_000u64, SwapDirection::SellQuote)
            .unwrap();
        assert_eq!(below_one_quote, 460_655_u64);

        // case 2: a trade with 5% of the reserve with lower price.
        let below_one_quote = pool_state
            .get_out_amount(5_000_000_u64, SwapDirection::SellQuote)
            .unwrap();
        assert_eq!(below_one_quote, 49_739_u64);

        // case 3: a trade with 0.5% of the reserve with lower price.
        let below_one_quote = pool_state
            .get_out_amount(500_000_u64, SwapDirection::SellQuote)
            .unwrap();
        assert_eq!(below_one_quote, 4_997_u64);

        // Sell quote, above one.
        pool_state.multiplier = Multiplier::AboveOne;
        // case 1: a trade with 50% of the reserve with high price.
        let below_one_quote = pool_state
            .get_out_amount(50_000_000u64, SwapDirection::SellQuote)
            .unwrap();
        assert_eq!(below_one_quote, 460_655_u64);

        // case 2: a trade with 5% of the reserve with lower price.
        let below_one_quote = pool_state
            .get_out_amount(5_000_000_u64, SwapDirection::SellQuote)
            .unwrap();
        assert_eq!(below_one_quote, 49_739_u64);

        // case 3: a trade with 0.5% of the reserve with lower price.
        let below_one_quote = pool_state
            .get_out_amount(500_000_u64, SwapDirection::SellQuote)
            .unwrap();
        assert_eq!(below_one_quote, 4_997_u64);

        // Sell quote, equal one.
        pool_state.multiplier = Multiplier::One;
        // case 1: a trade with 50% of the reserve with high price.
        let below_one_quote = pool_state
            .get_out_amount(50_000_000u64, SwapDirection::SellQuote)
            .unwrap();
        assert_eq!(below_one_quote, 460_655_u64);

        // case 2: a trade with 5% of the reserve with lower price.
        let below_one_quote = pool_state
            .get_out_amount(5_000_000_u64, SwapDirection::SellQuote)
            .unwrap();
        assert_eq!(below_one_quote, 49_739_u64);

        // case 3: a trade with 0.5% of the reserve with lower price.
        let below_one_quote = pool_state
            .get_out_amount(500_000_u64, SwapDirection::SellQuote)
            .unwrap();
        assert_eq!(below_one_quote, 4_997_u64);
    }

    #[test]
    fn test_get_out_amount_logic_flow() {
        let mut pool_state = PoolState::new(InitPoolStateParams {
            market_price: default_market_price(),
            slope: default_slope(),
            base_reserve: Decimal::from(1_000_000u64),
            quote_reserve: Decimal::from(900_000_000u64),
            total_supply: 900_000,
            last_market_price: default_market_price(),
            last_valid_market_price_slot: 0,
        });

        pool_state.base_target = Decimal::from(4_062_255u64);
        pool_state.quote_target = Decimal::from(500_000_000u64);

        let amount_in: u64 = 500_000u64;

        pool_state.multiplier = Multiplier::BelowOne;
        let below_one_base = pool_state
            .get_out_amount(amount_in, SwapDirection::SellBase)
            .unwrap();
        let below_one_base_expect = get_target_amount_reverse_direction(
            pool_state.quote_target,
            pool_state.quote_reserve,
            Decimal::from(amount_in),
            pool_state.market_price,
            pool_state.slope,
        )
        .unwrap()
        .try_floor_u64()
        .unwrap();

        assert_eq!(below_one_base, below_one_base_expect);

        pool_state.multiplier = Multiplier::AboveOne;
        let above_one_quote = pool_state
            .get_out_amount(amount_in, SwapDirection::SellQuote)
            .unwrap();
        let above_one_quote_expect = get_target_amount_reverse_direction(
            pool_state.base_target,
            pool_state.base_reserve,
            Decimal::from(amount_in),
            pool_state.market_price.reciprocal().unwrap(),
            pool_state.slope,
        )
        .unwrap()
        .try_floor_u64()
        .unwrap();

        assert_eq!(above_one_quote, above_one_quote_expect);

        pool_state.multiplier = Multiplier::One;
        let one_base = pool_state
            .get_out_amount(amount_in, SwapDirection::SellBase)
            .unwrap();
        let one_base_expect = get_target_amount_reverse_direction(
            pool_state.quote_target,
            pool_state.quote_target,
            Decimal::from(amount_in),
            pool_state.market_price,
            pool_state.slope,
        )
        .unwrap()
        .try_floor_u64()
        .unwrap();

        assert_eq!(one_base, one_base_expect);

        pool_state.multiplier = Multiplier::One;
        let one_quote = pool_state
            .get_out_amount(amount_in, SwapDirection::SellQuote)
            .unwrap();
        let one_quote_expect = get_target_amount_reverse_direction(
            pool_state.base_target,
            pool_state.base_target,
            Decimal::from(amount_in),
            pool_state.market_price.reciprocal().unwrap(),
            pool_state.slope,
        )
        .unwrap()
        .try_floor_u64()
        .unwrap();

        assert_eq!(one_quote, one_quote_expect);

        pool_state.multiplier = Multiplier::AboveOne;
        let above_one_base_greater = pool_state
            .get_out_amount(amount_in, SwapDirection::SellBase)
            .unwrap();
        let above_one_base_greater_expect = get_target_amount(
            pool_state.base_target,
            pool_state
                .base_reserve
                .try_add(Decimal::from(amount_in))
                .unwrap(),
            pool_state.base_reserve,
            pool_state.market_price,
            pool_state.slope,
        )
        .unwrap()
        .min(Decimal::from(400_000_000u64))
        .try_floor_u64()
        .unwrap();

        assert_eq!(above_one_base_greater, above_one_base_greater_expect);

        let amount_in_base_equal: u64 = 3_062_255u64;
        let above_one_base_equal = pool_state
            .get_out_amount(amount_in_base_equal, SwapDirection::SellBase)
            .unwrap();
        let above_one_base_equal_expect: u64 = 400_000_000u64;

        assert_eq!(above_one_base_equal, above_one_base_equal_expect);

        let amount_in_base_less: u64 = 5_000_000u64;
        let above_one_base_less = pool_state
            .get_out_amount(amount_in_base_less, SwapDirection::SellBase)
            .unwrap();
        let above_one_base_less_expect: u64 = get_target_amount_reverse_direction(
            pool_state.quote_target,
            pool_state.quote_target,
            Decimal::from(1937745u64),
            pool_state.market_price,
            pool_state.slope,
        )
        .unwrap()
        .try_add(Decimal::from(400_000_000u64))
        .unwrap()
        .try_floor_u64()
        .unwrap();

        assert_eq!(above_one_base_less, above_one_base_less_expect);

        pool_state.base_reserve = Decimal::from(6_000_000u64);
        pool_state.quote_reserve = Decimal::from(200_000_000u64);
        pool_state.multiplier = Multiplier::BelowOne;

        let below_one_quote_greater = pool_state
            .get_out_amount(amount_in, SwapDirection::SellQuote)
            .unwrap();
        let below_one_quote_greater_expect = get_target_amount(
            pool_state.quote_target,
            pool_state
                .quote_reserve
                .try_add(Decimal::from(amount_in))
                .unwrap(),
            pool_state.quote_reserve,
            pool_state.market_price.reciprocal().unwrap(),
            pool_state.slope,
        )
        .unwrap()
        .min(Decimal::from(1_937_745u64))
        .try_floor_u64()
        .unwrap();

        assert_eq!(below_one_quote_greater, below_one_quote_greater_expect);

        let amount_in_quote_equal: u64 = 300_000_000u64;
        let below_one_quote_equal = pool_state
            .get_out_amount(amount_in_quote_equal, SwapDirection::SellQuote)
            .unwrap();
        let below_one_quote_equal_expect: u64 = 1_937_745u64;

        assert_eq!(below_one_quote_equal, below_one_quote_equal_expect);

        let amount_in_quote_less: u64 = 350_000_000u64;
        let below_one_quote_less = pool_state
            .get_out_amount(amount_in_quote_less, SwapDirection::SellQuote)
            .unwrap();
        let below_one_quote_less_expect = get_target_amount_reverse_direction(
            pool_state.base_target,
            pool_state.base_target,
            Decimal::from(50_000_000u64),
            pool_state.market_price.reciprocal().unwrap(),
            pool_state.slope,
        )
        .unwrap()
        .try_add(Decimal::from(1_937_745u64))
        .unwrap()
        .try_floor_u64()
        .unwrap();

        assert_eq!(below_one_quote_less, below_one_quote_less_expect);
    }

    #[test]
    fn test_get_out_amount_imbalanced_pool() {
        let mut pool_state = PoolState::new(InitPoolStateParams {
            market_price: default_market_price(),
            slope: default_slope(), // 0.1
            base_reserve: Decimal::zero(),
            quote_reserve: Decimal::zero(),
            total_supply: 0,
            last_market_price: default_market_price(), // 100
            last_valid_market_price_slot: 0,
        });

        pool_state.buy_shares(1_000_000, 10_000_000).unwrap();

        // Sell base, below one.
        assert_eq!(pool_state.multiplier, Multiplier::BelowOne);
        // case 1: a trade with 50% of the reserve with high price.
        let below_one_base = pool_state
            .get_out_amount(500_000u64, SwapDirection::SellBase)
            .unwrap();
        assert_eq!(below_one_base, 6_963_810_u64);

        // case 2: a trade with 5% of the reserve with lower price.
        let below_one_base = pool_state
            .get_out_amount(50_000_u64, SwapDirection::SellBase)
            .unwrap();
        assert_eq!(below_one_base, 1_580_019_u64);

        // case 3: a trade with 0.5% of the reserve with lower price.
        let above_one_base = pool_state
            .get_out_amount(5_000_u64, SwapDirection::SellBase)
            .unwrap();
        assert_eq!(above_one_base, 176_001_u64);

        // case 4: a very small trade.
        let above_one_base = pool_state
            .get_out_amount(10_u64, SwapDirection::SellBase)
            .unwrap();
        assert_eq!(above_one_base, 355_u64);

        // Sell base, below one.
        pool_state.slope = Decimal::from_scaled_val(1_000_000_000_u128); // 0.001
        pool_state.multiplier = Multiplier::BelowOne;
        // case 1: a trade with 50% of the reserve with high price.
        let below_one_base = pool_state
            .get_out_amount(500_000u64, SwapDirection::SellBase)
            .unwrap();
        assert_eq!(below_one_base, 9_952_625_u64);

        // case 2: a trade with 5% of the reserve with lower price.
        let below_one_base = pool_state
            .get_out_amount(50_000_u64, SwapDirection::SellBase)
            .unwrap();
        assert_eq!(below_one_base, 4_826_914_u64);

        // case 3: a trade with 0.5% of the reserve with lower price.
        let above_one_base = pool_state
            .get_out_amount(5_000_u64, SwapDirection::SellBase)
            .unwrap();
        assert_eq!(above_one_base, 490_652_u64);

        // case 4: a very small trade.
        let above_one_base = pool_state
            .get_out_amount(10_u64, SwapDirection::SellBase)
            .unwrap();
        assert_eq!(above_one_base, 982_u64);

        // Sell base, below one.
        pool_state.slope = Decimal::from_scaled_val(10_000_000_u128); // 0.00001
        pool_state.multiplier = Multiplier::BelowOne;
        // case 1: a trade with 50% of the reserve with high price.
        let below_one_base = pool_state
            .get_out_amount(500_000u64, SwapDirection::SellBase)
            .unwrap();
        assert_eq!(below_one_base, 9_999_523_u64);
    }

    #[test]
    fn test_get_out_amount_low_slope_imbalanced_pool() {
        // With slope = 0.0001, even 10x imbalanced, it works well.
        let mut pool_state = PoolState::new(InitPoolStateParams {
            market_price: default_market_price(),
            slope: Decimal::from_scaled_val(100_000_000_u128), // 0.0001
            base_reserve: Decimal::zero(),
            quote_reserve: Decimal::zero(),
            total_supply: 0,
            last_market_price: default_market_price(), // 100
            last_valid_market_price_slot: 0,
        });

        pool_state.buy_shares(1_000_000, 10_000_000).unwrap();

        // Sell base, below one.
        // quote_reserve / base_reserve < market_price
        pool_state
            .set_market_price(6u8, 6u8, Decimal::from(15u64))
            .unwrap();
        assert_eq!(pool_state.multiplier, Multiplier::BelowOne);
        // case 1: a trade with 50% of the reserve with high price.
        let below_one_base = pool_state
            .get_out_amount(500_000u64, SwapDirection::SellBase)
            .unwrap();
        assert_eq!(below_one_base, 7_496_072_u64);

        // 10%
        let below_one_base = pool_state
            .get_out_amount(100_000u64, SwapDirection::SellBase)
            .unwrap();
        assert_eq!(below_one_base, 1499874_u64);

        // case 2: a trade with 5% of the reserve with lower price.
        let below_one_base = pool_state
            .get_out_amount(50_000_u64, SwapDirection::SellBase)
            .unwrap();
        assert_eq!(below_one_base, 749_948_u64);

        // case 3: a trade with 0.5% of the reserve with lower price.
        let above_one_base = pool_state
            .get_out_amount(5_000_u64, SwapDirection::SellBase)
            .unwrap();
        assert_eq!(above_one_base, 74_996_u64);

        // case 4: a very small trade.
        let above_one_base = pool_state
            .get_out_amount(5_u64, SwapDirection::SellBase)
            .unwrap();
        assert_eq!(above_one_base, 75_u64);

        // Sell base, above one.
        // quote_reserve / base_reserve > market_price
        pool_state
            .set_market_price(6u8, 6u8, Decimal::from(5u64))
            .unwrap();
        assert_eq!(pool_state.multiplier, Multiplier::AboveOne);
        // case 1: a trade with 50% of the reserve with high price.
        let above_one_base = pool_state
            .get_out_amount(500_000_u64, SwapDirection::SellBase)
            .unwrap();

        assert_eq!(above_one_base, 2_500_124_u64);

        // case 2: a trade with 5% of the reserve with high price.
        let above_one_base = pool_state
            .get_out_amount(50_000_u64, SwapDirection::SellBase)
            .unwrap();

        assert_eq!(above_one_base, 250_028_u64);

        // case 3: a trade with 0.5% of the reserve with high price.
        let above_one_base = pool_state
            .get_out_amount(5_000_u64, SwapDirection::SellBase)
            .unwrap();

        assert_eq!(above_one_base, 25_003_u64);

        // Sell base, equal one.
        // quote_reserve / base_reserve == market_price
        pool_state
            .set_market_price(6u8, 6u8, Decimal::from(10u64))
            .unwrap();
        assert_eq!(pool_state.multiplier, Multiplier::One);
        // case 1: a trade with 50% of the reserve with high price.
        let equal_one_base = pool_state
            .get_out_amount(500_000_u64, SwapDirection::SellBase)
            .unwrap();

        assert_eq!(equal_one_base, 4_999_500_u64);

        // case 2: a trade with 5% of the reserve with high price.
        let equal_one_base = pool_state
            .get_out_amount(50_000_u64, SwapDirection::SellBase)
            .unwrap();

        assert_eq!(equal_one_base, 499_997_u64);

        // case 3: a trade with 0.5% of the reserve with high price.
        let equal_one_base = pool_state
            .get_out_amount(5_000_u64, SwapDirection::SellBase)
            .unwrap();

        assert_eq!(equal_one_base, 49_999_u64);

        // Sell quote, below one.
        // quote_reserve / base_reserve < market_price
        pool_state
            .set_market_price(6u8, 6u8, Decimal::from(100u64))
            .unwrap();
        assert_eq!(pool_state.multiplier, Multiplier::BelowOne);
        // case 1: a trade with 50% of the reserve with high price.
        let below_one_quote = pool_state
            .get_out_amount(50_000_000u64, SwapDirection::SellQuote)
            .unwrap();
        assert_eq!(below_one_quote, 500_202_u64);

        // case 2: a trade with 5% of the reserve with lower price.
        let below_one_quote = pool_state
            .get_out_amount(5_000_000_u64, SwapDirection::SellQuote)
            .unwrap();
        assert_eq!(below_one_quote, 50_095_u64);

        // case 3: a trade with 0.5% of the reserve with lower price.
        let below_one_quote = pool_state
            .get_out_amount(500_000_u64, SwapDirection::SellQuote)
            .unwrap();
        assert_eq!(below_one_quote, 5_013_u64);

        // Sell quote, above one.
        // quote_reserve / base_reserve > market_price
        pool_state
            .set_market_price(6u8, 6u8, Decimal::from(5u64))
            .unwrap();
        assert_eq!(pool_state.multiplier, Multiplier::AboveOne);
        // case 1: a trade with 50% of the reserve with high price.
        let below_one_quote = pool_state
            .get_out_amount(50_000_000u64, SwapDirection::SellQuote)
            .unwrap();
        assert_eq!(below_one_quote, 999_975_u64);

        // case 2: a trade with 5% of the reserve with lower price.
        let below_one_quote = pool_state
            .get_out_amount(5_000_000_u64, SwapDirection::SellQuote)
            .unwrap();
        assert_eq!(below_one_quote, 985_160_u64);

        // case 3: a trade with 0.5% of the reserve with lower price.
        let below_one_quote = pool_state
            .get_out_amount(500_000_u64, SwapDirection::SellQuote)
            .unwrap();
        assert_eq!(below_one_quote, 99_985_u64);

        // Sell quote, equal one.
        // quote_reserve / base_reserve == market_price
        pool_state
            .set_market_price(6u8, 6u8, Decimal::from(10u64))
            .unwrap();
        assert_eq!(pool_state.multiplier, Multiplier::One);
        // case 1: a trade with 50% of the reserve with high price.
        // the balance is not enough, and give all to trader.
        let below_one_quote = pool_state
            .get_out_amount(50_000_000u64, SwapDirection::SellQuote)
            .unwrap();
        assert_eq!(below_one_quote, 999_975_u64);

        // case 2: a trade with 5% of the reserve with lower price.
        let below_one_quote = pool_state
            .get_out_amount(5_000_000_u64, SwapDirection::SellQuote)
            .unwrap();
        assert_eq!(below_one_quote, 499_950_u64);

        // case 3: a trade with 0.5% of the reserve with lower price.
        let below_one_quote = pool_state
            .get_out_amount(500_000_u64, SwapDirection::SellQuote)
            .unwrap();
        assert_eq!(below_one_quote, 49_999_u64);
    }

    #[test]
    fn test_set_market_price() {
        let mut pool_state = PoolState::new(InitPoolStateParams {
            market_price: default_market_price(),
            slope: Decimal::from_scaled_val(100_000_000_u128), // 0.0001
            base_reserve: Decimal::zero(),
            quote_reserve: Decimal::zero(),
            total_supply: 0,
            last_market_price: default_market_price(), // 100
            last_valid_market_price_slot: 0,
        });

        pool_state
            .set_market_price(9, 6, Decimal::from(100u64))
            .unwrap();
        assert_eq!(
            pool_state.market_price,
            Decimal::from_scaled_val(100_000_000_000u128)
        );

        pool_state
            .set_market_price(6, 9, Decimal::from(100u64))
            .unwrap();
        assert_eq!(pool_state.market_price, Decimal::from(100000u64));

        pool_state
            .set_market_price(8, 8, Decimal::from(100u64))
            .unwrap();
        assert_eq!(pool_state.market_price, Decimal::from(100u64));
    }
}
