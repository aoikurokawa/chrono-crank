use anyhow::Context;
use jito_bytemuck::{AccountDeserialize, Discriminator};
use jito_restaking_core::{ncn_operator_state::NcnOperatorState, ncn_vault_ticket::NcnVaultTicket};
use jito_vault_client::{
    instructions::{
        CloseVaultUpdateStateTrackerBuilder, CrankVaultUpdateStateTrackerBuilder,
        InitializeVaultUpdateStateTrackerBuilder,
    },
    types::WithdrawalAllocationMethod,
};
use jito_vault_core::{
    vault_operator_delegation::VaultOperatorDelegation,
    vault_update_state_tracker::VaultUpdateStateTracker,
};
use solana_account_decoder::UiAccountEncoding;
use solana_client::{
    nonblocking::rpc_client::RpcClient,
    rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig},
    rpc_filter::{Memcmp, MemcmpEncodedBytes, RpcFilterType},
};
use solana_sdk::{
    commitment_config::CommitmentConfig, pubkey::Pubkey, signature::Keypair, signer::Signer,
    system_program, transaction::Transaction,
};

pub struct VaultUpdateStateTrackerHandler {
    rpc_url: String,
    payer: Keypair,
    restaking_program_id: Pubkey,
    vault_program_id: Pubkey,
    config_address: Pubkey,
    epoch_length: u64,
}

impl VaultUpdateStateTrackerHandler {
    pub fn new(
        rpc_url: &str,
        payer: Keypair,
        restaking_program_id: Pubkey,
        vault_program_id: Pubkey,
        config_address: Pubkey,
        epoch_length: u64,
    ) -> Self {
        Self {
            rpc_url: rpc_url.to_string(),
            payer,
            restaking_program_id,
            vault_program_id,
            config_address,
            epoch_length,
        }
    }

    fn get_rpc_client(&self) -> RpcClient {
        RpcClient::new_with_commitment(self.rpc_url.clone(), CommitmentConfig::confirmed())
    }

    async fn get_update_state_tracker(
        &self,
        tracker: &Pubkey,
    ) -> anyhow::Result<VaultUpdateStateTracker> {
        let rpc_client = self.get_rpc_client();
        match rpc_client.get_account(tracker).await {
            Ok(account) => match VaultUpdateStateTracker::try_from_slice_unchecked(&account.data) {
                Ok(tracker) => Ok(*tracker),
                Err(e) => {
                    log::error!("Error: Failed deserializing VaultUpdateStateTracker: {tracker}");
                    return Err(anyhow::Error::new(e).context("Failed deserialzing"));
                }
            },
            Err(e) => {
                log::error!("Error: Failed to get VaultUpdateStateTracker account: {tracker}");
                return Err(anyhow::Error::new(e).context("Failed to get VaultUpdateStateTracker"));
            }
        }
    }

