use std::path::PathBuf;

use clap::Parser;
use jito_vault_core::vault::Vault;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    message::legacy, pubkey::Pubkey, signature::read_keypair_file, signer::Signer,
    transaction::Transaction,
};

use crate::{jito_restaking_program_id, jito_vault_program_id};

#[derive(Parser)]
#[command(about = "Create Token Metadata")]
pub struct CreateTokenMetadata {
    /// Path to keypair used to pay for account creation and execute transactions
    #[arg(short, long, env, default_value = "~/.config/solana/id.json")]
    keypair_path: PathBuf,

    /// Path to keypair for vault base
    #[arg(long, env, default_value = "~/.config/solana/id.json")]
    vault_base_keypair_path: PathBuf,

    /// Path to keypair used to pay for account creation and execute transactions
    #[arg(short, long, env, default_value = "~/.config/solana/id.json")]
    lrt_mint: Pubkey,
}

pub fn command_create_token_metadata(args: CreateTokenMetadata, client: RpcClient) {
    let jito_vault_program_id = jito_vault_program_id();
    // let jito_restaking_program_id = jito_restaking_program_id();
    let admin = read_keypair_file(args.keypair_path).expect("Fail to read keypair");

    let vault_base =
        read_keypair_file(args.vault_base_keypair_path).expect("Fail to read vault_base keypair");

    let seeds = vec![
        b"metadata".as_ref().to_vec(),
        args.lrt_mint.to_bytes().to_vec(),
    ];
    let seeds_iter: Vec<_> = seeds.iter().map(|s| s.as_slice()).collect();
    let (metadata_pubkey, _bump) = Pubkey::find_program_address(&seeds_iter, &jito_vault_program_id);

    let vault_pubkey = Vault::find_program_address(&jito_vault_program_id, &vault_base.pubkey()).0;

    let instruction = jito_vault_sdk::sdk::create_token_metadata(
        &jito_vault_program_id,
        &metadata_pubkey,
        &vault_pubkey,
        &admin.pubkey(),
        "restaking JTO".to_string(),
        "rJTO".to_string(),
        "https://www".to_string(),
    );

    let message = legacy::Message::new(&[instruction], Some(&admin.pubkey()));

    let blockhash = client
        .get_latest_blockhash()
        .expect("Fail to get blockhash");
    let tx = Transaction::new(&[&admin], message, blockhash);

    let sig = client
        .send_and_confirm_transaction(&tx)
        .expect("Fail to send transaction");

    println!("Sig: {sig}");

    // let res = client.get_account_data(&config_pubkey).expect("Fail to fetch config account");
    // let config = jito_vault_core::config::Config::load_bytes(&res).expect("Fail to convert Config");

    // println!("config bump: {}", config.bump());
}
