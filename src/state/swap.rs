

use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};

use solana_program::{
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
};


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


#[cfg(test)]
mod tests {}
