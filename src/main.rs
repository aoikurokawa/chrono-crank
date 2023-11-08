use std::{
    collections::HashSet,
    error,
    sync::atomic::Ordering,
    sync::{
        atomic::{AtomicBool, AtomicU64},
        Arc,
    },
    thread,
    time::Instant,
};

use bip39::{Mnemonic, MnemonicType, Seed};
use clap::Parser;
use keygen::{
    command::{Cli, Command},
    keypair::{keypair_from_path, signer_from_path_with_config},
};
use solana_clap_v3_utils::{
    input_parsers::STDOUT_OUTFILE_TOKEN,
    keypair::{keypair_from_seed_phrase, prompt_passphrase},
};
use solana_cli_config::Config;
use solana_sdk::{
    derivation_path::DerivationPath,
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::{write_pubkey_file, Pubkey},
    signature::{
        keypair_from_seed, keypair_from_seed_and_derivation_path, write_keypair,
        write_keypair_file, Keypair,
    },
    signer::Signer,
};

fn main() -> Result<(), Box<dyn error::Error>> {
    let cli = Cli::parse();
    let default_num_threads = num_cpus::get();
    let config = if let Some(config_file) = &cli.config_file {
        Config::load(config_file.to_str().unwrap()).unwrap_or_default()
    } else {
        Config::default()
    };

    match cli.command {
        Command::New {
            outfile,
            force,
            silent,
            derivation_path,
            word_count,
            no_bip39_passphrase,
            no_outfile,
        } => {
            let mut path = dirs_next::home_dir().expect("home directory");
            let outfile = if let Some(outfile) = outfile {
                Some(outfile)
            } else if !no_outfile {
                None
            } else {
                path.extend([".config", "solana", "id.json"]);
                Some(path)
            };

            match outfile {
                Some(ref outfile) => {
                    if !force && outfile.exists() {
                        let err_msg = format!(
                            "Refusing to overwrite {} without --force flag",
                            outfile.to_str().unwrap()
                        );
                        return Err(err_msg.into());
                    }
                }
                None => {}
            };

            let word_count = word_count.parse::<usize>()?;
            let mnemonic_type = MnemonicType::for_word_count(word_count)?;

            if !silent {
                println!("Generating a new keypair");
            }

            // TODO: Only accept Engilish for now
            let mnemonic = Mnemonic::new(mnemonic_type, bip39::Language::English);

            let (passphrase, passphrase_message) = if no_bip39_passphrase {
                ("".to_string(), "".to_string())
            } else {
                match prompt_passphrase(
                    "\nFor added security, enter a BIP39 passphrase\n\
             \nNOTE! This passphrase improves security of the recovery seed phrase NOT the\n\
             keypair file itself, which is stored as insecure plain text\n\
             \nBIP39 Passphrase (empty for none): ",
                ) {
                    Ok(passphrase) => {
                        println!();
                        (passphrase, " and your BIP39 passphrase".to_string())
                    }
                    Err(_e) => ("".to_string(), "".to_string()),
                }
            };

            let seed = Seed::new(&mnemonic, &passphrase);
            let derivation_path = if let Some(path) = derivation_path {
                Some(DerivationPath::from_absolute_path_str(
                    path.to_str().unwrap(),
                )?)
            } else {
                None
            };

            let keypair = match derivation_path {
                Some(_) => keypair_from_seed_and_derivation_path(seed.as_bytes(), derivation_path)?,
                None => keypair_from_seed(seed.as_bytes())?,
            };

            if let Some(outfile) = outfile {
                output_keypair(&keypair, outfile.to_str().unwrap(), "new").map_err(|err| {
                    format!("Unable to write {}: {err}", outfile.to_str().unwrap())
                })?;
            }

            if !silent {
                let phrase = mnemonic.phrase();
                let divider = String::from_utf8(vec![b'='; phrase.len()]).unwrap();
                println!("{}\npubkey: {}\n{}\nSave this seed phrase{} to recover your new keypair:\n{}\n{}",
                                 &divider,
                                 keypair.pubkey(),
                                 &divider,
                                 passphrase_message,
                                 phrase,
                                 &divider
                             );
            }
        }
        Command::Grind {
            ignore_case,
            starts_with,
            ends_with,
            starts_and_ends_with,
            num_threads,
            use_mnemonic,
            derivation_path,
            word_count,
            no_outfile,
        } => {
            let starts_with_args: HashSet<String> = if let Some(starts_with) = starts_with {
                starts_with
                    .into_iter()
                    .map(|s| if ignore_case { s.to_lowercase() } else { s.clone() })
                    .collect()
            } else {
                HashSet::new()
            };
            let ends_with_args: HashSet<String> = if let Some(ends_with) = ends_with {
                ends_with
                    .into_iter()
                    .map(|s| if ignore_case { s.to_lowercase() } else { s.clone() })
                    .collect()
            } else {
                HashSet::new()
            };
            let starts_and_ends_with_args: HashSet<String> =
                if let Some(starts_and_ends_with) = starts_and_ends_with {
                    starts_and_ends_with
                        .into_iter()
                        .map(|s| if ignore_case { s.to_lowercase() } else { s.clone() })
                        .collect()
                } else {
                    HashSet::new()
                };

            if starts_with_args.is_empty()
                && ends_with_args.is_empty()
                && starts_with_args.is_empty()
            {
                return Err("Error: No keypair search criteria provided (--starts-with or --end-with or --starts-and-ends-with)".into());
            }

            let num_threads = match num_threads {
                Some(n) => n,
                None => default_num_threads,
            };

            let grind_matches = grind_parse_args(
                ignore_case,
                starts_with_args,
                ends_with_args,
                starts_and_ends_with_args,
                num_threads,
            );

            let word_count: usize = word_count.parse()?;
            let mnemonic_type = MnemonicType::for_word_count(word_count)?;
            let language = bip39::Language::English;

            let (passphrase, passphrase_message) = if use_mnemonic {
                // acquire_passphrase_and_message(matches)?
                match prompt_passphrase(
                    "\nFor added security, enter a BIP39 passphrase\n\
             \nNOTE! This passphrase improves security of the recovery seed phrase NOT the\n\
             keypair file itself, which is stored as insecure plain text\n\
             \nBIP39 Passphrase (empty for none): ",
                ) {
                    Ok(passphrase) => {
                        println!();
                        (passphrase, " and your BIP39 passphrase".to_string())
                    }
                    Err(_e) => ("".to_string(), "".to_string()),
                }
            } else {
                ("".to_string(), "".to_string())
            };

            let skip_len_44_pubkeys = grind_matches
                .iter()
                .map(|g| {
                    let target_key = if ignore_case {
                        g.starts.to_ascii_uppercase()
                    } else {
                        g.starts.clone()
                    };
                    let target_key =
                        target_key + &(0..44 - g.starts.len()).map(|_| "1").collect::<String>();
                    bs58::decode(target_key).into_vec()
                })
                .filter_map(|s| s.ok())
                .all(|s| s.len() > 32);

            let grind_matches_thread_safe = Arc::new(grind_matches);
            let attempts = Arc::new(AtomicU64::new(1));
            let found = Arc::new(AtomicU64::new(0));
            let start = Instant::now();
            let done = Arc::new(AtomicBool::new(false));

            let thread_handles: Vec<_> = (0..num_threads).map(|_| {
                 let done = done.clone();
                 let attempts = attempts.clone();
                 let found = found.clone();
                 let grind_matches_thread_safe = grind_matches_thread_safe.clone();
                 let passphrase = passphrase.clone();
                 let passphrase_message = passphrase_message.clone();
                 let derivation_path = derivation_path.clone();

                 thread::spawn(move || loop {
                     if done.load(Ordering::Relaxed) {
                         break;
                     }

                     let attempts = attempts.fetch_add(1, Ordering::Relaxed);
                     if attempts % 1_000_000 == 0 {
                         println!(
                             "Searched {} keypairs in {}s. {} matches found.",
                             attempts,
                             start.elapsed().as_secs(),
                             found.load(Ordering::Relaxed)
                         );
                     }
                     let (keypair, phrase) = if use_mnemonic {
                         let mnemonic = Mnemonic::new(mnemonic_type, language);
                         let seed = Seed::new(&mnemonic, &passphrase);
                         let keypair = match derivation_path {
                             Some(_) => keypair_from_seed_and_derivation_path(
                                 seed.as_bytes(),
                                 derivation_path.clone().map(|p| DerivationPath::from_absolute_path_str(p.to_str().unwrap()).unwrap()).clone(),
                             ),
                             None => keypair_from_seed(seed.as_bytes()),
                         }.unwrap();
                         (keypair, mnemonic.phrase().to_string())
                     } else {
                         (Keypair::new(), "".to_string())
                     };

                     if skip_len_44_pubkeys
                         && keypair.pubkey() >= smallest_length_44_public_key::PUBKEY
                     {
                         continue;
                     }
                     let mut pubkey = bs58::encode(keypair.pubkey()).into_string();
                     if ignore_case {
                         pubkey = pubkey.to_lowercase();
                     }
                     let mut total_matches_found = 0;
                     for i in 0..grind_matches_thread_safe.len() {
                         if grind_matches_thread_safe[i].count.load(Ordering::Relaxed) == 0 {
                             total_matches_found += 1;
                             continue;
                         }
                         if (!grind_matches_thread_safe[i].starts.is_empty()
                             && grind_matches_thread_safe[i].ends.is_empty()
                             && pubkey.starts_with(&grind_matches_thread_safe[i].starts))
                             || (grind_matches_thread_safe[i].starts.is_empty()
                                 && !grind_matches_thread_safe[i].ends.is_empty()
                                 && pubkey.ends_with(&grind_matches_thread_safe[i].ends))
                             || (grind_matches_thread_safe[i].starts.is_empty()
                                 && !grind_matches_thread_safe[i].ends.is_empty()
                                 && pubkey.starts_with(&grind_matches_thread_safe[i].starts)
                                 && pubkey.ends_with(&grind_matches_thread_safe[i].ends))
                         {
                             let _found = found.fetch_add(1, Ordering::Relaxed);
                             grind_matches_thread_safe[i]
                                 .count
                                 .fetch_sub(1, Ordering::Relaxed);
                             if !no_outfile {
                                 write_keypair_file(
                                     &keypair,
                                     &format!("{}.json", keypair.pubkey()),
                                 ).unwrap();
                                 println!(
                                     "Wrote keypair to {}",
                                     &format!("{}.json", keypair.pubkey())
                                 );
                             }
                             if use_mnemonic {
                                 let divider = String::from_utf8(vec![b'='; phrase.len()]).unwrap();
                                 println!("{}\nFound matching key {}", &divider, keypair.pubkey());
                                 println!("\nSave this seed phrase{} to recover your new keypair:\n{}\n{}", passphrase_message, phrase, &divider);
                             }
                         }
                     }
                     if total_matches_found == grind_matches_thread_safe.len() {
                         done.store(true, Ordering::Relaxed);
                     }
                 })
             })
             .collect();

            for thread_handles in thread_handles {
                thread_handles.join().unwrap();
            }
        }
        Command::Pubkey {
            outfile,
            keypair,
            force,
            skip_seed_phrase_validation,
        } => {
            let pubkey =
                get_keypair_from_matches(skip_seed_phrase_validation, keypair.clone(), config)?
                    .try_pubkey()?;

            if let Some(outfile) = outfile {
                if !force && outfile.exists() {
                    let err_msg = format!(
                        "Refusing to overwrite {} without --force flag",
                        outfile.to_str().unwrap()
                    );
                    return Err(err_msg.into());
                }
                write_pubkey_file(outfile.to_str().unwrap(), pubkey)?;
            } else {
                println!("{pubkey}");
            }
        }
        Command::Verify {
            keypair,
            pubkey,
            skip_seed_phrase_validation,
        } => {
            let keypair =
                get_keypair_from_matches(skip_seed_phrase_validation, keypair.clone(), config)?;
            let simple_message = Message::new(
                &[Instruction::new_with_bincode(
                    Pubkey::default(),
                    &0,
                    vec![AccountMeta::new(keypair.pubkey(), true)],
                )],
                Some(&keypair.pubkey()),
            )
            .serialize();
            let signature = keypair.try_sign_message(&simple_message)?;
            let pubkey_bs58 = &*pubkey;
            let pubkey = bs58::decode(pubkey_bs58).into_vec().unwrap();
            if signature.verify(&pubkey, &simple_message) {
                println!("Verification for public key: {pubkey_bs58}: Success");
            } else {
                let err_msg = format!("Verification for public key: {pubkey_bs58}: Failed");
                return Err(err_msg.into());
            }
        }
        Command::Recover {
            outfile,
            force,
            prompt_signer,
            skip_seed_phrase_validation,
        } => {
            let mut path = dirs_next::home_dir().expect("home directory");
            let outfile = if let Some(outfile) = outfile {
                outfile
            } else {
                path.extend([".config", "solana", "id.json"]);
                path
            };

            if outfile.to_str().unwrap() != STDOUT_OUTFILE_TOKEN {
                if !force && outfile.exists() {
                    let err_msg = format!(
                        "Refusing to overwrite {} without --force flag",
                        outfile.to_str().unwrap()
                    );
                    return Err(err_msg.into());
                }
            }

            let keypair_name = "recover";
            let keypair = if let Some(path) = prompt_signer {
                keypair_from_path(skip_seed_phrase_validation, &path, keypair_name, true)?
            } else {
                let skip_validation = skip_seed_phrase_validation;
                keypair_from_seed_phrase(keypair_name, skip_validation, true, None, true)?
            };
            output_keypair(&keypair, outfile.to_str().unwrap(), "recovered")?
        }
    }

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

fn get_keypair_from_matches(
    skip_seed_phrase_validation: bool,
    keypair: Option<String>,
    config: Config,
) -> Result<Box<dyn Signer>, Box<dyn error::Error>> {
    let mut path = dirs_next::home_dir().expect("home directory");
    let path = if let Some(keypair) = keypair {
        // matches.value_of("keypair").unwrap()
        keypair
    } else if !config.keypair_path.is_empty() {
        config.keypair_path
    } else {
        path.extend([".config", "solana", "id.json"]);
        path.to_str().unwrap().to_string()
    };

    signer_from_path(skip_seed_phrase_validation, &path, "pubkey_recovery")
}

fn signer_from_path(
    skip_seed_phrase_validation: bool,
    path: &str,
    keypair_name: &str,
) -> Result<Box<dyn Signer>, Box<dyn error::Error>> {
    signer_from_path_with_config(skip_seed_phrase_validation, path, keypair_name)
}

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

