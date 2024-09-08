use jito_vault_client::instructions::InitializeVaultBuilder;
use jito_vault_core::{config::Config, vault::Vault};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    pubkey::Pubkey,
    signature::{read_keypair_file, Keypair},
    signer::Signer,
    transaction::Transaction,
};

pub struct VaultHandler {
    rpc_url: String,
    payer: Keypair,
    vault_program_id: Pubkey,
    config_address: Pubkey,
    epoch_length: u64,
}

impl VaultHandler {
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

    pub async fn initialize(&self, token_mint: Pubkey) {
        // let keypair = self
        //     .cli_config
        //     .keypair
        //     .as_ref()
        //     .ok_or_else(|| anyhow!("Keypair not provided"))?;
        let rpc_client = self.get_rpc_client();

        let base = Keypair::new();
        let vault = Vault::find_program_address(&self.vault_program_id, &base.pubkey()).0;

        let vrt_mint = Keypair::new();

        let mut ix_builder = InitializeVaultBuilder::new();
        ix_builder
            .config(Config::find_program_address(&self.vault_program_id).0)
            .vault(vault)
            .vrt_mint(vrt_mint.pubkey())
            .token_mint(token_mint)
            .admin(self.payer.pubkey())
            .base(base.pubkey())
            .deposit_fee_bps(0)
            .withdrawal_fee_bps(0)
            .reward_fee_bps(0)
            .decimals(9);

        let blockhash = rpc_client.get_latest_blockhash().await.expect("");
        let tx = Transaction::new_signed_with_payer(
            &[ix_builder.instruction()],
            Some(&self.payer.pubkey()),
            &[&self.payer, &base, &vrt_mint],
            blockhash,
        );

        let sig = rpc_client
            .send_and_confirm_transaction(&tx)
            .await
            .expect("");

        println!("Signature {}", sig);
    }
}
