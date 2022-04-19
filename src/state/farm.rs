use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::{
    clock::{Slot, UnixTimestamp},
    entrypoint::ProgramResult,
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::{Pubkey, PUBKEY_BYTES},
};
use std::convert::TryFrom;

use crate::{
    error::SwapError,
    math::{Decimal, TryDiv, TryMul},
};

use super::*;

/// Max number of farm positions
pub const MAX_FARM_POSITIONS: usize = 1;

/// Min period towards next claim
pub const MIN_CLAIM_PERIOD: UnixTimestamp = 60;

/// Seconds per year
pub const SECONDS_OF_YEAR: UnixTimestamp = 31556926;

/// Farm states
#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FarmInfo {
    /// Initialized state
    pub is_initialized: bool,

    /// Transfer authority back to LP. pool_token to LP's pool_token
    pub bump_seed: u8,

    /// Config info
    pub config_key: Pubkey,

    /// Pool mint account
    pub pool_mint: Pubkey,
    /// Pool token account to reserve
    pub pool_token: Pubkey,

    /// Total staked amount
    pub reserved_amount: u64,

    /// Withdraw fee numerator
    pub fee_numerator: u64,
    /// Withdraw fee denominator
    pub fee_denominator: u64,

    /// APR numerator
    pub apr_numerator: u64,
    /// APR denominator
    pub apr_denominator: u64,

    /// Reserved 8 * 8 = 64 bytes for future use
    pub reserved: [u64; FARM_INFO_RESERVED_U64],
}

impl Sealed for FarmInfo {}
impl IsInitialized for FarmInfo {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

impl FarmInfo {
    /// Deposit lp token to the farm
    pub fn deposit(&mut self, amount: u64) -> ProgramResult {
        self.reserved_amount = self
            .reserved_amount
            .checked_add(amount)
            .ok_or(SwapError::CalculationFailure)?;
        Ok(())
    }

    /// Withdraw lp token from the farm
    pub fn withdraw(&mut self, amount: u64) -> ProgramResult {
        self.reserved_amount = self
            .reserved_amount
            .checked_sub(amount)
            .ok_or(SwapError::CalculationFailure)?;
        Ok(())
    }

    /// Check the reserve amount match the token amount in the farm.
    pub fn check_reserve_amount(&self, token_amount: u64) -> ProgramResult {
        if self.reserved_amount > token_amount {
            return Err(SwapError::InconsistentPoolState.into());
        }
        Ok(())
    }
}

const FARM_INFO_RESERVED_U64: usize = 8;
const FARM_INFO_RESERVED_BYTES: usize = FARM_INFO_RESERVED_U64 * 8;
const FARM_INFO_SIZE: usize = 138 + FARM_INFO_RESERVED_BYTES;

impl Pack for FarmInfo {
    const LEN: usize = FARM_INFO_SIZE;

    /// Unpacks a byte buffer into a [FarmInfo](struct.FarmInfo.html).
    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![src, 0, FARM_INFO_SIZE];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            is_initialized,
            bump_seed,
            config_key,
            pool_mint,
            pool_token,
            reserved_amount,
            fee_numerator,
            fee_denominator,
            apr_numerator,
            apr_denominator,
            _, // reserved bytes
        ) = array_refs![
            input,
            1,
            1,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            8,
            8,
            8,
            8,
            8,
            FARM_INFO_RESERVED_BYTES
        ];

        Ok(Self {
            is_initialized: unpack_bool(is_initialized)?,
            bump_seed: u8::from_le_bytes(*bump_seed),
            config_key: Pubkey::new_from_array(*config_key),
            pool_mint: Pubkey::new_from_array(*pool_mint),
            pool_token: Pubkey::new_from_array(*pool_token),
            reserved_amount: u64::from_le_bytes(*reserved_amount),
            fee_numerator: u64::from_le_bytes(*fee_numerator),
            fee_denominator: u64::from_le_bytes(*fee_denominator),
            apr_numerator: u64::from_le_bytes(*apr_numerator),
            apr_denominator: u64::from_le_bytes(*apr_denominator),
            // Set all reserved bytes to 0
            reserved: [0u64; FARM_INFO_RESERVED_U64],
        })
    }
    fn pack_into_slice(&self, dst: &mut [u8]) {
        let output = array_mut_ref![dst, 0, FARM_INFO_SIZE];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            is_initialized,
            bump_seed,
            config_key,
            pool_mint,
            pool_token,
            reserved_amount,
            fee_numerator,
            fee_denominator,
            apr_numerator,
            apr_denominator,
            reserved_bytes,
        ) = mut_array_refs![
            output,
            1,
            1,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            8,
            8,
            8,
            8,
            8,
            FARM_INFO_RESERVED_BYTES
        ];
        pack_bool(self.is_initialized, is_initialized);
        *bump_seed = self.bump_seed.to_le_bytes();
        config_key.copy_from_slice(self.config_key.as_ref());
        pool_mint.copy_from_slice(self.pool_mint.as_ref());
        pool_token.copy_from_slice(self.pool_token.as_ref());
        *reserved_amount = self.reserved_amount.to_le_bytes();
        *fee_numerator = self.fee_numerator.to_le_bytes();
        *fee_denominator = self.fee_denominator.to_le_bytes();
        *apr_numerator = self.apr_numerator.to_le_bytes();
        *apr_denominator = self.apr_denominator.to_le_bytes();
        // Set all reserved bytes to 0
        *reserved_bytes = [0u8; FARM_INFO_RESERVED_BYTES];
    }
}

