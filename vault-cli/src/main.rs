use std::{error, time::Duration};

use clap::{Parser, Subcommand};
use solana_client::rpc_client::RpcClient;
use vault_cli::{
    init_config::{command_init_config, InitConfig},
    init_vault::{command_init_vault, InitVault},
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
}

fn main() -> Result<(), Box<dyn error::Error>> {
    let args = Args::parse();
    let client = RpcClient::new_with_timeout(args.json_rpc_url.clone(), Duration::from_secs(60));

    match args.commands {
        Commands::InitConfig(args) => command_init_config(args, client),
        Commands::InitVault(args) => command_init_vault(args, client),
    }
    Ok(())
}
