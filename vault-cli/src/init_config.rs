use std::path::PathBuf;

use clap::Parser;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    message::legacy, signature::read_keypair_file, signer::Signer, transaction::Transaction,
};

use crate::{jito_restaking_program_id, jito_vault_program_id};

#[derive(Parser)]
#[command(about = "Initialize config account")]
pub struct InitConfig {
    /// Path to keypair used to pay for account creation and execute transactions
    #[arg(short, long, env, default_value = "~/.config/solana/id.json")]
    keypair_path: PathBuf,
}

pub fn command_init_config(args: InitConfig, client: RpcClient) {
    let jito_vault_program_id = jito_vault_program_id();
    let jito_restaking_program_id = jito_restaking_program_id();
    let admin = read_keypair_file(args.keypair_path).expect("Fail to read keypair");

    let config_pubkey =
        jito_vault_core::config::Config::find_program_address(&jito_vault_program_id).0;

    let instruction = jito_vault_sdk::initialize_config(
        &jito_vault_program_id,
        &config_pubkey,
        &admin.pubkey(),
        &jito_restaking_program_id,
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
}
