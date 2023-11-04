use std::{collections::HashSet, error, rc::Rc, sync::atomic::AtomicU64, sync::atomic::Ordering};

use clap::{crate_description, crate_name, Arg, ArgMatches, Command};
use solana_clap_utils::{
    input_parsers::STDOUT_OUTFILE_TOKEN, keypair::signer_from_path, DisplayError,
};
use solana_cli_config::{Config, CONFIG_FILE};
use solana_remote_wallet::remote_wallet::RemoteWalletManager;
use solana_sdk::{
    signature::{write_keypair, write_keypair_file, Keypair},
    signer::Signer,
};

fn main() -> Result<(), Box<dyn error::Error>> {
    let default_num_threads = num_cpus::get().to_string();
    let matches = app(&default_num_threads, solana_version::version!())
        .try_get_matches()
        .unwrap_or_else(|e| e.exit());
    do_main(&matches).map_err(|err| DisplayError::new_as_boxed(err).into())
}

fn do_main(matches: &ArgMatches) -> Result<(), Box<dyn error::Error>> {
    Ok(())
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

// fn get_keypair_from_matches(
//     matches: &ArgMatches,
//     config: Config,
//     wallet_manager: &mut Option<Rc<RemoteWalletManager>>,
// ) -> Result<Box<dyn Signer>, Box<dyn error::Error>> {
//     let mut path = dirs_next::home_dir().expect("home directory");
//     let path = if let Some(keypair) = matches.get_one::<String>("keypair") {
//         // matches.value_of("keypair").unwrap()
//         keypair
//     } else if !config.keypair_path.is_empty() {
//         &config.keypair_path
//     } else {
//         path.extend([".config", "solana", "id.json"]);
//         path.to_str().unwrap()
//     };
//
//     signer_from_path(matches, path, "pubkey_recovery", wallet_manager)
// }

fn output_keypair(
    keypair: &Keypair,
    outfile: &str,
    source: &str,
) -> Result<(), Box<dyn error::Error>> {
    if outfile == STDOUT_OUTFILE_TOKEN {
        let mut stdout = std::io::stdout();
        write_keypair(keypair, &mut stdout)?;
    } else {
        write_keypair_file(keypair, outfile)?;
        println!("Wrote {source} keypair to {outfile}");
    }

    Ok(())
}

fn grind_validator_starts_with(v: &str) -> Result<(), String> {
    if v.matches(':').count() != 1 || (v.starts_with(':') || v.ends_with(':')) {
        return Err(String::from("Expected : between PREFIX and COUNT"));
    }

    let args: Vec<&str> = v.split(':').collect();
    bs58::decode(&args[0])
        .into_vec()
        .map_err(|err| format!("{}: {:?}", args[0], err))?;

    let count = args[1].parse::<u64>();
    if count.is_err() || count.unwrap() == 0 {
        return Err(String::from("Expected COUNT to be of type u64"));
    }

    Ok(())
}

fn grind_validator_ends_with(v: &str) -> Result<(), String> {
    if v.matches(':').count() != 1 || (v.starts_with(':') || v.ends_with(':')) {
        return Err(String::from("Expected : between SUFFIX and COUNT"));
    }

    let args: Vec<&str> = v.split(':').collect();
    bs58::decode(&args[0])
        .into_vec()
        .map_err(|err| format!("{}: {:?}", args[0], err))?;

    let count = args[1].parse::<u64>();
    if count.is_err() || count.unwrap() == 0 {
        return Err(String::from("Expected COUNT to be of type u64"));
    }

    Ok(())
}

fn grind_validator_starts_and_end_with(v: &str) -> Result<(), String> {
    if v.matches(':').count() != 2 || (v.starts_with(':') || v.ends_with(':')) {
        return Err(String::from(
            "Expected : between PREFIX and SUFFIX and COUNT",
        ));
    }

    let args: Vec<&str> = v.split(':').collect();
    bs58::decode(&args[0])
        .into_vec()
        .map_err(|err| format!("{}: {:?}", args[0], err))?;
    bs58::decode(&args[1])
        .into_vec()
        .map_err(|err| format!("{}: {:?}", args[1], err))?;

    let count = args[2].parse::<u64>();
    if count.is_err() || count.unwrap() == 0 {
        return Err(String::from("Expected COUNT to be of type u64"));
    }
    Ok(())
}

fn grind_print_info(grind_matches: &[GrindMatch], num_threads: usize) {
    println!("Searching with {num_threads} threads for:");

    for gm in grind_matches {
        let mut msg = Vec::new();

        if gm.count.load(Ordering::Relaxed) > 1 {
            msg.push("pubkeys".to_string());
            msg.push("start".to_string());
            msg.push("end".to_string());
        } else {
            msg.push("pubkey".to_string());
            msg.push("starts".to_string());
            msg.push("ends".to_string());
        }

        println!(
            "\t{} {} that {} with '{}' and {} with '{}'",
            gm.count.load(Ordering::Relaxed),
            msg[0],
            msg[1],
            gm.starts,
            msg[2],
            gm.ends
        );
    }
}

fn grind_parse_args(
    ignore_case: bool,
    starts_with_args: HashSet<String>,
    ends_with_args: HashSet<String>,
    starts_and_ends_with_args: HashSet<String>,
    num_threads: usize,
) -> Vec<GrindMatch> {
    let mut grind_matches = Vec::new();
    for sw in starts_with_args {
        let args: Vec<&str> = sw.split(':').collect();
        grind_matches.push(GrindMatch {
            starts: if ignore_case {
                args[0].to_lowercase()
            } else {
                args[0].to_string()
            },
            ends: "".to_string(),
            count: AtomicU64::new(args[1].parse::<u64>().unwrap()),
        });
    }

    for ew in ends_with_args {
        let args: Vec<&str> = ew.split(':').collect();
        grind_matches.push(GrindMatch {
            starts: "".to_string(),
            ends: if ignore_case {
                args[0].to_lowercase()
            } else {
                args[0].to_string()
            },
            count: AtomicU64::new(args[1].parse::<u64>().unwrap()),
        });
    }

    for swew in starts_and_ends_with_args {
        let args: Vec<&str> = swew.split(':').collect();
        grind_matches.push(GrindMatch {
            starts: if ignore_case {
                args[0].to_lowercase()
            } else {
                args[0].to_string()
            },
            ends: if ignore_case {
                args[1].to_lowercase()
            } else {
                args[1].to_string()
            },
            count: AtomicU64::new(args[2].parse::<u64>().unwrap()),
        });
    }

    grind_print_info(&grind_matches, num_threads);

    grind_matches
}

fn app<'a>(num_threads: &'a str, crate_version: &'a str) -> Command<'a> {
    Command::new(crate_name!())
        .about(crate_description!())
        // .version(crate_version)
        .subcommand_required(true)
        .arg_required_else_help(true)
        .arg({
            let arg = Arg::new("config_file")
                .short('C')
                .long("config")
                .value_name("FILEPATH")
                // .takes_value(true)
                .global(true)
                .help("Configuration file to use");

            if let Some(config_file) = solana_cli_config::CONFIG_FILE.as_ref() {
                arg.default_value(&config_file)
            }  else {
                arg
            }

        })
        .subcommand(
            Command::new("verify")
                .about("Verify a keypair can sign and verify a message.")
                .arg(
                    Arg::new("pubkey")
                        .index(1)
                        .value_name("PUBKEY")
                        // .takes_value(true)
                        .required(true)
                        .help("Public key"),
                )
                .arg(
                    Arg::new("keypair")
                        .index(2)
                        .value_name("KEYPAIR")
                        // .takes_value(true)
                        .help("Filepath or URL to a keypair"),
                ),
        )
        .subcommand(Command::new("new")
        .about("Generate new keypair file from a random seed phrase and optional BIP39 passphrase")
        .disable_version_flag(true)
        .arg(
            Arg::new("outfile").short('o').long("outfile").value_name("FILEPATH")
                // .takes_value(true)
                .help("Path to generated file"),
        )
            .arg(
            Arg::new("force").short('f').long("force").help("Overwrite the output file if it exists"),
        )
            .arg(
            Arg::new("silent").short('s').long("silent").help("Do not display seed phrase. Useful when piping output to other programs that prompt for user input, like gpg")
        )
        )
}
