// #![allow(dead_code)]
// #![allow(clippy::too_many_arguments)]

// use assert_matches::*;
// use deltafi_swap::{
//     curve::{InitPoolStateParams, PoolState},
//     instruction::{
//         deposit, farm_deposit, farm_initialize, farm_user_initialize, farm_withdraw, initialize,
//         initialize_config, set_referrer, stable_deposit, stable_initialize, stable_swap,
//         stable_swap_v2, stable_withdraw, swap, swap_v2, withdraw, DepositData, FarmDepositData,
//         FarmInitializeData, FarmWithdrawData, InitializeData, StableInitializeData, SwapData,
//         WithdrawData,
//     },
//     math::Decimal,
//     processor::get_farm_user_pubkey,
//     pyth,
//     state::{
//         ConfigInfo, FarmInfo, FarmPosition, FarmUser, Fees, OraclePriorityFlag, Rewards, SwapInfo,
//         SwapType, UserReferrerData, PROGRAM_VERSION,
//     },
//     SERUM_DEX_V3_PROGRAM_ID,
// };

// use solana_program::{program_option::COption, program_pack::Pack, pubkey::Pubkey};
// use solana_program_test::*;
// use solana_sdk::{
//     account::Account,
//     hash::hashv,
//     signature::{read_keypair_file, Keypair},
//     signer::Signer,
//     system_instruction::{create_account, create_account_with_seed},
//     transaction::Transaction,
// };
// use spl_token::{
//     instruction::{approve, initialize_account, initialize_mint, set_authority, AuthorityType},
//     native_mint::DECIMALS,
//     state::{Account as Token, AccountState, Mint},
// };
// use std::{convert::TryInto, str::FromStr};

// pub const LAMPORTS_TO_SOL: u64 = 1_000_000_000;
// pub const FRACTIONAL_TO_USDC: u64 = 1_000_000;

// pub const ZERO_TS: i64 = 0;

// pub const TEST_FEES: Fees = Fees {
//     is_initialized: true,
//     admin_trade_fee_numerator: 2,
//     admin_trade_fee_denominator: 5,
//     admin_withdraw_fee_numerator: 2,
//     admin_withdraw_fee_denominator: 5,
//     trade_fee_numerator: 5,
//     trade_fee_denominator: 1_000,
//     withdraw_fee_numerator: 2,
//     withdraw_fee_denominator: 100,
// };

// pub const TEST_REWARDS: Rewards = Rewards {
//     is_initialized: true,
//     decimals: 9,
//     reserved: [0u8; 7],
//     trade_reward_numerator: 1,
//     trade_reward_denominator: 1_000,
//     trade_reward_cap: 10_000_000_000,
// };

// pub const SOL_PYTH_PRODUCT: &str = "3Mnn2fX6rQyUsyELYms1sBJyChWofzSNRoqYzvgMVz5E";
// pub const SOL_PYTH_PRICE: &str = "J83w4HKfqxwcq3BEMMkPFSppX3gqekLyLJBexebFVkix";

// pub const SRM_PYTH_PRODUCT: &str = "6MEwdxe4g1NeAF9u6KDG14anJpFsVEa2cvr5H6iriFZ8";
// pub const SRM_PYTH_PRICE: &str = "992moaMQKs32GKZ9dxi8keyM2bUmbrwBZpK4p2K6X5Vs";

// pub const SRM_MINT: &str = "SRMuApVNdxXokk5GT7XD5cUUgXMBCoAz2LHeuAoKWRt";

// pub const USDC_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
// pub const USDT_MINT: &str = "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB";

// pub const SERUM_MARKET: &str = "jyei9Fpj2GtHLDDGgcuhDacxYLLiSyxU4TY7KxB2xai";
// pub const SERUM_BIDS: &str = "4ZTJfhgKPizbkFXNvTRNLEncqg85yJ6pyT7NVHBAgvGw";
// pub const SERUM_ASKS: &str = "7hLgwZhHD1MRNyiF1qfAjfkMzwvP3VxQMLLTJmKSp4Y3";

// pub const SRM_DECIMALS: u8 = 6;

// trait AddPacked {
//     fn add_packable_account<T: Pack>(
//         &mut self,
//         pubkey: Pubkey,
//         amount: u64,
//         data: &T,
//         owner: &Pubkey,
//     );
// }

// impl AddPacked for ProgramTest {
//     fn add_packable_account<T: Pack>(
//         &mut self,
//         pubkey: Pubkey,
//         amount: u64,
//         data: &T,
//         owner: &Pubkey,
//     ) {
//         let mut account = Account::new(amount, T::get_packed_len(), owner);
//         data.pack_into_slice(&mut account.data);
//         self.add_account(pubkey, account);
//     }
// }

// pub struct TestOracle {
//     pub product_pubkey: Pubkey,
//     pub price_pubkey: Pubkey,
//     pub price: Decimal,
// }

// pub struct TestMint {
//     pub pubkey: Pubkey,
//     pub authority: Keypair,
//     pub decimals: u8,
// }

// pub struct TestSerumOracle {
//     pub serum_market_pubkey: Pubkey,
//     pub serum_bids_pubkey: Pubkey,
//     pub serum_asks_pubkey: Pubkey,
// }

// pub fn add_swap_config(test: &mut ProgramTest) -> TestSwapConfig {
//     let swap_config_keypair = Keypair::new();
//     let swap_config_pubkey = swap_config_keypair.pubkey();
//     let (market_authority, bump_seed) =
//         Pubkey::find_program_address(&[swap_config_pubkey.as_ref()], &deltafi_swap::id());

//     let admin = read_keypair_file("tests/fixtures/deltafi-owner.json").unwrap();
//     let pyth_program_id = Pubkey::from_str(pyth::PYTH_PROGRAM_ID).unwrap();

//     let deltafi_mint = Pubkey::new_unique();
//     let deltafi_token = Pubkey::new_unique();
//     test.add_packable_account(
//         deltafi_mint,
//         u32::MAX as u64,
//         &Mint {
//             is_initialized: true,
//             decimals: DECIMALS,
//             mint_authority: COption::Some(market_authority),
//             freeze_authority: COption::Some(admin.pubkey()),
//             supply: 0,
//         },
//         &spl_token::id(),
//     );

//     test.add_packable_account(
//         deltafi_token,
//         u32::MAX as u64,
//         &Token {
//             mint: deltafi_mint,
//             owner: market_authority,
//             amount: 1000_000,
//             state: AccountState::Initialized,
//             ..Token::default()
//         },
//         &spl_token::id(),
//     );

//     test.add_packable_account(
//         swap_config_pubkey,
//         u32::MAX as u64,
//         &ConfigInfo {
//             version: PROGRAM_VERSION,
//             bump_seed,
//             admin_key: admin.pubkey(),
//             deltafi_mint,
//             pyth_program_id,
//             fees: TEST_FEES,
//             rewards: TEST_REWARDS,
//             deltafi_token: deltafi_token,
//             ..ConfigInfo::default()
//         },
//         &deltafi_swap::id(),
//     );

