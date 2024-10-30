use std::{cmp::Ordering, collections::HashMap, fs::File, path::PathBuf, time::Duration};

use chrono_crank::{
    vault_program_handler::VaultProgramHandler, vault_state_manager::VaultStateManager,
};
use clap::{Parser, Subcommand};
use jito_vault_core::{vault::Vault, vault_operator_delegation::VaultOperatorDelegation};
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
        default_value = "Vau1t6sLNxnzB7ZDsef8TLbPLfyZMYXH8WTNqUdm9g8"
    )]
    vault_program_id: Pubkey,

    /// Restaking program ID (Pubkey as base58 string)
    #[arg(
        long,
        env,
        default_value = "RestkWeAVL8fRGgzhfeoqFhsqKRchg6aa1XrcH96z4Q"
    )]
    restaking_program_id: Pubkey,

    #[command(subcommand)]
    commands: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Run,
    GetVaultUpdateStateTrackers,
}

#[tokio::main]
async fn main() -> anyhow::Result<(), anyhow::Error> {
    let log_file = File::create("app.log").expect("create log file");

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .target(env_logger::Target::Pipe(Box::new(log_file)))
        .init();

    let args = Args::parse();
    let payer = read_keypair_file(args.keypair).expect("read keypair file");

    let vault_program_handler = VaultProgramHandler::new(&args.rpc_url, args.vault_program_id)
        .await
        .expect("Failed to construct VaultProgramHandler");

    match args.commands {
        Commands::Run => {
            loop {
                let vaults: HashMap<Pubkey, Vault> = vault_program_handler.get_vaults().await?;
                let vault_operator_delegations = vault_program_handler
                    .get_vault_operator_delegations()
                    .await?;
                let trackers = vault_program_handler.get_update_state_trackers().await?;

                let mut grouped_delegations: HashMap<
                    Pubkey,
                    Vec<(Pubkey, VaultOperatorDelegation)>,
                > = HashMap::new();
                for (pubkey, delegation) in vault_operator_delegations {
                    grouped_delegations
                        .entry(delegation.vault)
                        .or_default()
                        .push((pubkey, delegation));
                }

                let mut manager_map = HashMap::new();
                for (vault_pubkey, vault) in vaults.iter() {
                    let mut vault_state_manager = VaultStateManager::new(
                        &args.rpc_url,
                        args.vault_program_id,
                        &payer,
                        (*vault_pubkey, *vault),
                    );

                    // Trackers
                    if let Some(tracker) = trackers.get(vault_pubkey) {
                        vault_state_manager.set_tracker(*tracker);
                    }

                    // VaultOperatorDelegations
                    if let Some(operator_delegations) = grouped_delegations.get(vault_pubkey) {
                        vault_state_manager.set_operator_delegations(operator_delegations);
                    }

                    manager_map
                        .entry(vault_pubkey)
                        .or_insert(vault_state_manager);
                }

                match vaults.len().cmp(&trackers.len()) {
                    Ordering::Less => {}
                    Ordering::Equal => {
                        // All vaults are tracked
                        log::info!("All vaults are tracked");

                        let mut cranking_vaults = Vec::new();
                        for (vault_pubkey, vault) in vaults.iter() {
                            if let Some(tracker) = trackers.get(vault_pubkey) {
                                if tracker.1.last_updated_index() != vault.operator_count() {
                                    // crank
                                    cranking_vaults.push(vault_pubkey);
                                }
                            }
                        }

                        log::info!(
                            "The vaults which should be cranked: {}",
                            cranking_vaults.len()
                        );

                        if !cranking_vaults.is_empty() {
                            for cranking_vault_pubkey in cranking_vaults {
                                if let Some(manager) = manager_map.get(&cranking_vault_pubkey) {
                                    manager.crank().await?;
                                }
                            }
                        } else {
                            // Close
                            for manager in manager_map.values() {
                                manager.close().await?;
                            }
                        }
                    }
                    Ordering::Greater => {
                        // Initialize
                        let current_epoch = vault_program_handler.get_current_epoch().await?;
                        let config = vault_program_handler.get_config().await;
                        let mut count = 0;
                        for manager in manager_map.values() {
                            if !manager.is_tracked()
                                && manager.is_update_needed(current_epoch, config.epoch_length())
                            {
                                manager.initialize(current_epoch).await?;
                                count += 1;
                            }
                        }

                        log::info!("Initialize {count} vaults");
                    }
                }

                // ---------- SLEEP (1 hour)----------
                tokio::time::sleep(Duration::from_secs(60 * 60)).await;
            }
        }
        Commands::GetVaultUpdateStateTrackers => {
            let trackers = vault_program_handler
                .get_update_state_trackers()
                .await
                .unwrap();
            println!("{:?}", trackers);

            Ok(())
        }
    }
}
