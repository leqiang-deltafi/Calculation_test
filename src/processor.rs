//! Program state processor

#![allow(clippy::too_many_arguments)]

// use std::{cmp::min, convert::TryInto, str::FromStr};

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    program_pack::Pack
};

use solana_program::msg;
use solana_program::pubkey::Pubkey;
use solana_program::entrypoint::ProgramResult;

use crate::{
    state::MockedSwap,
    curve::calc::simple_powf,
    math::Decimal
};

// use solana_program::pubkey::PubkeyError;
// use solana_program::{
//     account_info::{next_account_info, AccountInfo},
//     entrypoint::ProgramResult,
//     hash::hashv,
//     instruction::Instruction,
//     msg,
//     program::{invoke, invoke_signed},
//     program_error::ProgramError,
//     program_pack::{IsInitialized, Pack},
//     pubkey::Pubkey,
//     sysvar::{clock::Clock, rent::Rent, Sysvar},
// };
// use spl_token::{
//     self,
//     instruction::AuthorityType,
//     state::{Account, Mint},
// };

// use crate::{
//     admin::{is_admin, process_admin_instruction},
//     curve::{InitPoolStateParams, PoolState, SwapDirection},
//     error::SwapError,
//     instruction::{
//         DepositData, FarmDepositData, FarmInitializeData, FarmInstruction, FarmWithdrawData,
//         InitializeData, InstructionType, StableInitializeData, StableSwapInstruction, SwapData,
//         SwapInstruction, WithdrawData,
//     },
//     math::{Decimal, TryAdd, TryDiv, TryMul},
//     pyth::{self, PriceStatus},
//     state::{
//         ConfigInfo, FarmInfo, FarmPosition, FarmUser, OraclePriorityFlag, SwapInfo, SwapType,
//         UserReferrerData,
//     },
//     utils, DUMMY_REFERRER_ADDRESS, SERUM_DEX_V3_PROGRAM_ID,
// };


/// Processes an [Instruction](enum.Instruction.html).
pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], _input: &[u8]) -> ProgramResult {

    // match InstructionType::check(input) {
    //     Some(InstructionType::Admin) => process_admin_instruction(program_id, accounts, input),
    //     Some(InstructionType::Swap) => process_swap_instruction(program_id, accounts, input),
    //     Some(InstructionType::StableSwap) => {
    //         process_stable_swap_instruction(program_id, accounts, input)
    //     }
    //     Some(InstructionType::Farm) => process_farm_instruction(program_id, accounts, input),
    //     _ => Err(ProgramError::InvalidInstructionData),
    // }
    process_native_powf(program_id, accounts)
}

fn process_native_powf(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let mocked_swap_info = next_account_info(account_info_iter)?;
    let mocked_swap = MockedSwap::unpack(&mocked_swap_info.data.borrow())?;
    
    let result = simple_powf(
        Decimal::from(mocked_swap.market_price), 
        Decimal::from(mocked_swap.target_reserve_a), 
        Decimal::from(mocked_swap.target_reserve_b), 
        Decimal::from(mocked_swap.current_reserve_a), 
        Decimal::from(mocked_swap.current_reserve_b), 
        Decimal::from(13u64))?;
    
    msg!("result: {}", result);
    Ok(())
}