/// Liquidity farm user
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FarmUser {
    /// Initialization status
    pub is_initialized: bool,
    /// Config pubkey
    pub config_key: Pubkey,
    /// Farm pool pubkey
    pub farm_pool_key: Pubkey,
    /// Owner authority
    pub owner: Pubkey,
    /// Farm position
    pub position: FarmPosition,
    /// Reserved 8 * 8 = 64 bytes for future use
    pub reserved: [u64; FARM_INFO_RESERVED_U64],
}

impl FarmUser {
    /// Constructor
    ///
    /// # Arguments
    /// * owner - liquidity farm owner address
    /// * ositions - liquidity provider's current position
    pub fn new(
        config_key: Pubkey,
        farm_pool_key: Pubkey,
        owner: Pubkey,
        position: FarmPosition,
    ) -> Self {
        let mut user = Self::default();
        user.init(config_key, farm_pool_key, owner, position);
        user
    }

    /// Initialize a liquidity provider
    ///
    /// # Arguments
    /// * owner - liquidity provider owner address.
    /// * positions - liquidity provider's current position.
    pub fn init(
        &mut self,
        config_key: Pubkey,
        farm_pool_key: Pubkey,
        owner: Pubkey,
        position: FarmPosition,
    ) {
        self.is_initialized = true;
        self.config_key = config_key;
        self.farm_pool_key = farm_pool_key;
        self.owner = owner;
        self.position = position;
    }

    /// Withdraw liquidity and remove it from deposits if zeroed out
    ///
    /// # Arguments
    /// * withdraw_amount - amount to withdraw from the pool.
    /// * position_index - pool position index
    ///
    /// # Return value
    /// withdraw status
    pub fn withdraw(&mut self, withdraw_amount: u64, current_slot: Slot) -> ProgramResult {
        self.position.withdraw(withdraw_amount, current_slot)?;
        Ok(())
    }

    /// Claim rewards in corresponding position
    ///
    /// # Arguments
    /// * pool - pool address.
    ///
    /// # Return value
    /// claimed amount
    pub fn claim(&mut self) -> Result<u64, ProgramError> {
        let claimed_amount = self.position.claim_rewards()?;
        Ok(claimed_amount)
    }
}

impl Sealed for FarmUser {}
impl IsInitialized for FarmUser {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

const FARM_USER_RESERVED_U64: usize = 8;
const FARM_USER_RESERVED_BYTES: usize = FARM_USER_RESERVED_U64 * 8;
const FARM_POSITION_SIZE: usize = 88;
const FARM_USER_SIZE: usize =
    1 + PUBKEY_BYTES * 3 + 1 + FARM_POSITION_SIZE * MAX_FARM_POSITIONS + FARM_USER_RESERVED_BYTES;
impl Pack for FarmUser {
    const LEN: usize = FARM_USER_SIZE;

    fn pack_into_slice(&self, dst: &mut [u8]) {
        let output = array_mut_ref![dst, 0, FARM_USER_SIZE];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            is_initialized,
            config_key,
            farm_pool_key,
            owner,
            positions_len,
            data_flat,
            reserved_bytes,
        ) = mut_array_refs![
            output,
            1,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            1,
            FARM_POSITION_SIZE * MAX_FARM_POSITIONS,
            FARM_USER_RESERVED_BYTES
        ];
        is_initialized[0] = self.is_initialized as u8;
        config_key.copy_from_slice(self.config_key.as_ref());
        farm_pool_key.copy_from_slice(self.farm_pool_key.as_ref());
        owner.copy_from_slice(self.owner.as_ref());
        // Still pack len = 1 for backward compitability.
        *positions_len = [1u8; 1];

