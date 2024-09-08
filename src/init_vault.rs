// use std::{path::PathBuf, str::FromStr};
//
// use clap::Parser;
// use jito_vault_core::vault::Vault;
// use solana_client::rpc_client::RpcClient;
// use solana_sdk::{
//     message::legacy, pubkey::Pubkey, signature::read_keypair_file, signer::Signer,
//     transaction::Transaction,
// };
//
// use crate::jito_vault_program_id;
//
// #[derive(Parser)]
// #[command(about = "Initialize vault account")]
// pub struct InitVault {
//     /// Path to keypair for vault base
//     #[arg(long, env, default_value = "~/.config/solana/id.json")]
//     vault_base_keypair_path: PathBuf,
//
//     /// Path to keypair for lrt mint
//     #[arg(short, long, env, default_value = "~/.config/solana/id.json")]
//     lrt_mint_keypair_path: PathBuf,
//
//     /// Path to keypair for vault admin
//     #[arg(long, env, default_value = "~/.config/solana/id.json")]
//     vault_admin_keypair_path: PathBuf,
//
//     /// Path to keypair for vault admin
//     #[arg(short, long, env)]
//     token_mint_pubkey: String,
// }
//
// pub fn command_init_vault(args: InitVault, client: RpcClient) {
//     let jito_vault_program_id = jito_vault_program_id();
//
//     let vault_base =
//         read_keypair_file(args.vault_base_keypair_path).expect("Fail to read vault_base keypair");
//     let vault_admin =
//         read_keypair_file(args.vault_admin_keypair_path).expect("Fail to read vault_admin keypair");
//     let lrt_mint =
//         read_keypair_file(args.lrt_mint_keypair_path).expect("Fail to read lrt_mint keypair");
//
//     let config_pubkey =
//         jito_vault_core::config::Config::find_program_address(&jito_vault_program_id).0;
//     let token_mint_pubkey =
//         Pubkey::from_str(&args.token_mint_pubkey).expect("Fail to read token_mint_pubkey");
//
//     let vault_pubkey = Vault::find_program_address(&jito_vault_program_id, &vault_base.pubkey()).0;
//     // let vault_delegation_list =
//     //     VaultDelegationList::find_program_address(&jito_vault_program_id, &vault_pubkey).0;
//
//     let instruction = jito_vault_sdk::sdk::initialize_vault(
//         &jito_vault_program_id,
//         &config_pubkey,
//         &vault_pubkey,
//         &lrt_mint.pubkey(),
//         &token_mint_pubkey,
//         &vault_admin.pubkey(),
//         &vault_base.pubkey(),
//         99,
//         100,
//     );
//
//     let message = legacy::Message::new(&[instruction], Some(&vault_admin.pubkey()));
//
//     let blockhash = client
//         .get_latest_blockhash()
//         .expect("Fail to get blockhash");
//     let tx = Transaction::new(&[&vault_admin, &lrt_mint, &vault_base], message, blockhash);
//
//     let sig = client
//         .send_and_confirm_transaction(&tx)
//         .expect("Fail to send transaction");
//
//     println!("Sig: {sig}");
// }