    pub async fn get_vaults(&self, ncn_address: Pubkey) -> anyhow::Result<Vec<Pubkey>> {
        let rpc_client = self.get_rpc_client();
        let accounts = rpc_client
            .get_program_accounts_with_config(
                &self.restaking_program_id,
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
            .with_context(|| {
                log::error!("Error failed to get NcnVaultTicket");
                format!("Failed to get NcnVaultTicket accounts: {}", ncn_address)
            })?;

        let tickets: Vec<NcnVaultTicket> = accounts
            .iter()
            .filter_map(|(_ncn_pubkey, ncn_vault_ticket)| {
                match NcnVaultTicket::try_from_slice_unchecked(&ncn_vault_ticket.data) {
                    Ok(ticket) => Some(*ticket),
                    Err(e) => {
                        log::error!("Error deserializing NcnVaultTicket: {:?}", e);
                        None
                    }
                }
            })
            .collect();

        let vaults: Vec<Pubkey> = tickets
            .into_iter()
            .filter_map(|ticket| {
                if ticket.ncn == ncn_address {
                    Some(ticket.vault)
                } else {
                    None
                }
            })
            .collect();

        Ok(vaults)
    }

    pub async fn get_operators(&self, ncn_address: Pubkey) -> anyhow::Result<Vec<Pubkey>> {
        let rpc_client = self.get_rpc_client();
        let accounts = rpc_client
            .get_program_accounts_with_config(
                &self.restaking_program_id,
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
            .with_context(|| {
                log::error!("Error failed to get NcnOperatorState");
                format!("Failed to get NcnOperatorState accounts: {}", ncn_address)
            })?;

        let states: Vec<NcnOperatorState> = accounts
            .iter()
            .filter_map(|(_ncn_pubkey, ncn_vault_ticket)| {
                match NcnOperatorState::try_from_slice_unchecked(&ncn_vault_ticket.data) {
                    Ok(state) => Some(*state),
                    Err(e) => {
                        log::error!("Error deserializing NcnOperatorState: {:?}", e);
                        None
                    }
                }
            })
            .collect();

        let operators: Vec<Pubkey> = states
            .into_iter()
            .filter_map(|state| {
                if state.ncn == ncn_address {
                    Some(state.operator)
                } else {
                    None
                }
            })
            .collect();

        Ok(operators)
    }

    pub async fn initialize(&self, vaults: &[Pubkey], epoch: u64) -> anyhow::Result<()> {
        let rpc_client = self.get_rpc_client();

        for vault in vaults {
            let tracker =
                VaultUpdateStateTracker::find_program_address(&self.vault_program_id, vault, epoch)
                    .0;

            log::info!("Initialize Vault Update State Tracker: {:?}", tracker);

            let mut ix_builder = InitializeVaultUpdateStateTrackerBuilder::new();
            ix_builder
                .config(self.config_address)
                .vault(*vault)
                .vault_update_state_tracker(tracker)
                .payer(self.payer.pubkey())
                .system_program(system_program::id())
                .withdrawal_allocation_method(WithdrawalAllocationMethod::Greedy);
            let mut ix = ix_builder.instruction();
            ix.program_id = self.vault_program_id;

            let blockhash = match rpc_client.get_latest_blockhash().await {
                Ok(bh) => bh,
                Err(e) => {
                    log::error!("Failed to get latest blockhash: {e}");
                    return Err(anyhow::Error::new(e).context("Failed to get latest blockhash"));
                }
            };
            let tx = Transaction::new_signed_with_payer(
                &[ix],
                Some(&self.payer.pubkey()),
                &[&self.payer],
                blockhash,
            );

            match rpc_client.send_and_confirm_transaction(&tx).await {
                Ok(sig) => {
                    log::info!("Transaction confirmed: {:?}", sig);
                }
                Err(e) => {
                    log::error!("Failed to send transaction: {:?}", e);
                    return Err(anyhow::Error::new(e).context("Failed to send transaction"));
                }
            }
        }

        Ok(())
    }

    pub async fn crank(&self, vaults: &[Pubkey], operators: &[Pubkey]) -> anyhow::Result<()> {
        let rpc_client = self.get_rpc_client();
        let slot = rpc_client.get_slot().await.expect("get slot");

        for vault in vaults {
            for operator in operators {
                let vault_operator_delegation = &VaultOperatorDelegation::find_program_address(
                    &self.vault_program_id,
                    vault,
                    operator,
                )
                .0;
                let tracker = VaultUpdateStateTracker::find_program_address(
                    &self.vault_program_id,
                    vault,
                    slot / self.epoch_length,
                )
                .0;

                log::info!(
                    "Crank Vault Operator Delegation: {}, Vault Update State Tracker: {}",
                    vault_operator_delegation,
                    tracker
                );

                let mut ix_builder = CrankVaultUpdateStateTrackerBuilder::new();
                ix_builder
                    .config(self.config_address)
                    .vault(*vault)
                    .operator(*operator)
                    .vault_operator_delegation(*vault_operator_delegation)
                    .vault_update_state_tracker(tracker);
                let mut ix = ix_builder.instruction();
                ix.program_id = self.vault_program_id;

                let blockhash = match rpc_client.get_latest_blockhash().await {
                    Ok(bh) => bh,
                    Err(e) => {
                        log::error!("Failed to get latest blockhash: {e}");
                        return Err(anyhow::Error::new(e).context("Failed to get latest blockhash"));
                    }
                };
                let tx = Transaction::new_signed_with_payer(
                    &[ix],
                    Some(&self.payer.pubkey()),
                    &[&self.payer],
                    blockhash,
                );

                match rpc_client.send_and_confirm_transaction(&tx).await {
                    Ok(sig) => {
                        log::info!("Transaction confirmed: {:?}", sig);
                    }
                    Err(e) => {
                        log::error!("Failed to send transaction: {:?}", e);
                        return Err(anyhow::Error::new(e).context("Failed to send transaction"));
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn close(&self, vaults: &[Pubkey], epoch: u64) -> anyhow::Result<()> {
        let rpc_client = self.get_rpc_client();
        let slot = rpc_client.get_slot().await.expect("get slot");

        for vault in vaults {
            let mut ix_builder = CloseVaultUpdateStateTrackerBuilder::new();
            let tracker =
                VaultUpdateStateTracker::find_program_address(&self.vault_program_id, vault, epoch)
                    .0;

            log::info!("Close Vault Update State Tracker: {:?}", tracker);

            ix_builder
                .config(self.config_address)
                .vault(*vault)
                .vault_update_state_tracker(tracker)
                .payer(self.payer.pubkey())
                .ncn_epoch(slot / self.epoch_length);
            let mut ix = ix_builder.instruction();
            ix.program_id = self.vault_program_id;

            let blockhash = match rpc_client.get_latest_blockhash().await {
                Ok(bh) => bh,
                Err(e) => {
                    log::error!("Failed to get latest blockhash: {e}");
                    return Err(anyhow::Error::new(e).context("Failed to get latest blockhash"));
                }
            };
            let tx = Transaction::new_signed_with_payer(
                &[ix],
                Some(&self.payer.pubkey()),
                &[&self.payer],
                blockhash,
            );

            match rpc_client.send_and_confirm_transaction(&tx).await {
                Ok(sig) => {
                    log::info!("Transaction confirmed: {:?}", sig);
                }
                Err(e) => {
                    log::error!("Failed to send transaction: {:?}", e);
                    return Err(anyhow::Error::new(e).context("Failed to send transaction"));
                }
            }
        }

        Ok(())
    }
}