//     TestSwapConfig {
//         keypair: swap_config_keypair,
//         pubkey: swap_config_pubkey,
//         admin,
//         market_authority,
//         deltafi_mint,
//         pyth_program_id,
//         fees: TEST_FEES,
//         rewards: TEST_REWARDS,
//         deltafi_token: deltafi_token,
//     }
// }

// #[derive(Default)]
// pub struct AddSwapInfoArgs {
//     pub token_a_mint: Pubkey,
//     pub token_b_mint: Pubkey,
//     pub token_a_amount: u64,
//     pub token_b_amount: u64,
//     pub oracle_a: Pubkey,
//     pub oracle_b: Pubkey,
//     pub market_price: Decimal,
//     pub slope: Decimal,
//     pub last_market_price: Decimal,
//     pub last_valid_market_price_slot: u64,
//     pub serum_market: Pubkey,
//     pub serum_bids: Pubkey,
//     pub serum_asks: Pubkey,
//     pub swap_out_limit_percentage: u8,
//     pub oracle_priority_flags: u8,
// }

// pub fn add_swap_info(
//     swap_type: SwapType,
//     test: &mut ProgramTest,
//     swap_config: &TestSwapConfig,
//     user_account_owner: &Keypair,
//     admin_account_owner: &Keypair,
//     args: AddSwapInfoArgs,
// ) -> TestSwapInfo {
//     let AddSwapInfoArgs {
//         token_a_mint,
//         token_b_mint,
//         token_a_amount,
//         token_b_amount,
//         oracle_a,
//         oracle_b,
//         market_price,
//         slope,
//         last_market_price,
//         last_valid_market_price_slot,
//         serum_market,
//         serum_bids,
//         serum_asks,
//         swap_out_limit_percentage,
//         oracle_priority_flags,
//     } = args;

//     let serum_combined_address = Pubkey::new(
//         hashv(&[
//             serum_market.as_ref(),
//             serum_bids.as_ref(),
//             serum_asks.as_ref(),
//         ])
//         .as_ref(),
//     );

//     let mut pool_state = PoolState::new(InitPoolStateParams {
//         market_price,
//         slope,
//         base_reserve: Decimal::zero(),
//         quote_reserve: Decimal::zero(),
//         total_supply: 0,
//         last_market_price,
//         last_valid_market_price_slot,
//     });

//     let (pool_mint_amount, _, _) = pool_state
//         .buy_shares(token_a_amount, token_b_amount)
//         .unwrap();

//     let swap_info_keypair = Keypair::new();
//     let swap_info_pubkey = swap_info_keypair.pubkey();
//     let (swap_authority_pubkey, nonce) =
//         Pubkey::find_program_address(&[swap_info_pubkey.as_ref()], &deltafi_swap::id());

//     let pool_mint = Pubkey::new_unique();
//     test.add_packable_account(
//         pool_mint,
//         u32::MAX as u64,
//         &Mint {
//             is_initialized: true,
//             decimals: DECIMALS,
//             mint_authority: COption::Some(swap_authority_pubkey),
//             freeze_authority: COption::None,
//             supply: pool_mint_amount,
//             ..Mint::default()
//         },
//         &spl_token::id(),
//     );

//     let pool_token = Pubkey::new_unique();
//     test.add_packable_account(
//         pool_token,
//         u32::MAX as u64,
//         &Token {
//             mint: pool_mint,
//             owner: user_account_owner.pubkey(),
//             amount: pool_mint_amount,
//             state: AccountState::Initialized,
//             ..Token::default()
//         },
//         &spl_token::id(),
//     );

//     let token_a = Pubkey::new_unique();
//     test.add_packable_account(
//         token_a,
//         u32::MAX as u64,
//         &Token {
//             mint: token_a_mint,
//             owner: swap_authority_pubkey,
//             amount: token_a_amount,
//             state: AccountState::Initialized,
//             ..Token::default()
//         },
//         &spl_token::id(),
//     );

//     let token_b = Pubkey::new_unique();
//     test.add_packable_account(
//         token_b,
//         u32::MAX as u64,
//         &Token {
//             mint: token_b_mint,
//             owner: swap_authority_pubkey,
//             amount: token_b_amount,
//             state: AccountState::Initialized,
//             ..Token::default()
//         },
//         &spl_token::id(),
//     );

//     let admin_fee_a_key = Pubkey::new_unique();
//     test.add_packable_account(
//         admin_fee_a_key,
//         u32::MAX as u64,
//         &Token {
//             mint: token_a_mint,
//             owner: admin_account_owner.pubkey(),
//             amount: 0,
//             state: AccountState::Initialized,
//             ..Token::default()
//         },
//         &spl_token::id(),
//     );

//     let admin_fee_b_key = Pubkey::new_unique();
//     test.add_packable_account(
//         admin_fee_b_key,
//         u32::MAX as u64,
//         &Token {
//             mint: token_b_mint,
//             owner: admin_account_owner.pubkey(),
//             amount: 0,
//             state: AccountState::Initialized,
//             ..Token::default()
//         },
//         &spl_token::id(),
//     );

//     // For swap between SOl and SRM
//     // if the pool is Serum_only, its base_quote token oder has to be "SRM-SOL"
//     let (token_a_decimals, token_b_decimals) =
//         match OraclePriorityFlag::from_bits_truncate(oracle_priority_flags) {
//             OraclePriorityFlag::SERUM_ONLY => (SRM_DECIMALS, DECIMALS),
//             _ => (DECIMALS, SRM_DECIMALS),
//         };

//     let swap_info = SwapInfo {
//         is_initialized: true,
//         is_paused: false,
//         nonce,
//         swap_type,
//         config_key: swap_config.pubkey,
//         token_a,
//         token_b,
//         pyth_a: oracle_a,
//         pyth_b: oracle_b,
//         serum_combined_address,
//         pool_mint,
//         token_a_mint,
//         token_b_mint,
//         admin_fee_key_a: admin_fee_a_key,
//         admin_fee_key_b: admin_fee_b_key,
//         fees: swap_config.fees.clone(),
//         rewards: swap_config.rewards.clone(),
//         pool_state,
//         swap_out_limit_percentage,
//         oracle_priority_flags: oracle_priority_flags,
//         token_a_decimals,
//         token_b_decimals,
//         ..SwapInfo::default()
//     };

//     test.add_packable_account(
//         swap_info_pubkey,
//         u32::MAX as u64,
//         &swap_info,
//         &deltafi_swap::id(),
//     );

//     TestSwapInfo {
//         keypair: swap_info_keypair,
//         pubkey: swap_info_pubkey,
//         authority: swap_authority_pubkey,
//         nonce,
//         token_a,
//         token_b,
//         pool_token,
//         pool_mint,
//         token_a_mint,
//         token_b_mint,
//         admin_fee_a_key,
//         admin_fee_b_key,
//         fees: swap_config.fees.clone(),
//         rewards: swap_config.rewards.clone(),
//         oracle_a,
//         oracle_b,
//         serum_market,
//         serum_bids,
//         serum_asks,
//     }
// }

