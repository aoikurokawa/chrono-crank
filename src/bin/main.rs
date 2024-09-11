use std::{fs::File, path::PathBuf, time::Duration};

use anyhow::Context;
use chrono_crank::vault_update_state_tracker_handler::VaultUpdateStateTrackerHandler;
use clap::Parser;
use jito_bytemuck::AccountDeserialize;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{pubkey::Pubkey, signature::read_keypair_file};

#[derive(Parser)]
struct Args {
    /// RPC URL for the cluster
    #[arg(short, long, env, default_value = "https://api.devnet.solana.com")]
    rpc_url: String,

    /// Path to keypair used to pay
    #[arg(long, env, default_value = "~/.config/solana/id.json")]
    keypair: PathBuf,

    /// Vault program ID (Pubkey as base58 string)
    #[arg(
        long,
        env,
        default_value = "34X2uqBhEGiWHu43RDEMwrMqXF4CpCPEZNaKdAaUS9jx"
    )]
    vault_program_id: Pubkey,

    /// Restaking program ID (Pubkey as base58 string)
    #[arg(
        long,
        env,
        default_value = "78J8YzXGGNynLRpn85MH77PVLBZsWyLCHZAXRvKaB6Ng"
    )]
    restaking_program_id: Pubkey,

    /// NCN
    #[arg(long)]
    ncn: Pubkey,
}

#[tokio::main]
async fn main() -> anyhow::Result<(), anyhow::Error> {
    let log_file = File::create("app.log").expect("create log file");

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .target(env_logger::Target::Pipe(Box::new(log_file)))
        .init();

    let args = Args::parse();
    let rpc_client = RpcClient::new_with_timeout(args.rpc_url.clone(), Duration::from_secs(60));
    let payer = read_keypair_file(args.keypair).expect("read keypair file");

    let config_address =
        jito_vault_core::config::Config::find_program_address(&args.vault_program_id).0;

    let account = rpc_client
        .get_account(&config_address)
        .await
        .expect("Failed to read Jito vault config address");
    let config = jito_vault_core::config::Config::try_from_slice_unchecked(&account.data)
        .expect("Failed to deserialize Jito vault config");

    let handler = VaultUpdateStateTrackerHandler::new(
        &args.rpc_url,
        payer,
        args.restaking_program_id,
        args.vault_program_id,
        config_address,
        config.epoch_length(),
    );

    let vaults: Vec<Pubkey> = handler.get_vaults(args.ncn).await?;

    // Initialize new tracker
    let slot = rpc_client.get_slot().await.context("get slot")?;
    let epoch = slot / config.epoch_length();
    handler.initialize(&vaults, epoch).await?;

    let mut last_epoch = epoch;
    loop {
        let slot = rpc_client.get_slot().await.context("get slot")?;
        let epoch = slot / config.epoch_length();

        log::info!("Slot: {slot}, Current Epoch: {epoch}, Last Epoch: {last_epoch}");

        if epoch != last_epoch {
            let vaults: Vec<Pubkey> = handler.get_vaults(args.ncn).await?;

            let operators: Vec<Pubkey> = handler.get_operators(args.ncn).await?;

            // Crank
            handler.crank(&vaults, &operators).await?;

            // Close previous epoch's tracker
            handler.close(&vaults, last_epoch).await?;

            // Initialize new tracker
            handler.initialize(&vaults, epoch).await?;

            last_epoch = epoch;
        }

        // ---------- SLEEP (1 hour)----------
        tokio::time::sleep(Duration::from_secs(60 * 60)).await;
    }
}
