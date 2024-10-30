use std::collections::HashMap;

use anyhow::Context;
use jito_bytemuck::{AccountDeserialize, Discriminator};
use jito_vault_core::{
    vault::Vault, vault_operator_delegation::VaultOperatorDelegation,
    vault_update_state_tracker::VaultUpdateStateTracker,
};
use solana_account_decoder::UiAccountEncoding;
use solana_client::{
    nonblocking::rpc_client::RpcClient,
    rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig},
    rpc_filter::{Memcmp, MemcmpEncodedBytes, RpcFilterType},
};
use solana_sdk::{commitment_config::CommitmentConfig, pubkey::Pubkey};

pub struct VaultProgramHandler {
    rpc_url: String,
    vault_program_id: Pubkey,
}

impl VaultProgramHandler {
    pub async fn new(rpc_url: &str, vault_program_id: Pubkey) -> anyhow::Result<Self> {
        Ok(Self {
            rpc_url: rpc_url.to_string(),
            vault_program_id,
        })
    }

    fn get_rpc_client(&self) -> RpcClient {
        RpcClient::new_with_commitment(self.rpc_url.clone(), CommitmentConfig::confirmed())
    }

    pub async fn get_config(&self) -> jito_vault_core::config::Config {
        let rpc_client = self.get_rpc_client();

        let config_pubkey =
            jito_vault_core::config::Config::find_program_address(&self.vault_program_id).0;
        let account = rpc_client
            .get_account(&config_pubkey)
            .await
            .expect("Failed to read Jito vault config address");
        let config = jito_vault_core::config::Config::try_from_slice_unchecked(&account.data)
            .expect("Failed to deserialize Jito vault config");

        *config
    }

    pub async fn get_current_epoch(&self) -> anyhow::Result<u64> {
        let rpc_client = self.get_rpc_client();

        let slot = rpc_client.get_slot().await.context("failed to get slot")?;

        let config = self.get_config().await;
        let epoch = slot / config.epoch_length();

        Ok(epoch)
    }

    pub async fn get_vaults(&self) -> anyhow::Result<HashMap<Pubkey, Vault>> {
        let rpc_client = self.get_rpc_client();
        let accounts = rpc_client
            .get_program_accounts_with_config(
                &self.vault_program_id,
                RpcProgramAccountsConfig {
                    filters: Some(vec![RpcFilterType::Memcmp(Memcmp::new(
                        0,
                        MemcmpEncodedBytes::Bytes(vec![Vault::DISCRIMINATOR]),
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
                log::error!("Error failed to get VaultUpdateStateTracker");
                "Failed to get VaultUpdateStateTracker accounts".to_string()
            })?;

        let vaults: Vec<(Pubkey, Vault)> = accounts
            .iter()
            .filter_map(|(pubkey, acc)| {
                let vault = Vault::try_from_slice_unchecked(&acc.data)
                    .context("Error: Failed to deserailize")
                    .ok()?;
                Some((*pubkey, *vault))
            })
            .collect();

        Ok(HashMap::from_iter(vaults))
    }

    /// Retrieves all existing `VaultOperatorDelegation` accounts associated with the program.
    ///
    /// # Returns
    ///
    /// An `anyhow::Result` containing a vector of `(Pubkey, VaultOperatorDelegation)` tuples. Each
    /// tuple represents a vault operator delegation account and includes:
    /// - `Pubkey`: The public key of the vault operator delegation account.
    /// - `VaultOperatorDelegation`: The deserialized vault operator delegation data.
    pub async fn get_vault_operator_delegations(
        &self,
    ) -> anyhow::Result<Vec<(Pubkey, VaultOperatorDelegation)>> {
        let rpc_client = self.get_rpc_client();
        let accounts = rpc_client
            .get_program_accounts_with_config(
                &self.vault_program_id,
                RpcProgramAccountsConfig {
                    filters: Some(vec![RpcFilterType::Memcmp(Memcmp::new(
                        0,
                        MemcmpEncodedBytes::Bytes(vec![VaultOperatorDelegation::DISCRIMINATOR]),
                    ))]),
                    account_config: RpcAccountInfoConfig {
                        encoding: Some(UiAccountEncoding::Base64),
                        ..RpcAccountInfoConfig::default()
                    },
                    ..RpcProgramAccountsConfig::default()
                },
            )
            .await?;

        let delegations: Vec<(Pubkey, VaultOperatorDelegation)> = accounts
            .into_iter()
            .filter_map(|(pubkey, acc)| {
                VaultOperatorDelegation::try_from_slice_unchecked(&acc.data)
                    .map_or(None, |v| Some((pubkey, *v)))
            })
            .collect();

        Ok(delegations)
    }

    pub async fn get_update_state_trackers(
        &self,
    ) -> anyhow::Result<HashMap<Pubkey, (Pubkey, VaultUpdateStateTracker)>> {
        let rpc_client = self.get_rpc_client();
        let accounts = rpc_client
            .get_program_accounts_with_config(
                &self.vault_program_id,
                RpcProgramAccountsConfig {
                    filters: Some(vec![RpcFilterType::Memcmp(Memcmp::new(
                        0,
                        MemcmpEncodedBytes::Bytes(vec![VaultUpdateStateTracker::DISCRIMINATOR]),
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
                log::error!("Error failed to get VaultUpdateStateTracker");
                "Failed to get VaultUpdateStateTracker accounts".to_string()
            })?;

        let trackers: Vec<(Pubkey, VaultUpdateStateTracker)> = accounts
            .iter()
            .filter_map(|(pubkey, tracker_acc)| {
                match VaultUpdateStateTracker::try_from_slice_unchecked(&tracker_acc.data) {
                    Ok(tracker) => Some((*pubkey, *tracker)),
                    Err(e) => {
                        log::error!("Error deserializing VaultUpdateStateTracker: {:?}", e);
                        None
                    }
                }
            })
            .collect();

        let mut map = HashMap::new();
        for tracker in trackers {
            map.entry(tracker.1.vault).or_insert((tracker.0, tracker.1));
        }

        Ok(map)
    }
}
