use bip39::{Language, Mnemonic, Seed};
use rpassword::prompt_password;
use solana_clap_v3_utils::keypair::{presigner_from_pubkey_sigs, prompt_passphrase};
use solana_remote_wallet::locator::LocatorError as RemoteWalletLocatorError;
use solana_sdk::{
    derivation_path::{DerivationPath, DerivationPathError},
    pubkey::Pubkey,
    signature::{
        generate_seed_from_seed_phrase_and_passphrase, read_keypair, read_keypair_file, Keypair,
        NullSigner, Signature,
    },
    signer::{EncodableKey, EncodableKeypair, SeedDerivable, Signer},
};
use thiserror::Error;

use std::{
    error,
    io::{stdin, stdout, Write},
    process::exit,
    str::FromStr,
};

#[derive(Debug)]
pub struct SignerSource {
    pub kind: SignerSourceKind,
    pub derivation_path: Option<DerivationPath>,
    pub legacy: bool,
}

impl SignerSource {
    fn new(kind: SignerSourceKind) -> Self {
        Self {
            kind,
            derivation_path: None,
            legacy: false,
        }
    }

    fn new_legacy(kind: SignerSourceKind) -> Self {
        Self {
            kind,
            derivation_path: None,
            legacy: true,
        }
    }
}

#[derive(Debug)]
pub enum SignerSourceKind {
    Prompt,
    Filepath(String),
    // Usb(RemoteWalletLocator),
    Stdin,
    // Pubkey(Pubkey),
}

pub fn signer_from_path_with_config(
    skip_seed_phrase_validation: bool,
    path: &str,
    keypair_name: &str,
) -> Result<Box<dyn Signer>, Box<dyn error::Error>> {
    let SignerSource {
        kind,
        derivation_path,
        legacy,
    } = parse_signer_source(path)?;

    match kind {
        SignerSourceKind::Prompt => {
            let skip_validation = skip_seed_phrase_validation;
            Ok(Box::new(keypair_from_seed_phrase(
                keypair_name,
                skip_validation,
                false,
                derivation_path,
                legacy,
            )?))
        }
        SignerSourceKind::Filepath(path) => match read_keypair_file(&path) {
            Err(e) => Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("could not read keypair file \"{path}\". Run \"solana-keygen new\" to create a keypair file: {e}"),
            )
            .into()),
            Ok(file) => Ok(Box::new(file)),
        },
        SignerSourceKind::Stdin => {
            let mut stdin = std::io::stdin();
            Ok(Box::new(read_keypair(&mut stdin)?))
        }
        // SignerSourceKind::Usb(locator) => {
        //     if wallet_manager.is_none() {
        //         *wallet_manager = maybe_wallet_manager()?;
        //     }
        //     if let Some(wallet_manager) = wallet_manager {
        //         let confirm_key = matches.try_contains_id("confirm_key").unwrap_or(false);
        //         Ok(Box::new(generate_remote_keypair(
        //             locator,
        //             derivation_path.unwrap_or_default(),
        //             wallet_manager,
        //             confirm_key,
        //             keypair_name,
        //         )?))
        //     } else {
        //         Err(RemoteWalletError::NoDeviceFound.into())
        //     }
        // }
        // SignerSourceKind::Pubkey(pubkey) => {
        //     let presigner = pubkeys_sigs_of(matches, SIGNER_ARG.name)
        //         .as_ref()
        //         .and_then(|presigners| presigner_from_pubkey_sigs(&pubkey, presigners));
        //     if let Some(presigner) = presigner {
        //         Ok(Box::new(presigner))
        //     } else if config.allow_null_signer || matches.try_contains_id(SIGN_ONLY_ARG.name)? {
        //         Ok(Box::new(NullSigner::new(&pubkey)))
        //     } else {
        //         Err(std::io::Error::new(
        //             std::io::ErrorKind::Other,
        //             format!("missing signature for supplied pubkey: {pubkey}"),
        //         )
        //         .into())
        //     }
        // }
    }
}

// pub fn pubkeys_sigs_of(matches: &ArgMatches, name: &str) -> Option<Vec<(Pubkey, Signature)>> {
//     matches.values_of(name).map(|values| {
//         values
//             .map(|pubkey_signer_string| {
//                 let mut signer = pubkey_signer_string.split('=');
//                 let key = Pubkey::from_str(signer.next().unwrap()).unwrap();
//                 let sig = Signature::from_str(signer.next().unwrap()).unwrap();
//                 (key, sig)
//             })
//             .collect()
//     })
// }