// pub fn add_farm_user(
//     test: &mut ProgramTest,
//     config_key: Pubkey,
//     farm_pool_key: Pubkey,
//     user_account_owner: &Keypair,
// ) -> TestFarmUser {
//     let farm_user_pubkey = Pubkey::new_unique();
//     test.add_packable_account(
//         farm_user_pubkey,
//         u32::MAX as u64,
//         &FarmUser {
//             is_initialized: true,
//             config_key,
//             farm_pool_key,
//             owner: user_account_owner.pubkey(),
//             position: FarmPosition {
//                 ..FarmPosition::default()
//             },
//             ..FarmUser::default()
//         },
//         &deltafi_swap::id(),
//     );

//     TestFarmUser {
//         pubkey: farm_user_pubkey,
//         config_key,
//         farm_pool_key,
//         owner: user_account_owner.pubkey(),
//         positions: vec![],
//     }
// }

// pub struct TestSwapConfig {
//     pub keypair: Keypair,
//     pub pubkey: Pubkey,
//     pub admin: Keypair,
//     pub market_authority: Pubkey,
//     pub deltafi_mint: Pubkey,
//     pub pyth_program_id: Pubkey,
//     pub fees: Fees,
//     pub rewards: Rewards,
//     pub deltafi_token: Pubkey,
// }

// impl TestSwapConfig {
//     pub async fn init(banks_client: &mut BanksClient, payer: &Keypair) -> Self {
//         let admin = read_keypair_file("tests/fixtures/deltafi-owner.json").unwrap();
//         let pyth_program_id = Pubkey::from_str(pyth::PYTH_PROGRAM_ID).unwrap();
//         let admin_pubkey = admin.pubkey();
//         let swap_config_keypair = Keypair::new();
//         let swap_config_pubkey = swap_config_keypair.pubkey();
//         let (market_authority_pubkey, _bump_seed) = Pubkey::find_program_address(
//             &[&swap_config_pubkey.to_bytes()[..32]],
//             &deltafi_swap::id(),
//         );
//         let deltafi_mint = Keypair::new();

//         let rent = banks_client.get_rent().await.unwrap();
//         let mut transaction = Transaction::new_with_payer(
//             &[
//                 create_account(
//                     &payer.pubkey(),
//                     &deltafi_mint.pubkey(),
//                     rent.minimum_balance(Mint::LEN),
//                     Mint::LEN as u64,
//                     &spl_token::id(),
//                 ),
//                 initialize_mint(
//                     &spl_token::id(),
//                     &deltafi_mint.pubkey(),
//                     &market_authority_pubkey,
//                     Some(&admin_pubkey),
//                     DECIMALS,
//                 )
//                 .unwrap(),
//             ],
//             Some(&payer.pubkey()),
//         );

//         let recent_blockhash = banks_client.get_recent_blockhash().await.unwrap();
//         transaction.sign(&[payer, &deltafi_mint], recent_blockhash);
//         assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));

//         let deltafi_token_account = create_and_mint_to_token_account(
//             banks_client,
//             deltafi_mint.pubkey(),
//             None,
//             &payer,
//             market_authority_pubkey,
//             1000_000,
//         )
//         .await;

//         let mut transaction = Transaction::new_with_payer(
//             &[
//                 create_account(
//                     &payer.pubkey(),
//                     &swap_config_pubkey,
//                     rent.minimum_balance(ConfigInfo::LEN),
//                     ConfigInfo::LEN as u64,
//                     &deltafi_swap::id(),
//                 ),
//                 initialize_config(
//                     deltafi_swap::id(),
//                     swap_config_pubkey,
//                     market_authority_pubkey,
//                     deltafi_mint.pubkey(),
//                     admin_pubkey,
//                     pyth_program_id,
//                     TEST_FEES,
//                     TEST_REWARDS,
//                     deltafi_token_account,
//                 )
//                 .unwrap(),
//             ],
//             Some(&payer.pubkey()),
//         );
//         let recent_blockhash = banks_client.get_recent_blockhash().await.unwrap();
//         transaction.sign(&[payer, &admin, &swap_config_keypair], recent_blockhash);
//         assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));

//         Self {
//             keypair: swap_config_keypair,
//             pubkey: swap_config_pubkey,
//             admin,
//             market_authority: market_authority_pubkey,
//             deltafi_mint: deltafi_mint.pubkey(),
//             pyth_program_id,
//             fees: TEST_FEES,
//             rewards: TEST_REWARDS,
//             deltafi_token: deltafi_token_account,
//         }
//     }

//     pub async fn get_state(&self, banks_client: &mut BanksClient) -> ConfigInfo {
//         let swap_config_account: Account = banks_client
//             .get_account(self.pubkey)
//             .await
//             .unwrap()
//             .unwrap();
//         ConfigInfo::unpack(&swap_config_account.data[..]).unwrap()
//     }

//     pub async fn validate_state(&self, banks_client: &mut BanksClient) {
//         let swap_config = self.get_state(banks_client).await;
//         assert_eq!(swap_config.version, PROGRAM_VERSION);
//         assert_eq!(swap_config.admin_key, self.admin.pubkey());
//         assert_eq!(swap_config.deltafi_mint, self.deltafi_mint);
//         assert_eq!(swap_config.fees, self.fees);
//         assert_eq!(swap_config.rewards, self.rewards);
//     }
// }

// pub struct TestSwapInfo {
//     pub keypair: Keypair,
//     pub pubkey: Pubkey,
//     pub authority: Pubkey,
//     pub nonce: u8,
//     pub token_a: Pubkey,
//     pub token_b: Pubkey,
//     pub pool_token: Pubkey,
//     pub pool_mint: Pubkey,
//     pub token_a_mint: Pubkey,
//     pub token_b_mint: Pubkey,
//     pub admin_fee_a_key: Pubkey,
//     pub admin_fee_b_key: Pubkey,
//     pub fees: Fees,
//     pub rewards: Rewards,
//     pub oracle_a: Pubkey,
//     pub oracle_b: Pubkey,
//     pub serum_market: Pubkey,
//     pub serum_bids: Pubkey,
//     pub serum_asks: Pubkey,
// }

// pub struct SwapInitArgs {
//     pub mid_price: u128,
//     pub slope: u64,
//     pub token_a_amount: u64,
//     pub token_b_amount: u64,
//     pub oracle_priority_flags: u8,
// }

// impl TestSwapInfo {
//     pub async fn init(
//         swap_type: SwapType,
//         banks_client: &mut BanksClient,
//         swap_config: &TestSwapConfig,
//         oracle_a: &TestOracle,
//         oracle_b: &TestOracle,
//         token_a_mint: Pubkey,
//         token_b_mint: Pubkey,
//         token_a: Pubkey,
//         token_b: Pubkey,
//         admin_fee_a_key: Pubkey,
//         admin_fee_b_key: Pubkey,
//         user_account_owner: &Keypair,
//         admin_keypair: &Keypair,
//         serum_market_pubkey: Pubkey,
//         serum_bids_pubkey: Pubkey,
//         serum_asks_pubkey: Pubkey,
//         payer: &Keypair,
//         args: &SwapInitArgs,
//     ) -> Self {
//         let swap_info_keypair = Keypair::new();
//         let swap_info_pubkey = swap_info_keypair.pubkey();

