//! Program rewards

use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::{
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
};

use crate::math::{Decimal, TryDiv, TryMul};

use super::*;

const REWARD_TOKEN_DECIMALS: u8 = 6;
const REWARD_TOKEN_THRESHOLD: u64 = 1000000;

/// Rewards structure
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rewards {
    /// Initialized state
    pub is_initialized: bool,
    /// Base token decimals
    pub decimals: u8,
    /// Reserved for future use
    pub reserved: [u8; 7],
    /// Trade reward numerator
    pub trade_reward_numerator: u64,
    /// Trade reward denominator
    pub trade_reward_denominator: u64,
    /// Trade reward cap
    pub trade_reward_cap: u64,
}

const REFERRAL_REWARD_NUMERATOR: u64 = 1;
const REFERRAL_REWARD_DENOMINATOR: u64 = 20;

impl Rewards {
    /// Create new rewards
    ///
    /// # Arguments
    ///
    /// * params - rewards params.
    ///
    /// # Return value
    ///
    /// rewards constructed.
    pub fn new(params: &Self) -> Self {
        Rewards {
            is_initialized: params.is_initialized,
            decimals: params.decimals,
            reserved: params.reserved,
            trade_reward_numerator: params.trade_reward_numerator,
            trade_reward_denominator: params.trade_reward_denominator,
            trade_reward_cap: params.trade_reward_cap,
        }
    }

    /// Calc trade reward amount with [`u64`]
    ///
    /// # Arguments
    ///
    /// * amount - trade amount.
    ///
    /// # Return value
    ///
    /// trade reward.
    pub fn trade_reward_u64(&self, amount: u64) -> Result<u64, ProgramError> {
        let normalized_input_amount_decimal = Decimal::from(amount)
            .try_mul(10u64.pow(REWARD_TOKEN_DECIMALS as u32))?
            .try_div(10u64.pow(self.decimals as u32))?;
        let threshold = Decimal::from(REWARD_TOKEN_THRESHOLD);

        let c_reward = if normalized_input_amount_decimal <= threshold {
            normalized_input_amount_decimal
                .try_mul(self.trade_reward_numerator)?
                .try_div(self.trade_reward_denominator)?
        } else {
            normalized_input_amount_decimal
                .sqrt()?
                .try_mul(threshold.sqrt()?)?
                .try_mul(self.trade_reward_numerator)?
                .try_div(self.trade_reward_denominator)?
        };

        Ok(if c_reward > Decimal::from(self.trade_reward_cap) {
            self.trade_reward_cap
        } else {
            c_reward.try_floor_u64()?
        })
    }

    /// Calculate the referral rewards.
    pub fn referral_reward(&self, trade_reward: u64) -> Result<u64, ProgramError> {
        Decimal::from(trade_reward)
            .try_mul(REFERRAL_REWARD_NUMERATOR)?
            .try_div(REFERRAL_REWARD_DENOMINATOR)?
            .try_floor_u64()
    }
}

impl Sealed for Rewards {}
impl IsInitialized for Rewards {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

const REWARDS_SIZE: usize = 33;
impl Pack for Rewards {
    const LEN: usize = REWARDS_SIZE;
    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        // println!("input len {}", input.len());

