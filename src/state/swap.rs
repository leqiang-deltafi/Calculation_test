use std::{
    cmp::Ordering,
    convert::{TryFrom, TryInto},
};

use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use bitflags::bitflags;
use solana_program::{
    entrypoint::ProgramResult,
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::{Pubkey, PUBKEY_BYTES},
};

use super::*;
use crate::{
    curve::{PoolState, SwapDirection},
    error::SwapError,
    math::{Decimal, TryDiv, TryMul},
};

/// SwapType enumerated definition
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum SwapType {
    /// Standard swap pool with external price guidence
    Normal,
    /// Stable swap pool
    Stable,
}

impl Default for SwapType {
    fn default() -> Self {
        Self::Normal
    }
}

impl TryFrom<u8> for SwapType {
    type Error = ProgramError;

    fn try_from(curve_type: u8) -> Result<Self, Self::Error> {
        match curve_type {
            0 => Ok(SwapType::Normal),
            1 => Ok(SwapType::Stable),
            _ => Err(ProgramError::InvalidAccountData),
        }
    }
}

bitflags! {
#[derive(Default)]
#[repr(C)]
    /// Oracle priority bitflags definition
    pub struct OraclePriorityFlag: u8 {
        /// PYTH_ONLY = 0b0
        const PYTH_ONLY = 0b00000000;
        /// SERUM_ONLY = 0b1
        const SERUM_ONLY = 0b00000001;
    }
}

impl OraclePriorityFlag {
    /// is_pyth_only
    #[inline(always)]
    pub fn is_pyth_only(&self) -> bool {
        self.contains(OraclePriorityFlag::PYTH_ONLY)
    }

    /// is_serum_only
    #[inline(always)]
    pub fn is_serum_only(&self) -> bool {
        self.contains(OraclePriorityFlag::SERUM_ONLY)
    }
}

/// User referrer data
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UserReferrerData {
    /// Initialized state
    pub is_initialized: bool,
    /// Config pubkey
    pub config_key: Pubkey,
    /// Owner pubkey
    pub owner: Pubkey,
    /// Referrer pubkey
    pub referrer: Pubkey,
}

const USER_REFERRER_DATA_LEN: usize = 1 + PUBKEY_BYTES * 3;

impl Pack for UserReferrerData {
    const LEN: usize = USER_REFERRER_DATA_LEN;

    /// Unpacks a byte buffer into a UserReferrerData
    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![src, 0, USER_REFERRER_DATA_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (is_initialized, config_key, owner, referrer) =
            array_refs![input, 1, PUBKEY_BYTES, PUBKEY_BYTES, PUBKEY_BYTES];

        Ok(Self {
            is_initialized: unpack_bool(is_initialized)?,
            config_key: Pubkey::new_from_array(*config_key),
            owner: Pubkey::new_from_array(*owner),
            referrer: Pubkey::new_from_array(*referrer),
        })
    }

    fn pack_into_slice(&self, dst: &mut [u8]) {
        let output = array_mut_ref![dst, 0, USER_REFERRER_DATA_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (is_initialized, config_key, owner, referrer) =
            mut_array_refs![output, 1, PUBKEY_BYTES, PUBKEY_BYTES, PUBKEY_BYTES];

        pack_bool(self.is_initialized, is_initialized);
        config_key.copy_from_slice(self.config_key.as_ref());
        owner.copy_from_slice(self.owner.as_ref());
        referrer.copy_from_slice(self.referrer.as_ref());
    }
}

impl Sealed for UserReferrerData {}
impl IsInitialized for UserReferrerData {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

/// mocked swap struct
#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MockedSwap {
    /// target reserve a
    pub target_reserve_a: u64,
    /// target reserve b
    pub target_reserve_b: u64,
    /// current reserve a
    pub current_reserve_a: u64,
    /// current reserve b
    pub current_reserve_b: u64,
    /// market price
    pub market_price: u64,
}

const MOCKED_SWAP_LEN: usize = 40;
impl Pack for MockedSwap {
    const LEN: usize = MOCKED_SWAP_LEN;

    /// Unpacks a byte buffer into a UserReferrerData
    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![src, 0, MOCKED_SWAP_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (target_reserve_a, target_reserve_b, current_reserve_a, current_reserve_b, market_price) =
            array_refs![input, 8, 8, 8, 8, 8];

        Ok(Self {
            target_reserve_a: u64::from_le_bytes(*target_reserve_a),
            target_reserve_b: u64::from_le_bytes(*target_reserve_b),
            current_reserve_a: u64::from_le_bytes(*current_reserve_a),
            current_reserve_b: u64::from_le_bytes(*current_reserve_b),
            market_price: u64::from_le_bytes(*market_price),
        })
    }