#[derive(Debug, Error)]
pub(crate) enum SignerSourceError {
    #[error("unrecognized signer source")]
    UnrecognizedSource,
    #[error(transparent)]
    RemoteWalletLocatorError(#[from] RemoteWalletLocatorError),
    #[error(transparent)]
    DerivationPathError(#[from] DerivationPathError),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
}

fn parse_signer_source<S: AsRef<str>>(source: S) -> Result<SignerSource, SignerSourceError> {
    let source = source.as_ref();
    let source = {
        #[cfg(target_family = "windows")]
        {
            // trim matched single-quotes since cmd.exe won't
            let mut source = source;
            while let Some(trimmed) = source.strip_prefix('\'') {
                source = if let Some(trimmed) = trimmed.strip_suffix('\'') {
                    trimmed
                } else {
                    break;
                }
            }
            source.replace('\\', "/")
        }
        #[cfg(not(target_family = "windows"))]
        {
            source.to_string()
        }
    };
    const SIGNER_SOURCE_PROMPT: &str = "prompt";
    const SIGNER_SOURCE_FILEPATH: &str = "file";
    const SIGNER_SOURCE_USB: &str = "usb";
    const SIGNER_SOURCE_STDIN: &str = "stdin";
    const SIGNER_SOURCE_PUBKEY: &str = "pubkey";
    match uriparse::URIReference::try_from(source.as_str()) {
        Err(_) => Err(SignerSourceError::UnrecognizedSource),
        Ok(uri) => {
            if let Some(scheme) = uri.scheme() {
                let scheme = scheme.as_str().to_ascii_lowercase();
                match scheme.as_str() {
                    SIGNER_SOURCE_PROMPT => Ok(SignerSource {
                        kind: SignerSourceKind::Prompt,
                        derivation_path: DerivationPath::from_uri_any_query(&uri)?,
                        legacy: false,
                    }),
                    SIGNER_SOURCE_FILEPATH => Ok(SignerSource::new(SignerSourceKind::Filepath(
                        uri.path().to_string(),
                    ))),
                    // SIGNER_SOURCE_USB => Ok(SignerSource {
                    //     kind: SignerSourceKind::Usb(RemoteWalletLocator::new_from_uri(&uri)?),
                    //     derivation_path: DerivationPath::from_uri_key_query(&uri)?,
                    //     legacy: false,
                    // }),
                    SIGNER_SOURCE_STDIN => Ok(SignerSource::new(SignerSourceKind::Stdin)),
                    _ => {
                        #[cfg(target_family = "windows")]
                        // On Windows, an absolute path's drive letter will be parsed as the URI
                        // scheme. Assume a filepath source in case of a single character shceme.
                        if scheme.len() == 1 {
                            return Ok(SignerSource::new(SignerSourceKind::Filepath(source)));
                        }
                        Err(SignerSourceError::UnrecognizedSource)
                    }
                }
            } else {
                const STDOUT_OUTFILE_TOKEN: &str = "-";
                const ASK_KEYWORD: &str = "ASK";
                match source.as_str() {
                    STDOUT_OUTFILE_TOKEN => Ok(SignerSource::new(SignerSourceKind::Stdin)),
                    ASK_KEYWORD => Ok(SignerSource::new_legacy(SignerSourceKind::Prompt)),
                    _ => match Pubkey::from_str(source.as_str()) {
                        Ok(pubkey) => {
                            // Ok(SignerSource::new(SignerSourceKind::Pubkey(pubkey)))
                            Err(SignerSourceError::UnrecognizedSource)
                        }
                        Err(_) => std::fs::metadata(source.as_str())
                            .map(|_| SignerSource::new(SignerSourceKind::Filepath(source)))
                            .map_err(|err| err.into()),
                    },
                }
            }
        }
    }
}

pub fn keypair_from_seed_phrase(
    keypair_name: &str,
    skip_validation: bool,
    confirm_pubkey: bool,
    derivation_path: Option<DerivationPath>,
    legacy: bool,
) -> Result<Keypair, Box<dyn error::Error>> {
    let keypair: Keypair =
        encodable_key_from_seed_phrase(keypair_name, skip_validation, derivation_path, legacy)?;
    if confirm_pubkey {
        confirm_encodable_keypair_pubkey(&keypair, "pubkey");
    }
    Ok(keypair)
}

fn encodable_key_from_seed_phrase<K: EncodableKey + SeedDerivable>(
    key_name: &str,
    skip_validation: bool,
    derivation_path: Option<DerivationPath>,
    legacy: bool,
) -> Result<K, Box<dyn error::Error>> {
    let seed_phrase = prompt_password(format!("[{key_name}] seed phrase: "))?;
    let seed_phrase = seed_phrase.trim();
    let passphrase_prompt = format!(
        "[{key_name}] If this seed phrase has an associated passphrase, enter it now. Otherwise, press ENTER to continue: ",
    );

    let key = if skip_validation {
        let passphrase = prompt_passphrase(&passphrase_prompt)?;
        if legacy {
            K::from_seed_phrase_and_passphrase(seed_phrase, &passphrase)?
        } else {
            let seed = generate_seed_from_seed_phrase_and_passphrase(seed_phrase, &passphrase);
            K::from_seed_and_derivation_path(&seed, derivation_path)?
        }
    } else {
        let sanitized = sanitize_seed_phrase(seed_phrase);
        let parse_language_fn = || {
            for language in &[
                Language::English,
                Language::ChineseSimplified,
                Language::ChineseTraditional,
                Language::Japanese,
                Language::Spanish,
                Language::Korean,
                Language::French,
                Language::Italian,
            ] {
                if let Ok(mnemonic) = Mnemonic::from_phrase(&sanitized, *language) {
                    return Ok(mnemonic);
                }
            }
            Err("Can't get mnemonic from seed phrases")
        };
        let mnemonic = parse_language_fn()?;
        let passphrase = prompt_passphrase(&passphrase_prompt)?;
        let seed = Seed::new(&mnemonic, &passphrase);
        if legacy {
            K::from_seed(seed.as_bytes())?
        } else {
            K::from_seed_and_derivation_path(seed.as_bytes(), derivation_path)?
        }
    };
    Ok(key)
}

fn sanitize_seed_phrase(seed_phrase: &str) -> String {
    seed_phrase
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
}

fn confirm_encodable_keypair_pubkey<K: EncodableKeypair>(keypair: &K, pubkey_label: &str) {
    let pubkey = keypair.encodable_pubkey().to_string();
    println!("Recovered {pubkey_label} `{pubkey:?}`. Continue? (y/n): ");
    let _ignored = stdout().flush();
    let mut input = String::new();
    stdin().read_line(&mut input).expect("Unexpected input");
    if input.to_lowercase().trim() != "y" {
        println!("Exiting");
        exit(1);
    }
}
