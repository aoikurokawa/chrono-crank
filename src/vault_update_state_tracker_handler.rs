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

pub struct VaultUpdateStateTrackerHandler {
    rpc_url: String,
    payer: Keypair,
    vault_program_id: Pubkey,
    config_address: Pubkey,
    epoch_length: u64,
}

impl VaultUpdateStateTrackerHandler {
    pub fn new(
        rpc_url: &str,
        payer: Keypair,
        vault_program_id: Pubkey,
        config_address: Pubkey,
        epoch_length: u64,
    ) -> Self {
        Self {
            rpc_url: rpc_url.to_string(),
            payer,
            vault_program_id,
            config_address,
            epoch_length,
        }
    }

    fn get_rpc_client(&self) -> RpcClient {
        RpcClient::new_with_commitment(self.rpc_url.clone(), CommitmentConfig::confirmed())
    }

    pub async fn get_vault(&self, vault_address: Pubkey) -> Vault {
        let rpc_client = self.get_rpc_client();
        let account = rpc_client
            .get_account(&vault_address)
            .await
            .expect("get accounts");
        let vault = Vault::try_from_slice_unchecked(&account.data).expect("");
        // let vaults: Vec<Vault> = accounts
        //     .iter()
        //     .filter(|account| account.is_some())
        //     .map(|account| Vault::try_from_slice_unchecked(&account.unwrap().data).expect(""))
        //     .collect();

        *vault
    }

    pub async fn initialize(&self, vaults: &[Pubkey]) {
        let rpc_client = self.get_rpc_client();
        let slot = rpc_client.get_slot().await.expect("get slot");

        for vault in vaults {
            let tracker = VaultUpdateStateTracker::find_program_address(
                &self.vault_program_id,
                vault,
                slot / self.epoch_length,
            )
            .0;

            let mut ix_builder = InitializeVaultUpdateStateTrackerBuilder::new();
            ix_builder
                .config(self.config_address)
                .vault(*vault)
                .vault_update_state_tracker(tracker)
                .payer(self.payer.pubkey())
                .system_program(system_program::id())
                .withdrawal_allocation_method(WithdrawalAllocationMethod::Greedy);

            let blockhash = rpc_client
                .get_latest_blockhash()
                .await
                .expect("get latest blockhash");
            let tx = Transaction::new_signed_with_payer(
                &[ix_builder.instruction()],
                Some(&self.payer.pubkey()),
                &[&self.payer],
                blockhash,
            );

            rpc_client
                .send_and_confirm_transaction(&tx)
                .await
                .expect("send transaction");
        }
    }

    pub async fn crank(&self, vaults: &[Pubkey], operators: &[Pubkey]) {
        let rpc_client = self.get_rpc_client();
        let slot = rpc_client.get_slot().await.expect("get slot");

        for vault in vaults {
            for operator in operators {
                let vault_operator_delegation = &VaultOperatorDelegation::find_program_address(
                    &self.vault_program_id,
                    &vault,
                    &operator,
                )
                .0;

                let mut ix_builder = CrankVaultUpdateStateTrackerBuilder::new();
                let tracker = VaultUpdateStateTracker::find_program_address(
                    &self.vault_program_id,
                    &vault,
                    slot / self.epoch_length,
                )
                .0;
                ix_builder
                    .config(self.config_address)
                    .vault(*vault)
                    .operator(*operator)
                    .vault_operator_delegation(*vault_operator_delegation)
                    .vault_update_state_tracker(tracker);

                let blockhash = rpc_client
                    .get_latest_blockhash()
                    .await
                    .expect("get latest blockhash");
                let tx = Transaction::new_signed_with_payer(
                    &[ix_builder.instruction()],
                    Some(&self.payer.pubkey()),
                    &[&self.payer],
                    blockhash,
                );

                rpc_client
                    .send_and_confirm_transaction(&tx)
                    .await
                    .expect("send transaction");
            }
        }
    }

    pub async fn close(&self, vaults: &[Pubkey]) {
        let rpc_client = self.get_rpc_client();
        let slot = rpc_client.get_slot().await.expect("get slot");

        for vault in vaults {
            let mut ix_builder = CloseVaultUpdateStateTrackerBuilder::new();
            let tracker = VaultUpdateStateTracker::find_program_address(
                &self.vault_program_id,
                &vault,
                slot / self.epoch_length,
            )
            .0;
            ix_builder
                .config(self.config_address)
                .vault(*vault)
                .vault_update_state_tracker(tracker)
                .payer(self.payer.pubkey())
                .ncn_epoch(slot / self.epoch_length);

            let blockhash = rpc_client
                .get_latest_blockhash()
                .await
                .expect("get latest blockhash");
            let tx = Transaction::new_signed_with_payer(
                &[ix_builder.instruction()],
                Some(&self.payer.pubkey()),
                &[&self.payer],
                blockhash,
            );

            rpc_client
                .send_and_confirm_transaction(&tx)
                .await
                .expect("send transaction");
        }
    }
}
