use std::ops::Rem;

use jito_bytemuck::AccountDeserialize;
use jito_vault_client::{
    instructions::{
        CloseVaultUpdateStateTrackerBuilder, CrankVaultUpdateStateTrackerBuilder,
        InitializeVaultUpdateStateTrackerBuilder,
    },
    types::WithdrawalAllocationMethod,
};
use jito_vault_core::{
    vault::Vault, vault_operator_delegation::VaultOperatorDelegation,
    vault_update_state_tracker::VaultUpdateStateTracker,
};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig, pubkey::Pubkey, signature::Keypair, signer::Signer,
    system_program, transaction::Transaction,
};

pub struct VaultStateManager<'a> {
    /// RPC URL
    rpc_url: String,

    /// Jito Vault Program ID
    vault_program_id: Pubkey,

    /// The payer to send tx
    payer: &'a Keypair,

    config_pubkey: Pubkey,
    vault: (Pubkey, Vault),
    tracker: Option<(Pubkey, VaultUpdateStateTracker)>,
    operator_delegations: Option<Vec<(Pubkey, VaultOperatorDelegation)>>,
}

impl<'a> VaultStateManager<'a> {
    pub fn new(
        rpc_url: &str,
        vault_program_id: Pubkey,
        payer: &'a Keypair,
        vault: (Pubkey, Vault),
    ) -> Self {
        let config_pubkey =
            jito_vault_core::config::Config::find_program_address(&vault_program_id).0;

        Self {
            rpc_url: rpc_url.to_string(),
            vault_program_id,
            payer,
            config_pubkey,
            vault,
            tracker: None,
            operator_delegations: None,
        }
    }

    pub fn is_tracked(&self) -> bool {
        self.tracker.is_some()
    }

    pub fn is_update_needed(&self, current_epoch: u64, epoch_length: u64) -> bool {
        let last_update_epoch = self.vault.1.last_full_state_update_slot() / epoch_length;

        last_update_epoch < current_epoch
    }

    pub fn set_tracker(&mut self, tracker: (Pubkey, VaultUpdateStateTracker)) {
        self.tracker = Some(tracker);
    }