//         let (swap_authority_pubkey, nonce) = Pubkey::find_program_address(
//             &[&swap_info_pubkey.to_bytes()[..32]],
//             &deltafi_swap::id(),
//         );

//         let pool_mint_keypair = Keypair::new();
//         let user_pool_token_keypair = Keypair::new();

//         let rent = banks_client.get_rent().await.unwrap();
//         let mut transaction = Transaction::new_with_payer(
//             &[
//                 create_account(
//                     &payer.pubkey(),
//                     &pool_mint_keypair.pubkey(),
//                     rent.minimum_balance(Mint::LEN),
//                     Mint::LEN as u64,
//                     &spl_token::id(),
//                 ),
//                 initialize_mint(
//                     &spl_token::id(),
//                     &pool_mint_keypair.pubkey(),
//                     &swap_authority_pubkey,
//                     None,
//                     DECIMALS,
//                 )
//                 .unwrap(),
//                 create_account(
//                     &payer.pubkey(),
//                     &user_pool_token_keypair.pubkey(),
//                     rent.minimum_balance(Token::LEN),
//                     Token::LEN as u64,
//                     &spl_token::id(),
//                 ),
//                 initialize_account(
//                     &spl_token::id(),
//                     &user_pool_token_keypair.pubkey(),
//                     &pool_mint_keypair.pubkey(),
//                     &user_account_owner.pubkey(),
//                 )
//                 .unwrap(),
//                 set_authority(
//                     &spl_token::id(),
//                     &token_a,
//                     Some(&swap_authority_pubkey),
//                     AuthorityType::AccountOwner,
//                     &user_account_owner.pubkey(),
//                     &[],
//                 )
//                 .unwrap(),
//                 set_authority(
//                     &spl_token::id(),
//                     &token_b,
//                     Some(&swap_authority_pubkey),
//                     AuthorityType::AccountOwner,
//                     &user_account_owner.pubkey(),
//                     &[],
//                 )
//                 .unwrap(),
//                 create_account(
//                     &payer.pubkey(),
//                     &swap_info_pubkey,
//                     rent.minimum_balance(SwapInfo::LEN),
//                     SwapInfo::LEN as u64,
//                     &deltafi_swap::id(),
//                 ),
//                 match swap_type {
//                     SwapType::Normal => initialize(
//                         deltafi_swap::id(),
//                         swap_config.pubkey,
//                         swap_info_pubkey,
//                         swap_authority_pubkey,
//                         admin_fee_a_key,
//                         admin_fee_b_key,
//                         token_a,
//                         token_b,
//                         pool_mint_keypair.pubkey(),
//                         user_pool_token_keypair.pubkey(),
//                         oracle_a.product_pubkey,
//                         oracle_a.price_pubkey,
//                         oracle_b.product_pubkey,
//                         oracle_b.price_pubkey,
//                         admin_keypair.pubkey(),
//                         serum_market_pubkey,
//                         serum_bids_pubkey,
//                         serum_asks_pubkey,
//                         InitializeData {
//                             nonce,
//                             mid_price: args.mid_price,
//                             slope: args.slope,
//                             token_a_decimals: 9u8,
//                             token_b_decimals: 9u8,
//                             token_a_amount: args.token_a_amount,
//                             token_b_amount: args.token_b_amount,
//                             oracle_priority_flags: args.oracle_priority_flags,
//                         },
//                     )
//                     .unwrap(),
//                     SwapType::Stable => stable_initialize(
//                         deltafi_swap::id(),
//                         swap_config.pubkey,
//                         swap_info_pubkey,
//                         swap_authority_pubkey,
//                         admin_fee_a_key,
//                         admin_fee_b_key,
//                         token_a,
//                         token_b,
//                         pool_mint_keypair.pubkey(),
//                         user_pool_token_keypair.pubkey(),
//                         admin_keypair.pubkey(),
//                         StableInitializeData {
//                             nonce,
//                             slope: args.slope,
//                             token_a_decimals: 9u8,
//                             token_b_decimals: 9u8,
//                             token_a_amount: args.token_a_amount,
//                             token_b_amount: args.token_b_amount,
//                         },
//                     )
//                     .unwrap(),
//                 },
//             ],
//             Some(&payer.pubkey()),
//         );

//         let recent_blockhash = banks_client.get_recent_blockhash().await.unwrap();
//         transaction.sign(
//             &vec![
//                 payer,
//                 user_account_owner,
//                 &swap_info_keypair,
//                 &pool_mint_keypair,
//                 &user_pool_token_keypair,
//                 &admin_keypair,
//             ],
//             recent_blockhash,
//         );

//         assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));

//         Self {
//             keypair: swap_info_keypair,
//             pubkey: swap_info_pubkey,
//             authority: swap_authority_pubkey,
//             nonce,
//             token_a,
//             token_b,
//             pool_token: user_pool_token_keypair.pubkey(),
//             pool_mint: pool_mint_keypair.pubkey(),
//             admin_fee_a_key,
//             admin_fee_b_key,
//             token_a_mint,
//             token_b_mint,
//             fees: swap_config.fees.clone(),
//             rewards: swap_config.rewards.clone(),
//             oracle_a: oracle_a.price_pubkey,
//             oracle_b: oracle_b.price_pubkey,
//             serum_market: serum_market_pubkey,
//             serum_bids: serum_bids_pubkey,
//             serum_asks: serum_asks_pubkey,
//         }
//     }

//     pub async fn set_referrer(
//         &self,
//         banks_client: &mut BanksClient,
//         config_info: &TestSwapConfig,
//         user_account_owner: &Keypair,
//         user_referrer_data_pubkey: Pubkey,
//         referrer_token_pubkey: Pubkey,
//         payer: &Keypair,
//     ) {
//         let joint_key = format!("referrer{}", config_info.pubkey);
//         let rent = banks_client.get_rent().await.unwrap();
//         let mut transaction = Transaction::new_with_payer(
//             &[
//                 create_account_with_seed(
//                     &payer.pubkey(),
//                     &user_referrer_data_pubkey,
//                     &user_account_owner.pubkey(),
//                     &joint_key.as_str()[0..32],
//                     rent.minimum_balance(UserReferrerData::LEN),
//                     UserReferrerData::LEN as u64,
//                     &deltafi_swap::id(),
//                 ),
//                 set_referrer(
//                     deltafi_swap::id(),
//                     config_info.pubkey,
//                     user_account_owner.pubkey(),
//                     user_referrer_data_pubkey,
//                     referrer_token_pubkey,
//                 )
//                 .unwrap(),
//             ],
//             Some(&payer.pubkey()),
//         );

