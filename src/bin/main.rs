use std::{path::PathBuf, time::Duration};

use chrono_crank::{
    vault_handler::VaultHandler, vault_update_state_tracker_handler::VaultUpdateStateTrackerHandler,
};
use clap::Parser;
use jito_bytemuck::AccountDeserialize;
use jito_vault_core::config::Config;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{pubkey, pubkey::Pubkey, signature::read_keypair_file};

#[derive(Parser)]
struct Args {
    /// RPC URL for the cluster
    #[arg(short, long, env, default_value = "http://localhost:8899")]
    rpc_url: String,

    /// Path to keypair used to pay
    #[arg(long, env, default_value = "~/.config/solana/id.json")]
    keypair: PathBuf,

    /// Validator history program ID (Pubkey as base58 string)
    #[arg(
        long,
        env,
        default_value = "34X2uqBhEGiWHu43RDEMwrMqXF4CpCPEZNaKdAaUS9jx"
    )]
    vault_program_id: Pubkey,

    /// Vault base
    #[arg(long)]
    vaults: Vec<Pubkey>,

    /// Operators
    #[arg(long)]
    operators: Vec<Pubkey>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let rpc_client = RpcClient::new_with_timeout(args.rpc_url.clone(), Duration::from_secs(60));
    let payer = read_keypair_file(args.keypair).expect("read keypair file");

    let config_address = Config::find_program_address(&args.vault_program_id).0;
    let account = rpc_client.get_account(&config_address).await.expect("");
    let config = Config::try_from_slice_unchecked(&account.data).expect("");

    // let vault_addresses: Vec<Pubkey> = args
    //     .vault_bases
    //     .iter()
    //     .map(|base| Vault::find_program_address(&args.vault_program_id, base).0)
    //     .collect();
    let handler = VaultHandler::new(
        &args.rpc_url,
        payer,
        args.vault_program_id,
        config_address,
        config.epoch_length(),
    );
    handler.initialize(pubkey!("H1eVpmCu7bSsXgeYNm23P3CoYwtvskKiQWArjFh9mL9B")).await;

    // let handler = VaultUpdateStateTrackerHandler::new(
    //     &args.rpc_url,
    //     payer,
    //     args.vault_program_id,
    //     config_address,
    //     config.epoch_length(),
    // );

    // let vault = handler.get_vault(args.vaults[0]).await;
    // println!("Vault {:?}", vault);

    // let mut last_epoch = 0;
    // loop {
    //     let slot = rpc_client.get_slot().await.expect("get slot");
    //     let epoch = slot / config.epoch_length();

    //     // Initialize
    //     if epoch != last_epoch {
    //         handler.initialize(&args.vaults).await;
    //     }

    //     // Crank
    //     handler.crank(&args.vaults, &args.operators).await;

    //     // Close
    //     handler.close(&args.vaults).await;

    //     last_epoch = epoch;
    // }
}
