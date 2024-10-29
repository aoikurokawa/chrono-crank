use std::{cmp::Ordering, collections::HashMap, fs::File, ops::Rem, path::PathBuf, time::Duration};

use anyhow::Context;
use chrono_crank::{
    vault_handler::VaultHandler, vault_update_state_tracker_handler::VaultUpdateStateTrackerHandler,
};
use clap::{Parser, Subcommand};
use jito_bytemuck::AccountDeserialize;
use jito_vault_core::{vault::Vault, vault_operator_delegation::VaultOperatorDelegation};
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

    let vault_handler =
        VaultHandler::new(&args.rpc_url, &payer, args.vault_program_id, config_address);
    let handler = VaultUpdateStateTrackerHandler::new(
        &args.rpc_url,
        &payer,
        args.restaking_program_id,
        args.vault_program_id,
        config_address,
        config.epoch_length(),
    );

    match args.commands {
        Commands::Run => {
            loop {
                let slot = rpc_client.get_slot().await.context("failed to get slot")?;
                let epoch = slot / config.epoch_length();
                let vaults: HashMap<Pubkey, Vault> = vault_handler.get_vaults().await?;
                let vault_operator_delegations =
                    vault_handler.get_vault_operator_delegations().await?;
                let trackers = handler.get_update_state_trackers().await?;

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

                match vaults.len().cmp(&trackers.len()) {
                    Ordering::Less => {}
                    Ordering::Equal => {
                        // All vaults are tracked
                        log::info!("All vaults are tracked");
                        // let mut grouped_trackers = HashMap::new();
                        // for tracker in trackers {
                        //     grouped_trackers.entry(tracker.vault).or_insert(tracker);
                        // }

                        // let mut vault_tracker_set = Vec::with_capacity(vaults.len());
                        // for vault in vaults.iter() {
                        //     let vault_pubkey =
                        //         Vault::find_program_address(&args.vault_program_id, &vault.base).0;
                        //     for tracker in trackers.iter() {
                        //         if vault_pubkey == tracker.vault {
                        //             vault_tracker_set.push((vault_pubkey, vault, tracker));
                        //         } else {
                        //             continue;
                        //         }
                        //     }
                        // }

                        let mut cranking_vaults = Vec::new();
                        for (vault_pubkey, vault) in vaults.iter() {
                            if let Some(tracker) = trackers.get(vault_pubkey) {
                                if tracker.last_updated_index() != vault.operator_count() {
                                    // crank
                                    cranking_vaults.push((vault_pubkey, vault));
                                }
                            }
                        }

                        if !cranking_vaults.is_empty() {
                            for (cranking_vault_pubkey, cranking_vault) in cranking_vaults {
                                if let Some(delegations) =
                                    grouped_delegations.get(cranking_vault_pubkey)
                                {
                                    let mut sorted_operators =
                                        Vec::with_capacity(delegations.len());
                                    if let Some(tracker) = trackers.get(cranking_vault_pubkey) {
                                        let start_index =
                                            tracker.ncn_epoch().rem(&cranking_vault.ncn_count());
                                        loop {
                                            let mut next_index = start_index;
                                            for (_, delegation) in delegations {
                                                if delegation.index() == next_index {
                                                    sorted_operators.push(delegation.operator);
                                                    next_index += 1;
                                                }
                                            }

                                            if sorted_operators.len() == delegations.len() {
                                                break;
                                            }
                                        }
                                    }

                                    handler
                                        .crank(cranking_vault_pubkey, &sorted_operators)
                                        .await?;
                                }
                            }
                        } else {
                            // Close
                            let vault_pubkeys: Vec<Pubkey> = vaults.keys().copied().collect();

                            handler.close(&vault_pubkeys, epoch).await?;
                        }
                    }
                    Ordering::Greater => {
                        // Initialize
                        let mut uninitialized = Vec::new();
                        for (vault_pubkey, vault) in vaults {
                            if !trackers.contains_key(&vault_pubkey)
                                && vault.last_full_state_update_slot() / config.epoch_length()
                                    != epoch
                            {
                                uninitialized.push(vault_pubkey);
                            }

                            // for (i, tracker) in trackers.iter().enumerate() {
                            //     if tracker.vault.eq(&vault_pubkey) {
                            //         break;
                            //     } else {
                            //         if i == trackers.len() {
                            //             uninitialized.push(vault_pubkey);
                            //         }
                            //         continue;
                            //     }
                            // }
                        }

                        log::info!("Initialize {} vaults", uninitialized.len());
                        handler.initialize(&uninitialized, epoch).await?;
                    }
                }

                // let mut last_epoch = epoch;
                // let mut close_failed = false;
                // let mut count = 0;
                //     let slot = rpc_client.get_slot().await.context("get slot")?;
                //     let epoch = slot / config.epoch_length();

                //     log::info!("Slot: {slot}, Current Epoch: {epoch}, Last Epoch: {last_epoch}");

                //     if epoch != last_epoch || (close_failed && count < 10) {
                //         let ncn_vault_tickets: Vec<Pubkey> =
                //             match handler.get_ncn_vault_tickets(args.ncn).await {
                //                 Ok(v) => v,
                //                 Err(_) => vaults.clone(),
                //             };
                //         let vaults = vault_handler.get_vaults(&ncn_vault_tickets).await?;
                //         let vaults: Vec<Pubkey> = vaults
                //             .iter()
                //             .filter_map(|(pubkey, vault)| {
                //                 // Initialize new tracker
                //                 if vault.last_full_state_update_slot() / config.epoch_length() != epoch
                //                 {
                //                     Some(*pubkey)
                //                 } else {
                //                     None
                //                 }
                //             })
                //             .collect();

                //         // Close previous epoch's tracker
                //         match handler.close(&vaults, last_epoch).await {
                //             Ok(()) => {
                //                 // Initialize new tracker
                //                 handler.initialize(&vaults, epoch).await?;

                //                 last_epoch = epoch;
                //                 close_failed = false;
                //                 count = 0;
                //             }
                //             Err(e) => {
                //                 close_failed = true;
                //                 count += 1;

                //                 if count == 9 {
                //                     log::error!("Error: Failed to close tracker");
                //                     return Err(e);
                //                 }
                //             }
                //         }
                //     }

                // ---------- SLEEP (1 hour)----------
                tokio::time::sleep(Duration::from_secs(60 * 60)).await;
            }
        }
        Commands::GetVaultUpdateStateTrackers => {
            let trackers = handler.get_update_state_trackers().await.unwrap();
            println!("{:?}", trackers);

            Ok(())
        }
    }
}
