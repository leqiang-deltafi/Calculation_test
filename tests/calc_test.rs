#![cfg(feature = "test-bpf")]

mod utils;

use calculation_test::{
    // curve::calc::simple_powf, 
    processor::process,
    state::MockedSwap,
    instruction::mock_swap,
};
use solana_program_test::*;
use solana_program::msg;
// use solana_program::pubkey::Pubkey;

use solana_sdk::{
    account::Account,
    hash::hashv,
    signature::{read_keypair_file, Keypair},
    signer::Signer,
    system_instruction::{create_account, create_account_with_seed},
    transaction::Transaction,
};

use solana_program::{program_option::COption, program_pack::Pack, pubkey::Pubkey};

trait AddPacked {
    fn add_packable_account<T: Pack>(
        &mut self,
        pubkey: Pubkey,
        amount: u64,
        data: &T,
        owner: &Pubkey,
    );
}

impl AddPacked for ProgramTest {
    fn add_packable_account<T: Pack>(
        &mut self,
        pubkey: Pubkey,
        amount: u64,
        data: &T,
        owner: &Pubkey,
    ) {
        let mut account = Account::new(amount, T::get_packed_len(), owner);
        data.pack_into_slice(&mut account.data);
        self.add_account(pubkey, account);
    }
}

#[tokio::test]
async fn test_powf() {
    let mut test = ProgramTest::new("calculation_test", calculation_test::id(), processor!(process));

    let swap_config_keypair = Keypair::new();
    let swap_config_pubkey = swap_config_keypair.pubkey();
    test.add_packable_account(
        swap_config_pubkey,
        u32::MAX as u64,
        &MockedSwap {
            target_reserve_a: 100_001_423_523u64,
            target_reserve_b: 2_005_232_345_234u64,
            current_reserve_a: 2043u64,
            current_reserve_b: 996u64,
            market_price: 3u64,
        },
        &calculation_test::id()
    );

    // limit to track compute unit increase
    test.set_bpf_compute_max_units(30_000);
    let (mut banks_client, payer, recent_blockhash) = test.start().await;

    let mut transaction = Transaction::new_with_payer(
        &[mock_swap(
            calculation_test::id(),
            swap_config_pubkey,
            payer.pubkey()
        )
        .unwrap()],
        Some(&payer.pubkey()),
    );
    
    transaction.sign(&[&payer], recent_blockhash);

    banks_client
        .process_transaction(transaction)
        .await
        .map_err(|e| e.unwrap())
        .unwrap();

    // let result = simple_powf(2u64, 1001u64, 523u64, 2043u64, 996u64, 13u64);
    // println!("{}", result);
    // msg!("result {}", result);
    
    // assert_eq!(result, 1f64);
}