        let position = &self.position;
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            pool,
            depositied_amount,
            rewards_owed,
            rewards_estimated,
            cumulative_interest,
            last_update_ts,
            next_claim_ts,
            latest_deposit_slot,
        ) = mut_array_refs![data_flat, PUBKEY_BYTES, 8, 8, 8, 8, 8, 8, 8];
        pool.copy_from_slice(position.pool.as_ref());
        *depositied_amount = position.deposited_amount.to_le_bytes();
        *rewards_owed = position.rewards_owed.to_le_bytes();
        *rewards_estimated = position.rewards_estimated.to_le_bytes();
        *cumulative_interest = position.cumulative_interest.to_le_bytes();
        *last_update_ts = position.last_update_ts.to_le_bytes();
        *next_claim_ts = position.next_claim_ts.to_le_bytes();
        *latest_deposit_slot = position.latest_deposit_slot.to_le_bytes();

        *reserved_bytes = [0u8; FARM_USER_RESERVED_BYTES];
    }

    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![src, 0, FARM_USER_SIZE];
        #[allow(clippy::ptr_offset_with_cast)]
        let (is_initialized, config_key, farm_pool_key, owner, _, data_flat, _) = array_refs![
            input,
            1,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            1,
            FARM_POSITION_SIZE * MAX_FARM_POSITIONS,
            FARM_USER_RESERVED_BYTES
        ];

        let is_initialized = unpack_bool(is_initialized)?;

        let positions_flat = array_ref![data_flat, 0, FARM_POSITION_SIZE];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            pool,
            depositied_amount,
            rewards_owed,
            rewards_estimated,
            cumulative_interest,
            last_update_ts,
            next_claim_ts,
            latest_deposit_slot,
        ) = array_refs![positions_flat, PUBKEY_BYTES, 8, 8, 8, 8, 8, 8, 8];
        let position = FarmPosition {
            pool: Pubkey::new(pool),
            deposited_amount: u64::from_le_bytes(*depositied_amount),
            rewards_owed: u64::from_le_bytes(*rewards_owed),
            rewards_estimated: u64::from_le_bytes(*rewards_estimated),
            cumulative_interest: u64::from_le_bytes(*cumulative_interest),
            last_update_ts: i64::from_le_bytes(*last_update_ts),
            next_claim_ts: i64::from_le_bytes(*next_claim_ts),
            latest_deposit_slot: u64::from_le_bytes(*latest_deposit_slot),
        };
        Ok(Self {
            is_initialized,
            config_key: Pubkey::new(config_key),
            farm_pool_key: Pubkey::new(farm_pool_key),
            owner: Pubkey::new(owner),
            position,
            reserved: [0u64; FARM_USER_RESERVED_U64],
        })
    }
}

/// Farm position of a pool
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FarmPosition {
    /// Staked LP_Token amount owned by this position
    pub deposited_amount: u64,
    /// Farm pool address
    pub pool: Pubkey,
    /// Rewards amount owed
    pub rewards_owed: u64,
    /// Rewards amount estimated in new claim period
    pub rewards_estimated: u64,
    /// Cumulative interest
    pub cumulative_interest: u64,
    /// Last updated timestamp
    pub last_update_ts: UnixTimestamp,
    /// Next claim timestamp
    pub next_claim_ts: UnixTimestamp,
    /// Latest deposit clock slot
    pub latest_deposit_slot: Slot,
}

impl FarmPosition {
    /// Create a new farm position
    /// # Arguments
    ///
    /// * pool - pool address.
    /// * current_ts - unix timestamp
    ///
    /// # Return value
    ///
    /// farm position
    pub fn new(pool: Pubkey, current_ts: UnixTimestamp) -> Result<Self, ProgramError> {
        Ok(Self {
            pool,
            last_update_ts: current_ts,
            next_claim_ts: current_ts
                .checked_add(MIN_CLAIM_PERIOD)
                .ok_or(SwapError::CalculationFailure)?,
            latest_deposit_slot: 0,
            ..Default::default()
        })
    }

    /// Deposit liquidity
    ///
    /// # Arguments
    /// * amount - amount to deposit.
    /// * current_slot - current network slot of this instruction
    ///
    /// # Return value
    /// deposit status
    pub fn deposit(&mut self, amount: u64, current_slot: Slot) -> ProgramResult {
        self.deposited_amount = self
            .deposited_amount
            .checked_add(amount)
            .ok_or(SwapError::CalculationFailure)?;
        self.latest_deposit_slot = current_slot;

        Ok(())
    }