//         let recent_blockhash = banks_client.get_recent_blockhash().await.unwrap();
//         transaction.sign(&[payer, user_account_owner], recent_blockhash);

//         assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));
//     }

//     pub async fn swap(
//         &self,
//         swap_type: SwapType,
//         banks_client: &mut BanksClient,
//         config_info: &TestSwapConfig,
//         user_account_owner: &Keypair,
//         source_pubkey: Pubkey,
//         source_mint_pubkey: Pubkey,
//         destination_pubkey: Pubkey,
//         destination_mint_pubkey: Pubkey,
//         reward_token_pubkey: Pubkey,
//         amount_in: u64,
//         minimum_amount_out: u64,
//         payer: &Keypair,
//         user_referrer_data_pubkey: Option<Pubkey>,
//         referral_pubkey: Option<Pubkey>,
//     ) {
//         let user_transfer_authority = Keypair::new();
//         let mut transaction = Transaction::new_with_payer(
//             &[
//                 approve(
//                     &spl_token::id(),
//                     &source_pubkey,
//                     &user_transfer_authority.pubkey(),
//                     &user_account_owner.pubkey(),
//                     &[],
//                     amount_in,
//                 )
//                 .unwrap(),
//                 match swap_type {
//                     SwapType::Normal => swap(
//                         deltafi_swap::id(),
//                         config_info.pubkey,
//                         self.pubkey,
//                         config_info.market_authority,
//                         self.authority,
//                         user_transfer_authority.pubkey(),
//                         source_pubkey,
//                         self.token_a,
//                         source_mint_pubkey,
//                         self.token_b,
//                         destination_pubkey,
//                         destination_mint_pubkey,
//                         reward_token_pubkey,
//                         config_info.deltafi_token,
//                         self.admin_fee_b_key,
//                         self.oracle_a,
//                         self.oracle_b,
//                         user_referrer_data_pubkey,
//                         referral_pubkey,
//                         SwapData {
//                             amount_in,
//                             minimum_amount_out,
//                         },
//                     )
//                     .unwrap(),
//                     SwapType::Stable => stable_swap(
//                         deltafi_swap::id(),
//                         config_info.pubkey,
//                         self.pubkey,
//                         config_info.market_authority,
//                         self.authority,
//                         user_transfer_authority.pubkey(),
//                         source_pubkey,
//                         self.token_a,
//                         source_mint_pubkey,
//                         self.token_b,
//                         destination_pubkey,
//                         destination_mint_pubkey,
//                         reward_token_pubkey,
//                         config_info.deltafi_token,
//                         self.admin_fee_b_key,
//                         user_referrer_data_pubkey,
//                         referral_pubkey,
//                         SwapData {
//                             amount_in,
//                             minimum_amount_out,
//                         },
//                     )
//                     .unwrap(),
//                 },
//             ],
//             Some(&payer.pubkey()),
//         );

//         let recent_blockhash = banks_client.get_recent_blockhash().await.unwrap();
//         transaction.sign(
//             &[payer, user_account_owner, &user_transfer_authority],
//             recent_blockhash,
//         );

//         assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));
//     }

//     pub async fn swap_v2(
//         &self,
//         swap_type: SwapType,
//         banks_client: &mut BanksClient,
//         config_info: &TestSwapConfig,
//         user_account_owner: &Keypair,
//         source_pubkey: Pubkey,
//         destination_pubkey: Pubkey,
//         reward_token_pubkey: Pubkey,
//         amount_in: u64,
//         minimum_amount_out: u64,
//         payer: &Keypair,
//         user_referrer_data_pubkey: Option<Pubkey>,
//         referral_pubkey: Option<Pubkey>,
//     ) {
//         let user_transfer_authority = Keypair::new();
//         let mut transaction = Transaction::new_with_payer(
//             &[
//                 approve(
//                     &spl_token::id(),
//                     &source_pubkey,
//                     &user_transfer_authority.pubkey(),
//                     &user_account_owner.pubkey(),
//                     &[],
//                     amount_in,
//                 )
//                 .unwrap(),
//                 match swap_type {
//                     SwapType::Normal => swap_v2(
//                         deltafi_swap::id(),
//                         config_info.pubkey,
//                         self.pubkey,
//                         config_info.market_authority,
//                         self.authority,
//                         user_transfer_authority.pubkey(),
//                         source_pubkey,
//                         self.token_a,
//                         self.token_b,
//                         destination_pubkey,
//                         reward_token_pubkey,
//                         config_info.deltafi_token,
//                         self.admin_fee_b_key,
//                         self.oracle_a,
//                         self.oracle_b,
//                         self.serum_market,
//                         self.serum_bids,
//                         self.serum_asks,
//                         user_referrer_data_pubkey,
//                         referral_pubkey,
//                         SwapData {
//                             amount_in,
//                             minimum_amount_out,
//                         },
//                     )
//                     .unwrap(),
//                     SwapType::Stable => stable_swap_v2(
//                         deltafi_swap::id(),
//                         config_info.pubkey,
//                         self.pubkey,
//                         config_info.market_authority,
//                         self.authority,
//                         user_transfer_authority.pubkey(),
//                         source_pubkey,
//                         self.token_a,
//                         self.token_b,
//                         destination_pubkey,
//                         reward_token_pubkey,
//                         config_info.deltafi_token,
//                         self.admin_fee_b_key,
//                         user_referrer_data_pubkey,
//                         referral_pubkey,
//                         SwapData {
//                             amount_in,
//                             minimum_amount_out,
//                         },
//                     )
//                     .unwrap(),
//                 },
//             ],
//             Some(&payer.pubkey()),
//         );

//         let recent_blockhash = banks_client.get_recent_blockhash().await.unwrap();
//         transaction.sign(
//             &[payer, user_account_owner, &user_transfer_authority],
//             recent_blockhash,
//         );

//         assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));
//     }

//     pub async fn deposit(
//         &self,
//         swap_type: SwapType,
//         banks_client: &mut BanksClient,
//         user_account_owner: &Keypair,
//         deposit_token_a_pubkey: Pubkey,
//         deposit_token_b_pubkey: Pubkey,
//         pool_token_pubkey: Pubkey,
//         token_a_amount: u64,
//         token_b_amount: u64,
//         min_mint_amount: u64,
//         payer: &Keypair,
//     ) {
//         let user_transfer_authority = Keypair::new();
//         let mut transaction = Transaction::new_with_payer(
//             &[
//                 approve(
//                     &spl_token::id(),
//                     &deposit_token_a_pubkey,
//                     &user_transfer_authority.pubkey(),
//                     &user_account_owner.pubkey(),
//                     &[],
//                     token_a_amount,
//                 )
//                 .unwrap(),
//                 approve(
//                     &spl_token::id(),
//                     &deposit_token_b_pubkey,
//                     &user_transfer_authority.pubkey(),
//                     &user_account_owner.pubkey(),
//                     &[],
//                     token_b_amount,
//                 )
//                 .unwrap(),
//                 match swap_type {
//                     SwapType::Normal => deposit(
//                         deltafi_swap::id(),
//                         self.pubkey,
//                         self.authority,
//                         user_transfer_authority.pubkey(),
//                         deposit_token_a_pubkey,
//                         deposit_token_b_pubkey,
//                         self.token_a,
//                         self.token_b,
//                         self.pool_mint,
//                         pool_token_pubkey,
//                         DepositData {
//                             token_a_amount,
//                             token_b_amount,
//                             min_mint_amount,
//                         },
//                     )
//                     .unwrap(),
//                     SwapType::Stable => stable_deposit(
//                         deltafi_swap::id(),
//                         self.pubkey,
//                         self.authority,
//                         user_transfer_authority.pubkey(),
//                         deposit_token_a_pubkey,
//                         deposit_token_b_pubkey,
//                         self.token_a,
//                         self.token_b,
//                         self.pool_mint,
//                         pool_token_pubkey,
//                         DepositData {
//                             token_a_amount,
//                             token_b_amount,
//                             min_mint_amount,
//                         },
//                     )
//                     .unwrap(),
//                 },
//             ],
//             Some(&payer.pubkey()),
//         );

