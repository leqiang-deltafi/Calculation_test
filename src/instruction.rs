//! Instructions


use solana_program::{
    instruction::{AccountMeta, Instruction},
    program_error::ProgramError,
    pubkey::Pubkey,
};

/// Creates a 'mock_swap' instruction
pub fn mock_swap(
    program_id: Pubkey,
    mocked_swap_key: Pubkey,
    payer_key: Pubkey,
) -> Result<Instruction, ProgramError> {
    let data = vec!(1u8);

    let accounts = vec![
        AccountMeta::new_readonly(mocked_swap_key, false),
        AccountMeta::new_readonly(payer_key, true)
    ];

    Ok(Instruction {
        program_id,
        accounts,
        data,
    })
}