    /// Withdraw liquidity
    ///
    /// # Arguments
    /// * amount - amount to withdraw.
    /// * current_slot - current network slot of this instruction
    ///
    /// # Return value
    /// withdraw status
    pub fn withdraw(&mut self, amount: u64, current_slot: Slot) -> ProgramResult {
        if amount > self.deposited_amount {
            return Err(SwapError::InsufficientLiquidity.into());
        }

        if current_slot == self.latest_deposit_slot {
            return Err(SwapError::PotentialFlashLoanAttack.into());
        }

        self.deposited_amount = self
            .deposited_amount
            .checked_sub(amount)
            .ok_or(SwapError::CalculationFailure)?;
        Ok(())
    }

    /// Update next claim timestamp
    ///
    /// # Return value
    /// timestamp update status
    pub fn update_claim_ts(&mut self, current_ts: UnixTimestamp) -> ProgramResult {
        if self.deposited_amount != 0 {
            self.next_claim_ts = current_ts
                .checked_add(MIN_CLAIM_PERIOD)
                .ok_or(SwapError::CalculationFailure)?;
        }
        Ok(())
    }

    /// Calculate and update rewards
    ///
    /// # Arguments
    /// * apr - annual percentage ratio.
    /// * current_ts - current unix timestamp.
    /// * is_deposit_withdraw - if called before deposit/withdraw
    ///
    /// # Return value
    /// reward update status
    pub fn calc_and_update_rewards(
        &mut self,
        apr: Decimal,
        current_ts: UnixTimestamp,
        is_deposit_withdraw: bool,
    ) -> ProgramResult {
        let calc_period = current_ts
            .checked_sub(self.last_update_ts)
            .ok_or(SwapError::CalculationFailure)?;
        if calc_period > 0 {
            let new_rewards_estimated = apr
                .try_mul(self.deposited_amount)?
                .try_div(u64::try_from(SECONDS_OF_YEAR).unwrap())?
                .try_mul(u64::try_from(calc_period).unwrap())?
                .try_floor_u64()?
                .checked_add(self.rewards_estimated)
                .ok_or(SwapError::CalculationFailure)?;

            // Only update the rewards_estimated when there is non-zero rewards
            // OR this is called before deposit/withdraw.
            if is_deposit_withdraw || new_rewards_estimated > self.rewards_estimated {
                self.rewards_estimated = new_rewards_estimated;
                self.last_update_ts = current_ts;
            }
        }

        if current_ts >= self.next_claim_ts {
            self.rewards_owed = self
                .rewards_owed
                .checked_add(self.rewards_estimated)
                .ok_or(SwapError::CalculationFailure)?;
            self.rewards_estimated = 0;
            self.update_claim_ts(current_ts)?;
        }
        Ok(())
    }