//         let recent_blockhash = banks_client.get_recent_blockhash().await.unwrap();
//         transaction.sign(
//             &[payer, user_account_owner, &user_transfer_authority],
//             recent_blockhash,
//         );

//         assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));
//     }

//     pub async fn withdraw(
//         &self,
//         swap_type: SwapType,
//         banks_client: &mut BanksClient,
//         user_account_owner: &Keypair,
//         token_a_pubkey: Pubkey,
//         token_b_pubkey: Pubkey,
//         pool_token_pubkey: Pubkey,
//         pool_token_amount: u64,
//         minimum_token_a_amount: u64,
//         minimum_token_b_amount: u64,
//         payer: &Keypair,
//     ) {
//         let user_transfer_authority = Keypair::new();
//         let mut transaction = Transaction::new_with_payer(
//             &[
//                 approve(
//                     &spl_token::id(),
//                     &pool_token_pubkey,
//                     &user_transfer_authority.pubkey(),
//                     &user_account_owner.pubkey(),
//                     &[],
//                     pool_token_amount,
//                 )
//                 .unwrap(),
//                 match swap_type {
//                     SwapType::Normal => withdraw(
//                         deltafi_swap::id(),
//                         self.pubkey,
//                         self.authority,
//                         user_transfer_authority.pubkey(),
//                         self.pool_mint,
//                         pool_token_pubkey,
//                         self.token_a,
//                         self.token_b,
//                         token_a_pubkey,
//                         token_b_pubkey,
//                         self.admin_fee_a_key,
//                         self.admin_fee_b_key,
//                         WithdrawData {
//                             pool_token_amount,
//                             minimum_token_a_amount,
//                             minimum_token_b_amount,
//                         },
//                     )
//                     .unwrap(),
//                     SwapType::Stable => stable_withdraw(
//                         deltafi_swap::id(),
//                         self.pubkey,
//                         self.authority,
//                         user_transfer_authority.pubkey(),
//                         self.pool_mint,
//                         pool_token_pubkey,
//                         self.token_a,
//                         self.token_b,
//                         token_a_pubkey,
//                         token_b_pubkey,
//                         self.admin_fee_a_key,
//                         self.admin_fee_b_key,
//                         WithdrawData {
//                             pool_token_amount,
//                             minimum_token_a_amount,
//                             minimum_token_b_amount,
//                         },
//                     )
//                     .unwrap(),
//                 },
//             ],
//             Some(&payer.pubkey()),
//         );

//         let recent_blockhash = banks_client.get_recent_blockhash().await.unwrap();
//         transaction.sign(
//             &[payer, user_account_owner, &user_transfer_authority],
//             recent_blockhash,
//         );

//         assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));
//     }

//     pub async fn get_state(&self, banks_client: &mut BanksClient) -> SwapInfo {
//         let swap_account: Account = banks_client
//             .get_account(self.pubkey)
//             .await
//             .unwrap()
//             .unwrap();
//         SwapInfo::unpack(&swap_account.data[..]).unwrap()
//     }

//     pub async fn validate_state(&self, banks_client: &mut BanksClient) {
//         let swap_info = self.get_state(banks_client).await;
//         assert!(swap_info.is_initialized);
//         assert_eq!(swap_info.token_a, self.token_a);
//         assert_eq!(swap_info.token_b, self.token_b);
//         assert_eq!(swap_info.admin_fee_key_a, self.admin_fee_a_key);
//         assert_eq!(swap_info.admin_fee_key_b, self.admin_fee_b_key);
//         assert_eq!(swap_info.token_a_mint, self.token_a_mint);
//         assert_eq!(swap_info.token_b_mint, self.token_b_mint);
//         assert_eq!(swap_info.fees, self.fees);
//         assert_eq!(swap_info.rewards, self.rewards);
//     }
// }

// pub struct TestFarmPoolInfo {
//     pub farm_pool_key: Pubkey,
//     pub authority: Pubkey,
//     pub bump_seed: u8,
//     pub farm_pool_token: Pubkey,
// }

// impl TestFarmPoolInfo {
//     pub async fn init(
//         banks_client: &mut BanksClient,
//         swap_config: &TestSwapConfig,
//         swap_info: &TestSwapInfo,
//         payer: &Keypair,
//         fee_numerator: u64,
//         fee_denominator: u64,
//         rewards_numerator: u64,
//         rewards_denominator: u64,
//     ) -> Self {
//         let rent = banks_client.get_rent().await.unwrap();

//         let farm_pool_keypair = Keypair::new();
//         let farm_pool_key = farm_pool_keypair.pubkey();
//         let (authority, bump_seed) =
//             Pubkey::find_program_address(&[farm_pool_key.as_ref()], &deltafi_swap::id());

//         let farm_pool_token = create_and_mint_to_token_account(
//             banks_client,
//             swap_info.pool_mint,
//             None,
//             &payer,
//             authority,
//             0,
//         )
//         .await;

//         let mut transaction = Transaction::new_with_payer(
//             &[
//                 create_account(
//                     &payer.pubkey(),
//                     &farm_pool_key,
//                     rent.minimum_balance(FarmInfo::LEN),
//                     FarmInfo::LEN as u64,
//                     &deltafi_swap::id(),
//                 ),
//                 farm_initialize(
//                     deltafi_swap::id(),
//                     swap_config.pubkey,
//                     swap_info.pubkey,
//                     farm_pool_key,
//                     authority,
//                     farm_pool_token,
//                     swap_config.admin.pubkey(),
//                     FarmInitializeData {
//                         fee_numerator,
//                         fee_denominator,
//                         rewards_numerator,
//                         rewards_denominator,
//                         bump_seed,
//                     },
//                 )
//                 .unwrap(),
//             ],
//             Some(&payer.pubkey()),
//         );
//         let recent_blockhash = banks_client.get_recent_blockhash().await.unwrap();
//         transaction.sign(
//             &[payer, &farm_pool_keypair, &swap_config.admin],
//             recent_blockhash,
//         );
//         assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));

