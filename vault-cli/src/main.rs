use std::{error, time::Duration};

use clap::{Parser, Subcommand};
use sokoban::ZeroCopy;
use solana_client::rpc_client::RpcClient;
use vault_cli::{
    init_config::{command_init_config, InitConfig},
    init_vault::{command_init_vault, InitVault},
    jito_vault_program_id,
};

#[derive(Parser)]
struct Args {
    /// RPC URL for the cluster
    #[arg(short, long, env, default_value = "http://localhost:8899")]
    json_rpc_url: String,

    #[command(subcommand)]
    commands: Commands,
}

#[derive(Subcommand)]
enum Commands {
    InitConfig(InitConfig),
    InitVault(InitVault),
    GetConfig,
}

fn main() -> Result<(), Box<dyn error::Error>> {
    let args = Args::parse();
    let client = RpcClient::new_with_timeout(args.json_rpc_url.clone(), Duration::from_secs(60));

    match args.commands {
        Commands::InitConfig(args) => command_init_config(args, client),
        Commands::InitVault(args) => command_init_vault(args, client),
        Commands::GetConfig => {
            let jito_vault_program_id = jito_vault_program_id();
            let config_pubkey =
                jito_vault_core::config::Config::find_program_address(&jito_vault_program_id).0;
            let res = client
                .get_account_data(&config_pubkey)
                .expect("Fail to fetch config account");
            let config =
                jito_vault_core::config::Config::load_bytes(&res).expect("Fail to convert Config");

            println!("config bump: {}", config.bump());
        }
    }
    Ok(())
}