    /// Claim rewards owed
    ///
    /// # Return value
    /// claimed rewards
    pub fn claim_rewards(&mut self) -> Result<u64, ProgramError> {
        if self.rewards_owed == 0 {
            return Err(SwapError::InsufficientClaimAmount.into());
        }
        self.cumulative_interest = self
            .cumulative_interest
            .checked_add(self.rewards_owed)
            .ok_or(SwapError::CalculationFailure)?;
        let ret = self.rewards_owed;
        self.rewards_owed = 0;
        Ok(ret)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::{math::*, solana_program::clock::Clock};
    use proptest::prelude::*;
    use rand::random;
    use rand::seq::SliceRandom;
    use rand::thread_rng;

    prop_compose! {
        fn deposited_amount_and_ratio()(amount in 0..=u32::MAX)(
            deposited_amount in Just(amount as u64 * 10_000u64),
            apr in 1..=100u64
        ) -> (u64, u64) {
            (deposited_amount, apr)
        }
    }

    proptest! {
        #[test]
        fn test_update_claim_rewards (
            time_stamp_count in 10u64..=30u64,
            (deposited_amount, apr) in deposited_amount_and_ratio()
        ) {
            let mut rng = rand::thread_rng();
            let mut time_stamps = vec![rng.gen::<i64>(); time_stamp_count as usize];

            let mut max_ts = 0;
            for ts in time_stamps.iter_mut() {
                *ts = (ts.abs() % 1_000) + 1;
                max_ts = max_ts.max(*ts);
            }

            let mut farm_position_1 = FarmPosition {
                pool: Pubkey::new_unique(),
                deposited_amount,
                rewards_owed: 0u64,
                rewards_estimated: 0u64,
                cumulative_interest: 0u64,
                last_update_ts: 0i64,
                next_claim_ts: max_ts + 1,
                latest_deposit_slot: 0,
            };

            for ts in time_stamps {
                assert!(farm_position_1.calc_and_update_rewards(Decimal::from(apr), ts, false).is_ok());
            }

            let expected_reward = (apr * deposited_amount) as i64 * max_ts / (SECONDS_OF_YEAR as i64);

            assert_eq!(farm_position_1.rewards_owed, 0u64);
            assert!((farm_position_1.rewards_estimated as i64 <= expected_reward)
                    && (farm_position_1.rewards_estimated as i64 > expected_reward - time_stamp_count as i64));
            assert_eq!(farm_position_1.last_update_ts, max_ts);
            assert_eq!(Err(ProgramError::from(SwapError::InsufficientClaimAmount)), farm_position_1.claim_rewards());

            let mut farm_position_2 = FarmPosition {
                pool: Pubkey::new_unique(),
                deposited_amount,
                rewards_owed: 0u64,
                rewards_estimated: 0u64,
                cumulative_interest: 0u64,
                last_update_ts: 0i64,
                next_claim_ts: 1,
                latest_deposit_slot: 0,
            };

            time_stamps = vec![rng.gen::<i64>(); time_stamp_count as usize];
            max_ts = 0;
            for ts in time_stamps.iter_mut() {
                *ts = (ts.abs() % 1_000) + 1;
                max_ts = max_ts.max(*ts);
            }

            for ts in time_stamps {
                assert!(farm_position_2.calc_and_update_rewards(Decimal::from(apr), ts, false).is_ok());
            }

            let expected_reward = (apr * deposited_amount) as i64 * max_ts / (SECONDS_OF_YEAR as i64);

            assert_eq!(farm_position_2.rewards_estimated, 0u64);
            assert!((farm_position_2.rewards_owed as i64 <= expected_reward)
                    && (farm_position_2.rewards_owed as i64 > expected_reward - time_stamp_count as i64));
            let reward_owed_result = farm_position_2.rewards_owed;
            assert_eq!(Ok(reward_owed_result), farm_position_2.claim_rewards());
        }


        #[test]
        fn test_update_claim_ts (
            updates in 1_000..=2_000,
            deposited_amount in 1..=u16::MAX as u64
        ) {
            let mut farm_position = FarmPosition {
                deposited_amount,
                ..Default::default()
            };

            let mut current_ts = 100;
            let initial_claim_ts = current_ts + farm_position.next_claim_ts;
            for _ in 0..updates {
                assert!(farm_position.update_claim_ts(current_ts).is_ok());
                current_ts = farm_position.next_claim_ts;
            }

            assert_eq!(farm_position.next_claim_ts - initial_claim_ts, MIN_CLAIM_PERIOD * updates as i64);

            farm_position.deposited_amount = 0;
            farm_position.next_claim_ts = initial_claim_ts;
            for _ in 0..updates {
                assert!(farm_position.update_claim_ts(current_ts).is_ok());
            }

            assert_eq!(farm_position.next_claim_ts - initial_claim_ts, 0i64);
        }
    }

    #[test]
    fn test_farm_check_reserve_amount() {
        let mut farm_info: FarmInfo = FarmInfo {
            reserved_amount: 0,
            ..FarmInfo::default()
        };

        assert_eq!(farm_info.deposit(100), Ok(()));
        assert_eq!(farm_info.reserved_amount, 100);

        assert_eq!(farm_info.withdraw(50), Ok(()));
        assert_eq!(farm_info.reserved_amount, 50);

        assert_eq!(
            farm_info.withdraw(51),
            Err(SwapError::CalculationFailure.into())
        );

        assert_eq!(farm_info.reserved_amount, 50);
        assert_eq!(farm_info.check_reserve_amount(50), Ok(()));
        // It is ok to have more balance in token account.
        assert_eq!(farm_info.check_reserve_amount(51), Ok(()));

        assert_eq!(
            farm_info.check_reserve_amount(49),
            Err(SwapError::InconsistentPoolState.into())
        );
    }

    #[test]
    fn test_farm_info_packing() {
        let is_initialized = true;
        let bump_seed = 255;
        let config_key_raw = [2u8; 32];
        let config_key = Pubkey::new_from_array(config_key_raw);
        let pool_mint_raw = [4u8; 32];
        let pool_mint = Pubkey::new_from_array(pool_mint_raw);
        let pool_token_raw = [4u8; 32];
        let pool_token = Pubkey::new_from_array(pool_token_raw);
        let reserved_amount = 100;
        let fee_numerator = 1;
        let fee_denominator = 2;
        let apr_numerator = 12;
        let apr_denominator = 100;
        let reserved = [0u64; FARM_INFO_RESERVED_U64];

        let farm_info = FarmInfo {
            is_initialized,
            bump_seed,
            config_key,
            pool_mint,
            pool_token,
            reserved_amount,
            fee_numerator,
            fee_denominator,
            apr_numerator,
            apr_denominator,
            reserved,
        };

        let mut packed = [0u8; FarmInfo::LEN];
        FarmInfo::pack_into_slice(&farm_info, &mut packed);
        let unpacked = FarmInfo::unpack(&packed).unwrap();
        assert_eq!(farm_info, unpacked);

        let mut packed: Vec<u8> = vec![1, bump_seed];
        packed.extend_from_slice(&config_key_raw);
        packed.extend_from_slice(&pool_mint_raw);
        packed.extend_from_slice(&pool_token_raw);
        packed.extend_from_slice(&reserved_amount.to_le_bytes());
        packed.extend_from_slice(&fee_numerator.to_le_bytes());
        packed.extend_from_slice(&fee_denominator.to_le_bytes());
        packed.extend_from_slice(&apr_numerator.to_le_bytes());
        packed.extend_from_slice(&apr_denominator.to_le_bytes());
        packed.extend_from_slice(&[0u8; FARM_INFO_RESERVED_BYTES]);

        let unpacked = FarmInfo::unpack(&packed).unwrap();
        assert_eq!(farm_info, unpacked);

        let packed = [0u8; FarmInfo::LEN];
        let farm_info: FarmInfo = Default::default();
        let unpack_unchecked = FarmInfo::unpack_unchecked(&packed).unwrap();
        assert_eq!(unpack_unchecked, farm_info);
        let err = FarmInfo::unpack(&packed).unwrap_err();
        assert_eq!(err, ProgramError::UninitializedAccount);
    }

    #[test]
    fn test_farm_user_packing() {
        let is_initialized = true;
        let owner_raw = [1u8; 32];
        let owner = Pubkey::new_from_array(owner_raw);

        let pool_1_raw = [2u8; 32];
        let pool_1 = Pubkey::new_from_array(pool_1_raw);
        let market_config_raw = [3u8; 32];
        let config_key = Pubkey::new_from_array(market_config_raw);
        let farm_pool_key = Pubkey::new_unique();
        let deposited_amount_1: u64 = 300;
        let rewards_owed_1: u64 = 100;
        let rewards_estimated_1: u64 = 40;
        let cumulative_interest_1: u64 = 1000;
        let last_update_ts_1 = Clock::clone(&Default::default()).unix_timestamp + 300;
        let next_claim_ts_1 = last_update_ts_1 + MIN_CLAIM_PERIOD;
        let latest_deposit_slot_1: Slot = 10000;

        let position_1 = FarmPosition {
            pool: pool_1,
            deposited_amount: deposited_amount_1,
            rewards_owed: rewards_owed_1,
            rewards_estimated: rewards_estimated_1,
            cumulative_interest: cumulative_interest_1,
            last_update_ts: last_update_ts_1,
            next_claim_ts: next_claim_ts_1,
            latest_deposit_slot: latest_deposit_slot_1,
        };
        let reserved = [0u64; FARM_USER_RESERVED_U64];

        let farm_user = FarmUser {
            is_initialized,
            config_key,
            farm_pool_key,
            owner,
            position: position_1,
            reserved,
        };

        let mut packed = [0u8; FarmUser::LEN];
        FarmUser::pack_into_slice(&farm_user, &mut packed);
        let unpacked = FarmUser::unpack_from_slice(&packed).unwrap();
        assert_eq!(farm_user, unpacked);

        let mut packed: Vec<u8> = vec![1];
        packed.extend_from_slice(&market_config_raw);
        packed.extend_from_slice(farm_pool_key.as_ref());
        packed.extend_from_slice(&owner_raw);
        packed.extend_from_slice(&(1u8).to_le_bytes());
        packed.extend_from_slice(&pool_1_raw);
        packed.extend_from_slice(&deposited_amount_1.to_le_bytes());
        packed.extend_from_slice(&rewards_owed_1.to_le_bytes());
        packed.extend_from_slice(&rewards_estimated_1.to_le_bytes());
        packed.extend_from_slice(&cumulative_interest_1.to_le_bytes());
        packed.extend_from_slice(&last_update_ts_1.to_le_bytes());
        packed.extend_from_slice(&next_claim_ts_1.to_le_bytes());
        packed.extend_from_slice(&latest_deposit_slot_1.to_le_bytes());
        packed.extend_from_slice(&[0u8; FARM_USER_RESERVED_BYTES]);

        let unpacked = FarmUser::unpack(&packed).unwrap();
        assert_eq!(farm_user, unpacked);

        let packed = [0u8; FarmUser::LEN];
        let farm_user: FarmUser = Default::default();
        let unpack_unchecked = FarmUser::unpack_unchecked(&packed).unwrap();
        assert_eq!(unpack_unchecked, farm_user);

        let err = FarmUser::unpack(&packed).unwrap_err();
        assert_eq!(err, ProgramError::UninitializedAccount);
    }

    #[test]
    fn test_farm_position_deposit_withdraw() {
        let farm_position_res = FarmPosition::new(Pubkey::new_unique(), 0);
        assert!(farm_position_res.is_ok());
        let mut farm_position = farm_position_res.unwrap();

        let mut deposit_prices: [u64; 30] = random();
        let mut withdraw_prices: [u64; 30] = [0; 30];
        withdraw_prices.clone_from_slice(&deposit_prices);
        let mut rng = thread_rng();
        withdraw_prices.shuffle(&mut rng);

        for deposit in deposit_prices.iter_mut() {
            *deposit %= 10_000_000u64;
        }

        for withdraw in withdraw_prices.iter_mut() {
            *withdraw %= 10_000_000u64;
        }

        let current_slot: Slot = 100;
        for deposit in deposit_prices.iter() {
            assert!(farm_position.deposit(*deposit, current_slot).is_ok());
        }

        for withdraw in withdraw_prices.iter() {
            assert!(farm_position.withdraw(*withdraw, current_slot + 1).is_ok());
        }

        assert_eq!(farm_position.deposited_amount, 0);

        // single deposit and withdraw verification
        assert!(farm_position.deposit(30_u64, current_slot).is_ok());
        assert_eq!(farm_position.deposited_amount, 30_u64);
        assert!(farm_position.withdraw(30_u64, current_slot + 1).is_ok());
        assert_eq!(farm_position.deposited_amount, 0);

        // Flashloan detection
        assert!(farm_position.deposit(30_u64, current_slot).is_ok());
        assert_eq!(farm_position.deposited_amount, 30_u64);
        assert_eq!(
            farm_position.withdraw(30_u64, current_slot),
            Err(ProgramError::from(SwapError::PotentialFlashLoanAttack))
        );
        assert_eq!(farm_position.deposited_amount, 30_u64);
    }

    #[test]
    fn test_farm_user_withdraw() {
        let mut farm_user = FarmUser::new(
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            FarmPosition::new(Pubkey::new_unique(), 0i64).unwrap(),
        );
        let current_slot: Slot = 100;

        farm_user.position.deposited_amount = 100_000u64;
        assert!(!farm_user.withdraw(2_000_000, current_slot).is_ok());
        assert_eq!(farm_user.position.deposited_amount, 100_000u64);

        farm_user.position.deposited_amount = 2_500_000u64;
        assert!(farm_user.withdraw(2_000_000, current_slot).is_ok());
        assert_eq!(farm_user.position.deposited_amount, 500_000u64);

        assert!(farm_user.withdraw(500_000, current_slot).is_ok());
    }

    #[test]
    fn test_farm_user_claim() {
        let mut farm_user = FarmUser::new(
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            FarmPosition::new(Pubkey::new_unique(), 0i64).unwrap(),
        );

        assert_eq!(
            farm_user.claim(),
            Err(ProgramError::from(SwapError::InsufficientClaimAmount))
        );
        farm_user.position.cumulative_interest = 10_000u64;
        farm_user.position.rewards_owed = 0u64;
        assert_eq!(
            farm_user.claim(),
            Err(ProgramError::from(SwapError::InsufficientClaimAmount))
        );
    }

    #[test]
    fn test_farm_user_refresh_claim() {
        let max_ts = 10000;
        let mut farm_position_1 = FarmPosition {
            pool: Pubkey::new_unique(),
            deposited_amount: 100_000u64,
            rewards_owed: 0u64,
            rewards_estimated: 0u64,
            cumulative_interest: 0u64,
            last_update_ts: 0i64,
            next_claim_ts: max_ts + 1,
            latest_deposit_slot: 0,
        };
        assert_eq!(farm_position_1.rewards_estimated, 0);
        assert_eq!(farm_position_1.deposited_amount, 100_000u64);

        let current_slot: Slot = 100;
        let apr: u64 = 5;
        // calc_and_update_rewards before 1st deposit at ts 1000
        assert!(farm_position_1
            .calc_and_update_rewards(Decimal::from(apr), 1000, true)
            .is_ok());
        assert_eq!(farm_position_1.rewards_owed, 0);
        assert_eq!(farm_position_1.rewards_estimated, 15); // 5 * 100_000 * 1000 / SECONDS_OF_YEAR = 15

        assert!(farm_position_1.deposit(1_000u64, current_slot).is_ok()); // deposit 1_000u64
        assert_eq!(farm_position_1.deposited_amount, 101_000u64); // 100_000 + 1_000 = 101_000

        // calc_and_update_rewards before 2nd deposit at ts 2000
        assert!(farm_position_1
            .calc_and_update_rewards(Decimal::from(apr), 2000, true)
            .is_ok());
        assert_eq!(farm_position_1.rewards_estimated, 31); // 15 + 5 * 101_000 * 1000 / SECONDS_OF_YEAR = 31

        assert!(farm_position_1.deposit(100_000u64, current_slot).is_ok()); // deposit 100_000u64
        assert_eq!(farm_position_1.deposited_amount, 201_000u64);

        // calc_and_update_rewards at ts 3000
        assert!(farm_position_1
            .calc_and_update_rewards(Decimal::from(apr), 3000, true)
            .is_ok());
        assert_eq!(farm_position_1.rewards_estimated, 62); // 31 + 5 * 201_000 * 1000 / SECONDS_OF_YEAR = 62

        /* Test flash loan */
        let mut farm_position_2 = FarmPosition {
            pool: Pubkey::new_unique(),
            deposited_amount: 0u64,
            rewards_owed: 0u64,
            rewards_estimated: 0u64,
            cumulative_interest: 0u64,
            last_update_ts: 0i64,
            next_claim_ts: max_ts + 1,
            latest_deposit_slot: 0,
        };

        // 1. calc_and_update_rewards, deposit 1 token at ts 1
        assert!(farm_position_2
            .calc_and_update_rewards(Decimal::from(apr), 1, true)
            .is_ok());
        assert!(farm_position_2.deposit(1u64, current_slot).is_ok());
        assert_eq!(farm_position_2.last_update_ts, 1); // last_update_ts is updated to ts 1
        assert_eq!(farm_position_2.rewards_estimated, 0); // rewards_estimated is 0
        assert_eq!(farm_position_2.rewards_owed, 0);

        // 2. calc_and_update_rewards, deposit 100_000 tokens at ts 1000
        assert!(farm_position_2
            .calc_and_update_rewards(Decimal::from(apr), 1000, true)
            .is_ok());
        assert!(farm_position_2.deposit(100_000u64, current_slot).is_ok());
        assert_eq!(farm_position_2.last_update_ts, 1000);
        assert_eq!(farm_position_2.rewards_estimated, 0); //rewards_estimated is still 0
        assert_eq!(farm_position_2.rewards_owed, 0);

        // 3. calc_and_update_rewards at ts 2000
        assert!(farm_position_2
            .calc_and_update_rewards(Decimal::from(apr), 2000, false)
            .is_ok());
        assert_eq!(farm_position_2.rewards_estimated, 15); // 100_001 * (2000-1000) *5 /SECONDS_OF_YEAR = 15

        // next_claim_ts is not reached
        assert_eq!(
            farm_position_2.claim_rewards(),
            Err(ProgramError::from(SwapError::InsufficientClaimAmount))
        );

        // 4. calc_and_update_rewards at ts max_ts + 10 > next_claim_ts
        assert!(farm_position_2
            .calc_and_update_rewards(Decimal::from(apr), max_ts + 10, true)
            .is_ok());
        assert_eq!(farm_position_2.rewards_owed, 141); // 15 + 100_001 * (10000-2000) *5 /SECONDS_OF_YEAR = 141
        assert_eq!(farm_position_2.claim_rewards(), Ok(141));
        assert_eq!(
            farm_position_2.next_claim_ts,
            max_ts + 10 + MIN_CLAIM_PERIOD
        );
        assert_eq!(farm_position_2.rewards_owed, 0); // reset to 0 after claim

        // 5. withdraw 100_000 at the same slot with deposit, a PotentialFlashLoanAttack
        assert_eq!(
            farm_position_2.withdraw(100_000u64, current_slot),
            Err(ProgramError::from(SwapError::PotentialFlashLoanAttack))
        );

        // 6. withdraw 100_000 at a different slot
        assert!(farm_position_2
            .withdraw(100_000u64, current_slot + 1)
            .is_ok());
    }
}
