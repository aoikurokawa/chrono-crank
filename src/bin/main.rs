use std::{path::PathBuf, time::Duration};

use chrono_crank::vault_update_state_tracker_handler::VaultUpdateStateTrackerHandler;
use clap::Parser;
use jito_bytemuck::{AccountDeserialize, Discriminator};
use jito_restaking_core::{ncn_operator_state::NcnOperatorState, ncn_vault_ticket::NcnVaultTicket};
use solana_account_decoder::UiAccountEncoding;
use solana_client::{
    nonblocking::rpc_client::RpcClient,
    rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig},
    rpc_filter::{Memcmp, MemcmpEncodedBytes, RpcFilterType},
};
use solana_sdk::{pubkey::Pubkey, signature::read_keypair_file};

#[derive(Parser)]
struct Args {
    /// RPC URL for the cluster
    #[arg(short, long, env, default_value = "http://localhost:8899")]
    rpc_url: String,

    /// Path to keypair used to pay
    #[arg(long, env, default_value = "~/.config/solana/id.json")]
    keypair: PathBuf,

    /// Vault program ID (Pubkey as base58 string)
    #[arg(
        long,
        env,
        default_value = "BLCDL7LqxaYWxSEkayc4VYjs3iCNJJw8SQzsvEL2uVT"
    )]
    vault_program_id: Pubkey,

    /// Validator history program ID (Pubkey as base58 string)
    #[arg(
        long,
        env,
        default_value = "5b2dHDz9DLhXnwQDG612bgtBGJD62Riw9s9eYuDT3Zma"
    )]
    restaking_program_id: Pubkey,

    /// NCN
    #[arg(long)]
    ncn: Pubkey,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let rpc_client = RpcClient::new_with_timeout(args.rpc_url.clone(), Duration::from_secs(60));
    let payer = read_keypair_file(args.keypair).expect("read keypair file");

    let config_address =
        jito_vault_core::config::Config::find_program_address(&args.vault_program_id).0;

    let account = rpc_client.get_account(&config_address).await.expect("");
    let config =
        jito_vault_core::config::Config::try_from_slice_unchecked(&account.data).expect("");

    let handler = VaultUpdateStateTrackerHandler::new(
        &args.rpc_url,
        payer,
        args.vault_program_id,
        config_address,
        config.epoch_length(),
    );

    let vaults: Vec<Pubkey> = list_ncn_vault_tickets(&rpc_client, &args.restaking_program_id)
        .await
        .into_iter()
        .filter_map(|ticket| {
            if ticket.ncn == args.ncn {
                Some(ticket.vault)
            } else {
                None
            }
        })
        .collect();

    let operators: Vec<Pubkey> = list_ncn_operator_states(&rpc_client, &args.restaking_program_id)
        .await
        .into_iter()
        .filter_map(|state| {
            if state.ncn == args.ncn {
                Some(state.operator)
            } else {
                None
            }
        })
        .collect();

    let mut last_epoch = 0;
    loop {
        let slot = rpc_client.get_slot().await.expect("get slot");
        let epoch = slot / config.epoch_length();

        if epoch != last_epoch {
            // Crank
            handler.crank(&vaults, &operators).await;

            // Close previous epoch's tracker
            handler.close(&vaults, last_epoch).await;

            // Initialize new tracker
            handler.initialize(&vaults, epoch).await;

            last_epoch = epoch;
        }

        // ---------- SLEEP (6 hours)----------
        tokio::time::sleep(Duration::from_secs(6 * 60 * 60)).await;
    }
}

pub async fn list_ncn_vault_tickets(
    rpc_client: &RpcClient,
    restaking_program_id: &Pubkey,
) -> Vec<NcnVaultTicket> {
    let accounts = rpc_client
        .get_program_accounts_with_config(
            restaking_program_id,
            RpcProgramAccountsConfig {
                filters: Some(vec![RpcFilterType::Memcmp(Memcmp::new(
                    0,
                    MemcmpEncodedBytes::Bytes(vec![NcnVaultTicket::DISCRIMINATOR]),
                ))]),
                account_config: RpcAccountInfoConfig {
                    encoding: Some(UiAccountEncoding::Base64),
                    data_slice: None,
                    commitment: None,
                    min_context_slot: None,
                },
                with_context: None,
            },
        )
        .await
        .expect("");

    let tickets: Vec<NcnVaultTicket> = accounts
        .iter()
        .map(|(_ncn_pubkey, ncn_vault_ticket)| {
            *NcnVaultTicket::try_from_slice_unchecked(&ncn_vault_ticket.data).expect("")
        })
        .collect();

    tickets
}

pub async fn list_ncn_operator_states(
    rpc_client: &RpcClient,
    restaking_program_id: &Pubkey,
) -> Vec<NcnOperatorState> {
    let accounts = rpc_client
        .get_program_accounts_with_config(
            restaking_program_id,
            RpcProgramAccountsConfig {
                filters: Some(vec![RpcFilterType::Memcmp(Memcmp::new(
                    0,
                    MemcmpEncodedBytes::Bytes(vec![NcnOperatorState::DISCRIMINATOR]),
                ))]),
                account_config: RpcAccountInfoConfig {
                    encoding: Some(UiAccountEncoding::Base64),
                    data_slice: None,
                    commitment: None,
                    min_context_slot: None,
                },
                with_context: None,
            },
        )
        .await
        .expect("");

    let states: Vec<NcnOperatorState> = accounts
        .iter()
        .map(|(_ncn_pubkey, ncn_vault_ticket)| {
            *NcnOperatorState::try_from_slice_unchecked(&ncn_vault_ticket.data).expect("")
        })
        .collect();

    states
}