    fn pack_into_slice(&self, dst: &mut [u8]) {
        let output = array_mut_ref![dst, 0, MOCKED_SWAP_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (target_reserve_a, target_reserve_b, current_reserve_a, current_reserve_b, market_price) =
            mut_array_refs![output, 8, 8, 8, 8, 8];

        *target_reserve_a = self.target_reserve_a.to_le_bytes();
        *target_reserve_b = self.target_reserve_b.to_le_bytes();
        *current_reserve_a = self.current_reserve_a.to_le_bytes();
        *current_reserve_b = self.current_reserve_b.to_le_bytes();
        *market_price = self.market_price.to_le_bytes();
    }
}

impl Sealed for MockedSwap {}
impl IsInitialized for MockedSwap {
    fn is_initialized(&self) -> bool {
        true
    }
}


/// Swap states.
#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SwapInfo {
    /// Initialized state
    pub is_initialized: bool,

    /// Paused state
    pub is_paused: bool,

    /// Nonce used in program address
    /// The program address is created deterministically with the nonce,
    /// swap program id, and swap account pubkey.  This program address has
    /// authority over the swap's token A account, token B account, and pool
    /// token mint.
    pub nonce: u8,

    /// Swap type
    pub swap_type: SwapType,

    /// Config info
    pub config_key: Pubkey,

    /// Token A
    pub token_a: Pubkey,
    /// Token B
    pub token_b: Pubkey,

    /// Pyth A price
    pub pyth_a: Pubkey,
    /// Pyth B price
    pub pyth_b: Pubkey,

    /// Pool tokens are issued when A or B tokens are deposited.
    /// Pool tokens can be withdrawn back to the original A or B token.
    pub pool_mint: Pubkey,
    /// Mint information for token A
    pub token_a_mint: Pubkey,
    /// Mint information for token B
    pub token_b_mint: Pubkey,

    /// Public key of the admin token account to receive trading and / or withdrawal fees for token a
    pub admin_fee_key_a: Pubkey,
    /// Public key of the admin token account to receive trading and / or withdrawal fees for token b
    pub admin_fee_key_b: Pubkey,
    /// Fees
    pub fees: Fees,
    /// Rewards
    pub rewards: Rewards,

    /// Pool object
    pub pool_state: PoolState,

    /// decimals of the base token
    pub token_a_decimals: u8,
    /// decimals of the quote token
    pub token_b_decimals: u8,
    /// max percentage of the swap out amount to the reserved amount
    pub swap_out_limit_percentage: u8,

    /// oracle prioroty flags
    pub oracle_priority_flags: u8,

    /// Public key combined from serumMarket, serumBids and serumAsks together
    pub serum_combined_address: Pubkey,

    /// reserved u8 array for alignment
    pub reserved_u8: [u8; SWAP_INFO_RESERVED_U8],

    /// rest of reserved bytes
    pub reserved: [u64; SWAP_INFO_RESERVED_U64],
}

impl SwapInfo {
    /// check if the amount to be swapped out exceeds the limit
    pub fn check_swap_out_amount(
        &self,
        amount_out: u64,
        swap_direction: SwapDirection,
    ) -> ProgramResult {
        // value 0 means no limitation, this makes it compatible with the old version
        if self.swap_out_limit_percentage == 0u8 {
            return Ok(());
        }

        let reserved_amount = match swap_direction {
            SwapDirection::SellBase => self.pool_state.quote_reserve,
            SwapDirection::SellQuote => self.pool_state.base_reserve,
        };

        match reserved_amount
            .try_mul(self.swap_out_limit_percentage as u64)?
            .try_div(100u64)?
            .cmp(&Decimal::from(amount_out))
        {
            Ordering::Greater => Ok(()),
            Ordering::Equal | Ordering::Less => Err(SwapError::ExceededSwapOutAmount.into()),
        }
    }
}

impl Sealed for SwapInfo {}
impl IsInitialized for SwapInfo {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

/// this should not be changed
const SWAP_INFO_SIZE: usize = 615;
/// this should be updated every time we add new field
const USED_BYTES: usize = 587;
const SWAP_INFO_RESERVED_BYTES: usize = SWAP_INFO_SIZE - USED_BYTES;

const SWAP_INFO_RESERVED_U64: usize = SWAP_INFO_RESERVED_BYTES / 8;
const SWAP_INFO_RESERVED_U8: usize = SWAP_INFO_RESERVED_BYTES - SWAP_INFO_RESERVED_U64 * 8;

impl Pack for SwapInfo {
    const LEN: usize = SWAP_INFO_SIZE;