//         Self {
//             farm_pool_key,
//             authority,
//             bump_seed,
//             farm_pool_token,
//         }
//     }
// }

// pub struct TestFarmUser {
//     pub pubkey: Pubkey,
//     pub config_key: Pubkey,
//     pub farm_pool_key: Pubkey,
//     pub owner: Pubkey,
//     pub positions: Vec<FarmPosition>,
// }

// impl TestFarmUser {
//     pub async fn init(
//         banks_client: &mut BanksClient,
//         config_key: Pubkey,
//         farm_pool_key: Pubkey,
//         user_account_owner: &Keypair,
//         payer: &Keypair,
//     ) -> Self {
//         let joint_key = format!("farmUser{}", farm_pool_key);
//         let farm_user_pubkey = get_farm_user_pubkey(
//             &user_account_owner.pubkey(),
//             &farm_pool_key,
//             &deltafi_swap::id(),
//         )
//         .unwrap();

//         let rent = banks_client.get_rent().await.unwrap();
//         let mut transaction = Transaction::new_with_payer(
//             &[
//                 create_account_with_seed(
//                     &payer.pubkey(),
//                     &farm_user_pubkey,
//                     &user_account_owner.pubkey(),
//                     &joint_key.as_str()[0..32],
//                     rent.minimum_balance(FarmUser::LEN),
//                     FarmUser::LEN as u64,
//                     &deltafi_swap::id(),
//                 ),
//                 farm_user_initialize(
//                     deltafi_swap::id(),
//                     config_key,
//                     farm_pool_key,
//                     farm_user_pubkey,
//                     user_account_owner.pubkey(),
//                 )
//                 .unwrap(),
//             ],
//             Some(&payer.pubkey()),
//         );

//         let recent_blockhash = banks_client.get_recent_blockhash().await.unwrap();
//         transaction.sign(&vec![payer, user_account_owner], recent_blockhash);

//         assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));

//         Self {
//             pubkey: farm_user_pubkey,
//             config_key,
//             farm_pool_key,
//             owner: user_account_owner.pubkey(),
//             positions: vec![],
//         }
//     }

//     pub async fn get_state(&self, banks_client: &mut BanksClient) -> FarmUser {
//         let liquidity_provider: Account = banks_client
//             .get_account(self.pubkey)
//             .await
//             .unwrap()
//             .unwrap();
//         FarmUser::unpack(&liquidity_provider.data[..]).unwrap()
//     }

//     pub async fn validate_state(&self, banks_client: &mut BanksClient) {
//         let farm_user = self.get_state(banks_client).await;
//         assert!(farm_user.is_initialized);
//         assert_eq!(farm_user.owner, self.owner);
//     }

//     pub async fn do_farm_deposit(
//         &self,
//         banks_client: &mut BanksClient,
//         user_account_owner: &Keypair,
//         source_token_pubkey: Pubkey,
//         pool_token_pubkey: Pubkey,
//         amount: u64,
//         payer: &Keypair,
//     ) {
//         let user_transfer_authority = Keypair::new();
//         let mut transaction = Transaction::new_with_payer(
//             &[
//                 approve(
//                     &spl_token::id(),
//                     &source_token_pubkey,
//                     &user_transfer_authority.pubkey(),
//                     &user_account_owner.pubkey(),
//                     &[],
//                     amount,
//                 )
//                 .unwrap(),
//                 farm_deposit(
//                     deltafi_swap::id(),
//                     self.config_key,
//                     self.farm_pool_key,
//                     user_transfer_authority.pubkey(),
//                     source_token_pubkey,
//                     pool_token_pubkey,
//                     self.pubkey,
//                     user_account_owner.pubkey(),
//                     FarmDepositData { amount },
//                 )
//                 .unwrap(),
//             ],
//             Some(&payer.pubkey()),
//         );

//         let recent_blockhash = banks_client.get_recent_blockhash().await.unwrap();
//         transaction.sign(
//             &[payer, user_account_owner, &user_transfer_authority],
//             recent_blockhash,
//         );

//         assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));
//     }

//     pub async fn do_farm_withdraw(
//         &self,
//         banks_client: &mut BanksClient,
//         user_account_owner: &Keypair,
//         user_token_pubkey: Pubkey,
//         pool_token_pubkey: Pubkey,
//         authority: Pubkey,
//         amount: u64,
//         payer: &Keypair,
//     ) {
//         let mut transaction = Transaction::new_with_payer(
//             &[farm_withdraw(
//                 deltafi_swap::id(),
//                 self.config_key,
//                 self.farm_pool_key,
//                 self.pubkey,
//                 authority,
//                 pool_token_pubkey,
//                 user_token_pubkey,
//                 user_account_owner.pubkey(),
//                 FarmWithdrawData { amount },
//             )
//             .unwrap()],
//             Some(&payer.pubkey()),
//         );

//         let recent_blockhash = banks_client.get_recent_blockhash().await.unwrap();
//         transaction.sign(&[payer, user_account_owner], recent_blockhash);

//         assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));
//     }
// }

// pub async fn create_and_mint_to_token_account(
//     banks_client: &mut BanksClient,
//     mint_pubkey: Pubkey,
//     mint_authority: Option<&Keypair>,
//     payer: &Keypair,
//     authority: Pubkey,
//     amount: u64,
// ) -> Pubkey {
//     if let Some(mint_authority) = mint_authority {
//         let account_pubkey =
//             create_token_account(banks_client, mint_pubkey, &payer, Some(authority), None).await;

//         mint_to(
//             banks_client,
//             mint_pubkey,
//             &payer,
//             account_pubkey,
//             mint_authority,
//             amount,
//         )
//         .await;

//         account_pubkey
//     } else {
//         create_token_account(
//             banks_client,
//             mint_pubkey,
//             &payer,
//             Some(authority),
//             Some(amount),
//         )
//         .await
//     }
// }

// pub async fn create_token_account(
//     banks_client: &mut BanksClient,
//     mint_pubkey: Pubkey,
//     payer: &Keypair,
//     authority: Option<Pubkey>,
//     native_amount: Option<u64>,
// ) -> Pubkey {
//     let token_keypair = Keypair::new();
//     let token_pubkey = token_keypair.pubkey();
//     let authority_pubkey = authority.unwrap_or_else(|| payer.pubkey());

//     let rent = banks_client.get_rent().await.unwrap();
//     let lamports = rent.minimum_balance(Token::LEN) + native_amount.unwrap_or_default();
//     let mut transaction = Transaction::new_with_payer(
//         &[
//             create_account(
//                 &payer.pubkey(),
//                 &token_pubkey,
//                 lamports,
//                 Token::LEN as u64,
//                 &spl_token::id(),
//             ),
//             spl_token::instruction::initialize_account(
//                 &spl_token::id(),
//                 &token_pubkey,
//                 &mint_pubkey,
//                 &authority_pubkey,
//             )
//             .unwrap(),
//         ],
//         Some(&payer.pubkey()),
//     );