    pub fn set_operator_delegations(&mut self, delegations: &[(Pubkey, VaultOperatorDelegation)]) {
        self.operator_delegations = Some(delegations.to_vec());
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
                    Err(anyhow::Error::new(e).context("Failed deserialzing"))
                }
            },
            Err(e) => {
                log::error!("Error: Failed to get VaultUpdateStateTracker account: {tracker}");
                Err(anyhow::Error::new(e).context("Failed to get VaultUpdateStateTracker"))
            }
        }
    }

    pub async fn initialize(&self, epoch: u64) -> anyhow::Result<()> {
        let rpc_client = self.get_rpc_client();

        let tracker_pubkey = VaultUpdateStateTracker::find_program_address(
            &self.vault_program_id,
            &self.vault.0,
            epoch,
        )
        .0;

        if self.get_update_state_tracker(&tracker_pubkey).await.is_ok() {
            log::info!("VaultUpdateStateTracker already exists: {tracker_pubkey}");
            return Ok(());
        }

        log::info!("Initialize Vault Update State Tracker: {tracker_pubkey}");

        let mut ix_builder = InitializeVaultUpdateStateTrackerBuilder::new();
        ix_builder
            .config(self.config_pubkey)
            .vault(self.vault.0)
            .vault_update_state_tracker(tracker_pubkey)
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
                log::info!("Transaction confirmed: {sig}");
            }
            Err(e) => {
                log::error!("Failed to send transaction: {:?}", e);
                return Err(anyhow::Error::new(e).context("Failed to send transaction"));
            }
        }

        Ok(())
    }

    fn sort_by_delegation_index(&self) -> Option<Vec<(Pubkey, VaultOperatorDelegation)>> {
        if let Some(tracker) = self.tracker {
            if let Some(operator_delegations) = &self.operator_delegations {
                let start_index = tracker.1.ncn_epoch().rem(&self.vault.1.operator_count());

                let mut delegations = operator_delegations.clone();

                // Sort delegations by index in ascending order
                delegations.sort_by_key(|(_pubkey, delegation)| delegation.index());

                // Find the starting position based on `start_index`
                let start_position = delegations
                    .iter()
                    .position(|(_pubkey, delegation)| delegation.index() == start_index);

                // If a valid starting position is found, push operators from the sorted list
                let mut sorted_delegations = Vec::with_capacity(delegations.len());
                if let Some(start_position) = start_position {
                    sorted_delegations.extend(
                        delegations
                            .iter()
                            .cycle() // Allows the iteration to wrap around the list
                            .skip(start_position)
                            .take(delegations.len()),
                    );

                    return Some(sorted_delegations);
                }
            }
        }

        None
    }

    pub async fn crank(&self) -> anyhow::Result<()> {
        let rpc_client = self.get_rpc_client();

        // the vault does not have operator
        if self.vault.1.operator_count() == 0 {
            log::info!("The vault does not have operators currently");
            return Ok(());
        }

        let delegations = self.sort_by_delegation_index();

        if let Some(tracker) = self.tracker {
            if let Some(delegations) = delegations {
                for delegation in delegations {
                    log::info!(
                        "Crank Vault Operator Delegation: {}, Vault Update State Tracker: {}",
                        delegation.0,
                        tracker.0
                    );

                    let mut ix_builder = CrankVaultUpdateStateTrackerBuilder::new();
                    ix_builder
                        .config(self.config_pubkey)
                        .vault(self.vault.0)
                        .operator(delegation.1.operator)
                        .vault_operator_delegation(delegation.0)
                        .vault_update_state_tracker(tracker.0);
                    let mut ix = ix_builder.instruction();
                    ix.program_id = self.vault_program_id;

                    let blockhash = match rpc_client.get_latest_blockhash().await {
                        Ok(bh) => bh,
                        Err(e) => {
                            log::error!("Failed to get latest blockhash: {e}");
                            return Err(
                                anyhow::Error::new(e).context("Failed to get latest blockhash")
                            );
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
        }

        Ok(())
    }

    pub async fn close(&self) -> anyhow::Result<()> {
        let rpc_client = self.get_rpc_client();

        if let Some(tracker) = self.tracker {
            log::info!("Close Vault Update State Tracker: {:?}", tracker.0);

            let mut ix_builder = CloseVaultUpdateStateTrackerBuilder::new();
            ix_builder
                .config(self.config_pubkey)
                .vault(self.vault.0)
                .vault_update_state_tracker(tracker.0)
                .payer(self.payer.pubkey())
                .ncn_epoch(tracker.1.ncn_epoch());
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

#[cfg(test)]
mod tests {
    use super::*;

    // Operator count: 3
    // NCN epoch: 0
    // Start index: 0
    #[test]
    fn test_sort_by_delegation_index_epoch_0() {
        let mut vault = Vault::new(
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            0,
            Pubkey::new_unique(),
            0,
            0,
            0,
            0,
            0,
            0,
        )
        .unwrap();
        vault.increment_operator_count().unwrap();
        vault.increment_operator_count().unwrap();
        vault.increment_operator_count().unwrap();

        let vault = (Pubkey::new_unique(), vault);

        let delegation0 = (
            Pubkey::new_unique(),
            VaultOperatorDelegation::new(Pubkey::default(), Pubkey::default(), 0, 0, 0),
        );
        let delegation1 = (
            Pubkey::new_unique(),
            VaultOperatorDelegation::new(Pubkey::default(), Pubkey::default(), 1, 0, 0),
        );
        let delegation2 = (
            Pubkey::new_unique(),
            VaultOperatorDelegation::new(Pubkey::default(), Pubkey::default(), 2, 0, 0),
        );
        let operator_delegations = vec![delegation0, delegation1, delegation2];

        let payer = Keypair::new();
        let mut manager = VaultStateManager::new("", Pubkey::new_unique(), &payer, vault);
        manager.tracker = Some((
            Pubkey::new_unique(),
            VaultUpdateStateTracker::new(Pubkey::new_unique(), 0, 0),
        ));
        manager.operator_delegations = Some(operator_delegations);

        let delegations = manager.sort_by_delegation_index();

        assert!(delegations.is_some());
        assert_eq!(
            delegations.unwrap(),
            vec![delegation0, delegation1, delegation2]
        );
    }

    // Operator count: 3
    // NCN epoch: 1
    // Start index: 0
    #[test]
    fn test_sort_by_delegation_index_epoch_1() {
        let mut vault = Vault::new(
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            0,
            Pubkey::new_unique(),
            0,
            0,
            0,
            0,
            0,
            0,
        )
        .unwrap();
        vault.increment_operator_count().unwrap();
        vault.increment_operator_count().unwrap();
        vault.increment_operator_count().unwrap();

        let vault = (Pubkey::new_unique(), vault);

        let delegation0 = (
            Pubkey::new_unique(),
            VaultOperatorDelegation::new(Pubkey::default(), Pubkey::default(), 0, 0, 0),
        );
        let delegation1 = (
            Pubkey::new_unique(),
            VaultOperatorDelegation::new(Pubkey::default(), Pubkey::default(), 1, 0, 0),
        );
        let delegation2 = (
            Pubkey::new_unique(),
            VaultOperatorDelegation::new(Pubkey::default(), Pubkey::default(), 2, 0, 0),
        );
        let operator_delegations = vec![delegation0, delegation1, delegation2];

        let payer = Keypair::new();
        let mut manager = VaultStateManager::new("", Pubkey::new_unique(), &payer, vault);
        manager.tracker = Some((
            Pubkey::new_unique(),
            VaultUpdateStateTracker::new(Pubkey::new_unique(), 1, 0),
        ));
        manager.operator_delegations = Some(operator_delegations);

        let delegations = manager.sort_by_delegation_index();

        assert!(delegations.is_some());
        assert_eq!(
            delegations.unwrap(),
            vec![delegation1, delegation2, delegation0]
        );
    }

    // Operator count: 3
    // NCN epoch: 2
    // Start index: 0
    #[test]
    fn test_sort_by_delegation_index_epoch_2() {
        let mut vault = Vault::new(
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            0,
            Pubkey::new_unique(),
            0,
            0,
            0,
            0,
            0,
            0,
        )
        .unwrap();
        vault.increment_operator_count().unwrap();
        vault.increment_operator_count().unwrap();
        vault.increment_operator_count().unwrap();

        let vault = (Pubkey::new_unique(), vault);

        let delegation0 = (
            Pubkey::new_unique(),
            VaultOperatorDelegation::new(Pubkey::default(), Pubkey::default(), 0, 0, 0),
        );
        let delegation1 = (
            Pubkey::new_unique(),
            VaultOperatorDelegation::new(Pubkey::default(), Pubkey::default(), 1, 0, 0),
        );
        let delegation2 = (
            Pubkey::new_unique(),
            VaultOperatorDelegation::new(Pubkey::default(), Pubkey::default(), 2, 0, 0),
        );
        let operator_delegations = vec![delegation0, delegation1, delegation2];

        let payer = Keypair::new();
        let mut manager = VaultStateManager::new("", Pubkey::new_unique(), &payer, vault);
        manager.tracker = Some((
            Pubkey::new_unique(),
            VaultUpdateStateTracker::new(Pubkey::new_unique(), 2, 0),
        ));
        manager.operator_delegations = Some(operator_delegations);

        let delegations = manager.sort_by_delegation_index();

        assert!(delegations.is_some());
        assert_eq!(
            delegations.unwrap(),
            vec![delegation2, delegation0, delegation1]
        );
    }
}