    /// Unpacks a byte buffer into a [SwapInfo](struct.SwapInfo.html).
    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, SWAP_INFO_SIZE];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            is_initialized,
            is_paused,
            nonce,
            swap_type,
            config_key,
            token_a,
            token_b,
            pyth_a,
            pyth_b,
            pool_mint,
            token_a_mint,
            token_b_mint,
            admin_fee_key_a,
            admin_fee_key_b,
            fees,
            rewards,
            pool_state,
            token_a_decimals,
            token_b_decimals,
            swap_out_limit_percentage,
            oracle_priority_flags,
            serum_combined_address,
            _,
        ) = array_refs![
            input,
            1,
            1,
            1,
            1,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            Fees::LEN,
            Rewards::LEN,
            PoolState::LEN,
            1,
            1,
            1,
            1,
            PUBKEY_BYTES,
            SWAP_INFO_RESERVED_BYTES
        ];
        Ok(Self {
            is_initialized: unpack_bool(is_initialized)?,
            is_paused: unpack_bool(is_paused)?,
            nonce: u8::from_le_bytes(*nonce),
            swap_type: swap_type[0].try_into()?,
            config_key: Pubkey::new_from_array(*config_key),
            token_a: Pubkey::new_from_array(*token_a),
            token_b: Pubkey::new_from_array(*token_b),
            pyth_a: Pubkey::new_from_array(*pyth_a),
            pyth_b: Pubkey::new_from_array(*pyth_b),
            pool_mint: Pubkey::new_from_array(*pool_mint),
            token_a_mint: Pubkey::new_from_array(*token_a_mint),
            token_b_mint: Pubkey::new_from_array(*token_b_mint),
            admin_fee_key_a: Pubkey::new_from_array(*admin_fee_key_a),
            admin_fee_key_b: Pubkey::new_from_array(*admin_fee_key_b),
            fees: Fees::unpack_from_slice(fees)?,
            rewards: Rewards::unpack_from_slice(rewards)?,
            pool_state: PoolState::unpack_from_slice(pool_state)?,
            token_a_decimals: u8::from_le_bytes(*token_a_decimals),
            token_b_decimals: u8::from_le_bytes(*token_b_decimals),
            swap_out_limit_percentage: u8::from_le_bytes(*swap_out_limit_percentage),
            oracle_priority_flags: u8::from_le_bytes(*oracle_priority_flags),
            serum_combined_address: Pubkey::new_from_array(*serum_combined_address),
            ..Self::default()
        })
    }

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, SWAP_INFO_SIZE];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            is_initialized,
            is_paused,
            nonce,
            swap_type,
            config_key,
            token_a,
            token_b,
            pyth_a,
            pyth_b,
            pool_mint,
            token_a_mint,
            token_b_mint,
            admin_fee_key_a,
            admin_fee_key_b,
            fees,
            rewards,
            pool_state,
            token_a_decimals,
            token_b_decimals,
            swap_out_limit_percentage,
            oracle_priority_flags,
            serum_combined_address,
            _,
        ) = mut_array_refs![
            output,
            1,
            1,
            1,
            1,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            Fees::LEN,
            Rewards::LEN,
            PoolState::LEN,
            1,
            1,
            1,
            1,
            PUBKEY_BYTES,
            SWAP_INFO_RESERVED_BYTES
        ];
        pack_bool(self.is_initialized, is_initialized);
        pack_bool(self.is_paused, is_paused);
        *nonce = self.nonce.to_le_bytes();
        swap_type[0] = self.swap_type as u8;
        config_key.copy_from_slice(self.config_key.as_ref());
        token_a.copy_from_slice(self.token_a.as_ref());
        token_b.copy_from_slice(self.token_b.as_ref());
        pyth_a.copy_from_slice(self.pyth_a.as_ref());
        pyth_b.copy_from_slice(self.pyth_b.as_ref());
        pool_mint.copy_from_slice(self.pool_mint.as_ref());
        token_a_mint.copy_from_slice(self.token_a_mint.as_ref());
        token_b_mint.copy_from_slice(self.token_b_mint.as_ref());
        admin_fee_key_a.copy_from_slice(self.admin_fee_key_a.as_ref());
        admin_fee_key_b.copy_from_slice(self.admin_fee_key_b.as_ref());
        self.fees.pack_into_slice(&mut fees[..]);
        self.rewards.pack_into_slice(&mut rewards[..]);
        self.pool_state.pack_into_slice(&mut pool_state[..]);
        token_a_decimals.copy_from_slice(&self.token_a_decimals.to_le_bytes());
        token_b_decimals.copy_from_slice(&self.token_b_decimals.to_le_bytes());
        swap_out_limit_percentage.copy_from_slice(&self.swap_out_limit_percentage.to_le_bytes());
        oracle_priority_flags.copy_from_slice(&self.oracle_priority_flags.to_le_bytes());
        serum_combined_address.copy_from_slice(self.serum_combined_address.as_ref());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::curve::{default_market_price, default_slope, InitPoolStateParams, PoolState};

    #[test]
    fn test_swap_info_packing() {
        let is_initialized = true;
        let is_paused = false;
        let nonce = 255;
        let config_key_raw = [2u8; 32];
        let token_a_raw = [3u8; 32];
        let token_b_raw = [4u8; 32];
        let pool_mint_raw = [5u8; 32];
        let token_a_mint_raw = [6u8; 32];
        let token_b_mint_raw = [7u8; 32];
        let admin_fee_key_a_raw = [8u8; 32];
        let admin_fee_key_b_raw = [9u8; 32];
        let pyth_a_raw = [10u8; 32];
        let pyth_b_raw = [11u8; 32];
        let serum_combined_address_raw = [12u8; 32];
        let config_key = Pubkey::new_from_array(config_key_raw);
        let token_a = Pubkey::new_from_array(token_a_raw);
        let token_b = Pubkey::new_from_array(token_b_raw);
        let pyth_a = Pubkey::new_from_array(pyth_a_raw);
        let pyth_b = Pubkey::new_from_array(pyth_b_raw);
        let serum_combined_address = Pubkey::new_from_array(serum_combined_address_raw);
        let pool_mint = Pubkey::new_from_array(pool_mint_raw);
        let token_a_mint = Pubkey::new_from_array(token_a_mint_raw);
        let token_b_mint = Pubkey::new_from_array(token_b_mint_raw);
        let admin_fee_key_a = Pubkey::new_from_array(admin_fee_key_a_raw);
        let admin_fee_key_b = Pubkey::new_from_array(admin_fee_key_b_raw);
        let fees = DEFAULT_TEST_FEES;
        let rewards = DEFAULT_TEST_REWARDS;
        let swap_type = SwapType::Normal;
        let mut pool_state = PoolState::new(InitPoolStateParams {
            market_price: default_market_price(),
            slope: default_slope(),
            base_reserve: Decimal::zero(),
            quote_reserve: Decimal::zero(),
            total_supply: 0,
            last_market_price: default_market_price(),
            last_valid_market_price_slot: 0,
        });
        pool_state.adjust_target().unwrap();

        let token_a_decimals = 6u8;
        let token_b_decimals = 9u8;
        let swap_out_limit_percentage = 20u8;
        let oracle_priority_flags = 0b11u8;

        let swap_info = SwapInfo {
            is_initialized,
            is_paused,
            nonce,
            swap_type,
            config_key,
            token_a,
            token_b,
            pyth_a,
            pyth_b,
            pool_mint,
            token_a_mint,
            token_b_mint,
            admin_fee_key_a,
            admin_fee_key_b,
            fees: fees.clone(),
            rewards: rewards.clone(),
            pool_state: pool_state.clone(),
            token_a_decimals,
            token_b_decimals,
            swap_out_limit_percentage,
            oracle_priority_flags,
            serum_combined_address,
            ..SwapInfo::default()
        };

        let mut packed = [0u8; SwapInfo::LEN];
        SwapInfo::pack_into_slice(&swap_info, &mut packed);
        let unpacked = SwapInfo::unpack(&packed).unwrap();
        assert_eq!(swap_info, unpacked);

        let mut packed: Vec<u8> = vec![1, 0, nonce];
        packed.extend_from_slice(&(swap_type as u8).to_le_bytes());
        packed.extend_from_slice(&config_key_raw);
        packed.extend_from_slice(&token_a_raw);
        packed.extend_from_slice(&token_b_raw);
        packed.extend_from_slice(&pyth_a_raw);
        packed.extend_from_slice(&pyth_b_raw);
        packed.extend_from_slice(&pool_mint_raw);
        packed.extend_from_slice(&token_a_mint_raw);
        packed.extend_from_slice(&token_b_mint_raw);
        packed.extend_from_slice(&admin_fee_key_a_raw);
        packed.extend_from_slice(&admin_fee_key_b_raw);

        let mut packed_fees = [0u8; Fees::LEN];
        fees.pack_into_slice(&mut packed_fees);
        packed.extend_from_slice(&packed_fees);
        let mut packed_rewards = [0u8; Rewards::LEN];
        rewards.pack_into_slice(&mut packed_rewards);
        packed.extend_from_slice(&packed_rewards);
        let mut packed_pool_state = [0u8; PoolState::LEN];
        pool_state.pack_into_slice(&mut packed_pool_state);
        packed.extend_from_slice(&packed_pool_state);
        packed.extend_from_slice(&token_a_decimals.to_le_bytes());
        packed.extend_from_slice(&token_b_decimals.to_le_bytes());
        packed.extend_from_slice(&swap_out_limit_percentage.to_le_bytes());
        packed.extend_from_slice(&oracle_priority_flags.to_le_bytes());
        packed.extend_from_slice(&serum_combined_address_raw);
        packed.extend_from_slice(&[0u8; SWAP_INFO_RESERVED_BYTES]);

        let unpacked = SwapInfo::unpack(&packed).unwrap();
        assert_eq!(swap_info, unpacked);

        let packed = [0u8; SwapInfo::LEN];
        let swap_info: SwapInfo = Default::default();
        let unpack_unchecked = SwapInfo::unpack_unchecked(&packed).unwrap();
        assert_eq!(unpack_unchecked, swap_info);
        let err = SwapInfo::unpack(&packed).unwrap_err();
        assert_eq!(err, ProgramError::UninitializedAccount);
    }

    #[test]
    fn test_user_referrer_data_packing() {
        let is_initialized = true;
        let config_key = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let referrer = Pubkey::new_unique();

        let user_referrer_data = UserReferrerData {
            is_initialized,
            config_key,
            owner,
            referrer,
        };

        let mut packed = [0u8; UserReferrerData::LEN];
        UserReferrerData::pack_into_slice(&user_referrer_data, &mut packed);

        let unpacked = UserReferrerData::unpack(&packed).unwrap();
        assert_eq!(user_referrer_data, unpacked);
    }

    #[test]
    fn test_check_swap_out_amount() {
        let amount_out = 1_000_000u64;
        let swap_direction = SwapDirection::SellBase;
        let mut token_swap = SwapInfo {
            swap_out_limit_percentage: 10u8,
            pool_state: PoolState {
                base_reserve: Decimal::from(5_000_000u64),
                quote_reserve: Decimal::from(12_000_000u64),
                ..PoolState::default()
            },
            ..SwapInfo::default()
        };
        assert_eq!(
            token_swap.check_swap_out_amount(amount_out, swap_direction),
            Ok(())
        );

        let swap_direction = SwapDirection::SellQuote;
        assert_eq!(
            token_swap.check_swap_out_amount(amount_out, swap_direction),
            Err(SwapError::ExceededSwapOutAmount.into())
        );

        token_swap.pool_state.quote_reserve = Decimal::from(10_000_000u64);
        assert_eq!(
            token_swap.check_swap_out_amount(amount_out, swap_direction),
            Err(SwapError::ExceededSwapOutAmount.into())
        );

        token_swap.swap_out_limit_percentage = 0u8;
        assert_eq!(
            token_swap.check_swap_out_amount(amount_out, swap_direction),
            Ok(())
        );
    }

    #[test]
    fn test_check_oracle_flags() {
        assert!(OraclePriorityFlag::from_bits_truncate(0b00).is_pyth_only());
        assert!(OraclePriorityFlag::from_bits_truncate(0b01).is_serum_only());
        assert_eq!(
            OraclePriorityFlag::from_bits_truncate(0b00),
            OraclePriorityFlag::PYTH_ONLY
        );
        assert_eq!(
            OraclePriorityFlag::from_bits_truncate(0b01),
            OraclePriorityFlag::SERUM_ONLY
        );
        assert_eq!(OraclePriorityFlag::from_bits(0b10), None);
    }
}