//     let recent_blockhash = banks_client.get_recent_blockhash().await.unwrap();
//     transaction.sign(&[&payer, &token_keypair], recent_blockhash);

//     assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));

//     token_pubkey
// }

// pub async fn mint_to(
//     banks_client: &mut BanksClient,
//     mint_pubkey: Pubkey,
//     payer: &Keypair,
//     account_pubkey: Pubkey,
//     authority: &Keypair,
//     amount: u64,
// ) {
//     let mut transaction = Transaction::new_with_payer(
//         &[spl_token::instruction::mint_to(
//             &spl_token::id(),
//             &mint_pubkey,
//             &account_pubkey,
//             &authority.pubkey(),
//             &[],
//             amount,
//         )
//         .unwrap()],
//         Some(&payer.pubkey()),
//     );

//     let recent_blockhash = banks_client.get_recent_blockhash().await.unwrap();
//     transaction.sign(&[payer, authority], recent_blockhash);

//     assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));
// }

// pub async fn get_token_balance(banks_client: &mut BanksClient, pubkey: Pubkey) -> u64 {
//     let token: Account = banks_client.get_account(pubkey).await.unwrap().unwrap();

//     spl_token::state::Account::unpack(&token.data[..])
//         .unwrap()
//         .amount
// }

// pub fn add_oracle(
//     test: &mut ProgramTest,
//     product_pubkey: Pubkey,
//     price_pubkey: Pubkey,
//     price: Decimal,
// ) -> TestOracle {
//     let pyth_program_id = Pubkey::from_str(pyth::PYTH_PROGRAM_ID).unwrap();
//     // Add Pyth product account
//     test.add_account_with_file_data(
//         product_pubkey,
//         u32::MAX as u64,
//         pyth_program_id,
//         &format!("{}.bin", product_pubkey.to_string()),
//     );

//     // Add Pyth price account after setting the price
//     let filename = &format!("{}.bin", price_pubkey.to_string());
//     let mut pyth_price_data = read_file(find_file(filename).unwrap_or_else(|| {
//         panic!("Unable to locate {}", filename);
//     }));

//     let mut pyth_price = pyth::load_mut::<pyth::Price>(pyth_price_data.as_mut_slice()).unwrap();

//     let decimals = 10u64
//         .checked_pow(pyth_price.expo.checked_abs().unwrap().try_into().unwrap())
//         .unwrap();

//     pyth_price.valid_slot = 0;
//     pyth_price.agg.price = price
//         .try_round_u64()
//         .unwrap()
//         .checked_mul(decimals)
//         .unwrap()
//         .try_into()
//         .unwrap();

//     pyth_price.prev_price = pyth_price.agg.price;

//     test.add_account(
//         price_pubkey,
//         Account {
//             lamports: u32::MAX as u64,
//             data: pyth_price_data,
//             owner: pyth_program_id,
//             executable: false,
//             rent_epoch: 0,
//         },
//     );

//     TestOracle {
//         product_pubkey,
//         price_pubkey,
//         price,
//     }
// }

// pub fn add_sol_oracle(test: &mut ProgramTest) -> TestOracle {
//     add_oracle(
//         test,
//         Pubkey::from_str(SOL_PYTH_PRODUCT).unwrap(),
//         Pubkey::from_str(SOL_PYTH_PRICE).unwrap(),
//         // Set SOL price to $150
//         Decimal::from(150u64),
//     )
// }

// pub fn add_srm_oracle(test: &mut ProgramTest) -> TestOracle {
//     add_oracle(
//         test,
//         Pubkey::from_str(SRM_PYTH_PRODUCT).unwrap(),
//         Pubkey::from_str(SRM_PYTH_PRICE).unwrap(),
//         // Set USDC price to $7
//         Decimal::from(7u64),
//     )
// }

// pub fn add_token_mint(test: &mut ProgramTest, mint: &str, decimals: u8) -> TestMint {
//     let authority = Keypair::new();
//     let pubkey = Pubkey::from_str(mint).unwrap();
//     let admin = read_keypair_file("tests/fixtures/deltafi-owner.json").unwrap();

//     test.add_packable_account(
//         pubkey,
//         u32::MAX as u64,
//         &Mint {
//             is_initialized: true,
//             decimals,
//             mint_authority: COption::Some(authority.pubkey()),
//             freeze_authority: COption::Some(admin.pubkey()),
//             supply: 0,
//         },
//         &spl_token::id(),
//     );

//     TestMint {
//         pubkey,
//         authority,
//         decimals,
//     }
// }

// pub fn add_srm_mint(test: &mut ProgramTest) -> TestMint {
//     return add_token_mint(test, SRM_MINT, SRM_DECIMALS);
// }

// pub fn add_new_mint(test: &mut ProgramTest, decimals: u8) -> TestMint {
//     add_token_mint(test, &Pubkey::new_unique().to_string(), decimals)
// }

// pub fn add_serum_market(
//     test: &mut ProgramTest,
//     market_pubkey: Pubkey,
//     bids_pubkey: Pubkey,
//     asks_pubkey: Pubkey,
// ) -> (Pubkey, Pubkey, Pubkey) {
//     let serum_dex_program_id = Pubkey::from_str(SERUM_DEX_V3_PROGRAM_ID).unwrap();

//     // Add Serum market account for SRM-SOL
//     // Market account data is obtained from online data
//     // where
//     // baseLotSize = 10000, quoteLotSize = 10000, base_pricelot = 24,
//     // quote_pricelot = 26, baseDecimals = 6, quoteDecimals = 9
//     //
//     // SRM/SOL market_price = (priceLots * quoteLotSize * baseSplTokenMultiplier) / (baseLotSize * quoteSplTokenMultiplier)
//     //                      = ( 25 * 10000 * 10^6 ) / (10000 * 10^9 ) = 0.025
//     test.add_account_with_file_data(
//         market_pubkey,
//         u32::MAX as u64,
//         serum_dex_program_id,
//         &format!("{}.bin", market_pubkey.to_string()),
//     );

//     // Add Serum bids account
//     test.add_account_with_file_data(
//         bids_pubkey,
//         u32::MAX as u64,
//         serum_dex_program_id,
//         &format!("{}.bin", bids_pubkey.to_string()),
//     );

//     // Add Serum asks account
//     test.add_account_with_file_data(
//         asks_pubkey,
//         u32::MAX as u64,
//         serum_dex_program_id,
//         &format!("{}.bin", asks_pubkey.to_string()),
//     );

//     (market_pubkey, bids_pubkey, asks_pubkey)
// }

// pub fn add_srm_sol_serum_market(test: &mut ProgramTest) -> (Pubkey, Pubkey, Pubkey) {
//     let market_pubkey = Pubkey::from_str(SERUM_MARKET).unwrap();
//     let bids_pubkey = Pubkey::from_str(SERUM_BIDS).unwrap();
//     let asks_pubkey = Pubkey::from_str(SERUM_ASKS).unwrap();
//     add_serum_market(test, market_pubkey, bids_pubkey, asks_pubkey)
// }
