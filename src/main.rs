use std::{error, rc::Rc, sync::atomic::AtomicU64};

use clap::ArgMatches;
use solana_clap_utils::keypair::signer_from_path;
use solana_cli_config::Config;
use solana_remote_wallet::remote_wallet::RemoteWalletManager;
use solana_sdk::signer::Signer;

fn main() {
    println!("Hello, world!");
}

mod smallest_length_44_public_key {
    use solana_sdk::{pubkey, pubkey::Pubkey};

    pub(super) static PUBKEY: Pubkey = pubkey!("21111111111111111111111111111111111111111111");

    #[test]
    fn assert_length() {
        use crate::smallest_length_44_public_key;

        assert_eq!(smallest_length_44_public_key::PUBKEY.to_string().len(), 44);
    }
}

struct GrindMatch {
    starts: String,
    ends: String,
    count: AtomicU64,
}

fn get_keypair_from_matches(
    matches: &ArgMatches,
    config: Config,
    wallet_manager: &mut Option<Rc<RemoteWalletManager>>,
) -> Result<Box<dyn Signer>, Box<dyn error::Error>> {
    let mut path = dirs_next::home_dir().expect("home directory");
    let path = if matches.is_present("keypair") {
        matches.value_of("keypair").unwrap()
    } else if !config.keypair_path.is_empty() {
        &config.keypair_path
    } else {
        path.extend([".config", "solana", "id.json"]);
        path.to_str().unwrap()
    };

    signer_from_path(matches, path, "pubkey_recovery", wallet_manager)
}
