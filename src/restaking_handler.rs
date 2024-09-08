use jito_restaking_client::instructions::{InitializeNcnBuilder, InitializeOperatorBuilder};
use jito_restaking_core::{ncn::Ncn, operator::Operator};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig, pubkey, pubkey::Pubkey, signature::Keypair,
    signer::Signer, transaction::Transaction,
};

pub struct RestakingHandler<'a> {
    rpc_url: String,
    payer: &'a Keypair,
    restaking_program_id: Pubkey,
    config_address: Pubkey,
}

impl<'a> RestakingHandler<'a> {
    pub fn new(
        rpc_url: &str,
        payer: &'a Keypair,
        restaking_program_id: Pubkey,
        config_address: Pubkey,
    ) -> Self {
        Self {
            rpc_url: rpc_url.to_string(),
            payer,
            restaking_program_id,
            config_address,
        }
    }

    fn get_rpc_client(&self) -> RpcClient {
        RpcClient::new_with_commitment(self.rpc_url.clone(), CommitmentConfig::confirmed())
    }

    pub async fn initialize_config(&self) {
        let rpc_client = self.get_rpc_client();

        let mut ix_builder = jito_restaking_client::instructions::InitializeConfigBuilder::new();
        let config_address =
            jito_restaking_core::config::Config::find_program_address(&self.restaking_program_id).0;
        ix_builder
            .config(config_address)
            .admin(self.payer.pubkey())
            .vault_program(pubkey!("BLCDL7LqxaYWxSEkayc4VYjs3iCNJJw8SQzsvEL2uVT"));
        let mut ix = ix_builder.instruction();
        ix.program_id = self.restaking_program_id;

        let blockhash = rpc_client.get_latest_blockhash().await.expect("");
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.payer.pubkey()),
            &[&self.payer],
            blockhash,
        );
        rpc_client
            .send_and_confirm_transaction(&tx)
            .await
            .expect("");
    }

    pub async fn initialize_ncn(&self) {
        let rpc_client = self.get_rpc_client();

        let base = Keypair::new();
        let ncn = Ncn::find_program_address(&self.restaking_program_id, &base.pubkey()).0;

        let mut ix_builder = InitializeNcnBuilder::new();
        ix_builder
            .config(
                jito_restaking_core::config::Config::find_program_address(
                    &self.restaking_program_id,
                )
                .0,
            )
            .ncn(ncn)
            .admin(self.payer.pubkey())
            .base(base.pubkey())
            .instruction();
        let mut ix = ix_builder.instruction();
        ix.program_id = self.restaking_program_id;

        let blockhash = rpc_client.get_latest_blockhash().await.expect("");
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.payer.pubkey()),
            &[&self.payer, &base],
            blockhash,
        );
        rpc_client
            .send_and_confirm_transaction(&tx)
            .await
            .expect("");
    }

    pub async fn initialize_operator(&self) {
        let rpc_client = self.get_rpc_client();

        let base = Keypair::new();
        let operator = Operator::find_program_address(&self.restaking_program_id, &base.pubkey()).0;

        let mut ix_builder = InitializeOperatorBuilder::new();
        ix_builder
            .config(
                jito_restaking_core::config::Config::find_program_address(
                    &self.restaking_program_id,
                )
                .0,
            )
            .operator(operator)
            .admin(self.payer.pubkey())
            .base(base.pubkey())
            .instruction();

        let blockhash = rpc_client.get_latest_blockhash().await.expect("");
        let tx = Transaction::new_signed_with_payer(
            &[ix_builder.instruction()],
            Some(&self.payer.pubkey()),
            &[self.payer, &base],
            blockhash,
        );
        rpc_client
            .send_and_confirm_transaction(&tx)
            .await
            .expect("");
    }
}