        let input = array_ref![input, 0, REWARDS_SIZE];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            is_initialized,
            decimals,
            reserved,
            trade_reward_numerator,
            trade_reward_denominator,
            trade_reward_cap,
        ) = array_refs![input, 1, 1, 7, 8, 8, 8];
        Ok(Self {
            is_initialized: unpack_bool(is_initialized)?,
            decimals: u8::from_le_bytes(*decimals),
            reserved: *reserved,
            trade_reward_numerator: u64::from_le_bytes(*trade_reward_numerator),
            trade_reward_denominator: u64::from_le_bytes(*trade_reward_denominator),
            trade_reward_cap: u64::from_le_bytes(*trade_reward_cap),
        })
    }
    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, REWARDS_SIZE];
        let (
            is_initialized,
            decimals,
            reserved,
            trade_reward_numerator,
            trade_reward_denominator,
            trade_reward_cap,
        ) = mut_array_refs![output, 1, 1, 7, 8, 8, 8];
        pack_bool(self.is_initialized, is_initialized);
        *decimals = self.decimals.to_le_bytes();
        *reserved = self.reserved;
        *trade_reward_numerator = self.trade_reward_numerator.to_le_bytes();
        *trade_reward_denominator = self.trade_reward_denominator.to_le_bytes();
        *trade_reward_cap = self.trade_reward_cap.to_le_bytes();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::DEFAULT_TEST_REWARDS;

    #[test]
    fn pack_rewards() {
        let rewards = DEFAULT_TEST_REWARDS;

        let mut packed = [0u8; Rewards::LEN];
        Rewards::pack_into_slice(&rewards, &mut packed[..]);
        let unpacked = Rewards::unpack_from_slice(&packed).unwrap();
        assert_eq!(rewards, unpacked);

        let mut packed = vec![];
        let is_initialized = vec![1, rewards.is_initialized as u8];
        packed.extend_from_slice(&is_initialized[0].to_le_bytes());
        packed.extend_from_slice(&rewards.decimals.to_le_bytes());
        packed.extend_from_slice(&[0u8; 7]);
        packed.extend_from_slice(&rewards.trade_reward_numerator.to_le_bytes());
        packed.extend_from_slice(&rewards.trade_reward_denominator.to_le_bytes());
        packed.extend_from_slice(&rewards.trade_reward_cap.to_le_bytes());
        let unpacked = Rewards::unpack_from_slice(&packed).unwrap();
        assert_eq!(rewards, unpacked);
    }

    #[test]
    fn test_referral_reward() {
        let rewards = Rewards {
            is_initialized: true,
            decimals: 6,
            reserved: [0u8; 7],
            trade_reward_numerator: 1,
            trade_reward_denominator: 100,
            trade_reward_cap: 1_000_000u64,
        };
        assert_eq!(rewards.referral_reward(100), Ok(5));
    }

    #[test]
    fn test_reward_results() {
        let rewards = Rewards {
            is_initialized: true,
            decimals: 6,
            reserved: [0u8; 7],
            trade_reward_numerator: 1,
            trade_reward_denominator: 100,
            trade_reward_cap: 1_000_000u64,
        };
        assert!(rewards.is_initialized());
        // At threshold
        assert_eq!(rewards.trade_reward_u64(1_000_000u64), Ok(10_000u64));

        // Lower than threshold. reward = amount * 1%
        assert_eq!(rewards.trade_reward_u64(100_000u64), Ok(1_000u64));

        // Higher than threshold. reward = sqrt(amount) * 1000 * 1%
        assert_eq!(rewards.trade_reward_u64(100_000_000u64), Ok(100_000u64));

        // Set cap to 50.
        let capped_rewards = Rewards {
            trade_reward_cap: 50,
            ..rewards
        };
        assert_eq!(capped_rewards.trade_reward_u64(100_000_000u64), Ok(50));
    }

    #[test]
    fn test_reward_results_decimals_9() {
        let rewards = Rewards {
            is_initialized: true,
            decimals: 9,
            reserved: [0u8; 7],
            trade_reward_numerator: 1,
            trade_reward_denominator: 100,
            trade_reward_cap: 1_000_000u64,
        };
        assert!(rewards.is_initialized());
        // At threshold
        assert_eq!(rewards.trade_reward_u64(1_000_000_000u64), Ok(10_000u64));

        // Lower than threshold. reward = amount/1000 * 1%
        assert_eq!(rewards.trade_reward_u64(100_000_000u64), Ok(1_000u64));

        // Higher than threshold. reward = sqrt(amount/1000) * 1000 * 1%
        assert_eq!(rewards.trade_reward_u64(100_000_000_000u64), Ok(100_000u64));

        // Set cap to 50.
        let capped_rewards = Rewards {
            trade_reward_cap: 50,
            ..rewards
        };
        assert_eq!(capped_rewards.trade_reward_u64(100_000_000_000u64), Ok(50));
    }
}
