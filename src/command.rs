use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    pub config_file: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Verify a keypair can sign and verify a message.
    Verify {
        #[arg(short, long, value_name = "PUBKEY", help = "Public key")]
        pubkey: String,

        #[arg(
            short,
            long,
            value_name = "KEYPAIR",
            help = "Filepath or URL to a keypair"
        )]
        keypair: Option<String>,

        #[arg(
            long = "skip-seed-phrase-validation",
            help = "Skip validation of seed phrases. Use this if your phrase does not use the BIP39 official English word list"
        )]
        skip_seed_phrase_validation: bool,
    },

    /// Generate new keypair file from a random seed phrase and optional BIP39 passphrase
    New {
        #[arg(short, long, value_name = "FILEPATH", help = "Path to generated file")]
        outfile: Option<PathBuf>,

        #[arg(short, long, help = "Path to generated file")]
        force: bool,

        #[arg(
            short,
            long,
            help = "Do not display seed phrase. Useful when piping output to other programs that prompt for user input, like gpg"
        )]
        silent: bool,

        #[arg(
            long,
            value_name = "DERIVATION_PATH",
            help = "Derivation path. All indexes will be promoted to hardened. \
            If arg is not presented then derivation path will not be used. \
            If arg is presented with empty DERIVATION_PATH value then m/44'/501'/0'/0' will be used."
        )]
        derivation_path: Option<PathBuf>,

        #[arg(
            long,
            value_parser = ["12", "15", "18", "21", "24"],
            default_value = "12",
            value_name = "NUMBER",
            help = "Specify the number of words that will be present in the generated seed phrase"
        )]
        word_count: String,

        #[arg(
            long = "no-bip39-passphrase",
            alias = "no-passphrase",
            help = "Do not prompt for a BIP39 passphrase"
        )]
        no_bip39_passphrase: bool,

        #[arg(
            long = "no-outfile",
            help = "Onlyt print a seed phrase and pubkey. Do not output a keypair file"
        )]
        no_outfile: bool,
    },

    /// Grind for a verify keypairs
    Grind {
        #[arg(
            long,
            value_name = "PREFIX:COUNT",
            num_args = 1,
            value_parser = grind_validator_starts_with,
            help = "Saves specified number of keypairs whos public key starts with the indicated prefix\nExample: --starts-with sol:4\nPREFIX type is Base58\nCOUNT type is u64"
        )]
        starts_with: Option<String>,

        #[arg(
            long,
            value_name = "SUFFIX:COUNT",
            num_args = 1,
            value_parser = grind_validator_ends_with,
            help = "Saves specified number of keypairs whos public key ends with the indicated suffix\nExample: --ends-with ana:4\nSUFFIX type is Base58\nCOUNT type is u64"
        )]
        ends_with: Option<Vec<String>>,

        #[arg(
            long,
            value_name = "PREFIX:SUFFIX:COUNT",
            num_args = 1,
            value_parser = grind_validator_starts_and_ends_with,
            help = "Saves specified number of keypairs whos public key starts and ends with the indicated perfix and suffix\nExample: --starts-and-ends-with sol:ana:4\nPREFIX and SUFFIX type is Base58\nCOUNT type is u64"
        )]
        starts_and_ends_with: Option<Vec<String>>,

        #[arg(
            long = "num-threads", 
            value_name = "NUMBER", 
            value_parser = clap::value_parser!(usize),
            help = "Specify the number of grind threads"
        )]
        num_threads: Option<usize>,

        #[arg(long, help = "Perform case insensitive matches")]
        ignore_case: bool,

        #[arg(
            long = "use-mnemonic",
            help = "Generate using a mnemonic key phrase. Expect a significant slowdown in this mode"
        )]
        use_mnemonic: bool,

        #[arg(
            long,
            value_name = "DERIVATION_PATH",
            help = "Derivation path. All indexes will be promoted to hardened. \
            If arg is not presented then derivation path will not be used. \
            If arg is presented with empty DERIVATION_PATH value then m/44'/501'/0'/0' will be used."
        )]
        derivation_path: Option<PathBuf>,

        #[arg(
            long,
            value_parser = ["12", "15", "18", "21", "24"],
            default_value = "12",
            value_name = "NUMBER",
            help = "Specify the number of words that will be present in the generated seed phrase"
        )]
        word_count: String,

        #[arg(
            long = "no-outfile",
            help = "Onlyt print a seed phrase and pubkey. Do not output a keypair file"
        )]
        no_outfile: bool,
    },

    /// Display the pubkey from a keypair file
    Pubkey {
        #[arg(
            short,
            long,
            value_name = "KEYPAIR",
            help = "Filepath or URL to a keypair"
        )]
        keypair: Option<String>,

        #[arg(
            long = "skip-seed-phrase-validation",
            help = "Skip validation of seed phrases. Use this if your phrase does not use the BIP39 official English word list"
        )]
        skip_seed_phrase_validation: bool,

        #[arg(short, long, value_name = "FILEPATH", help = "Path to generated file")]
        outfile: Option<PathBuf>,

        #[arg(short, long, help = "Overwrite the output file if it exists")]
        force: bool,
    },

    /// Recover keypair from seed phrase and optional BIP39 passphrase
    Recover {
        #[arg(value_name = "KEYPAIR", help = "prompt: URI schme or `ASK` keyword")]
        prompt_signer: Option<String>,

        #[arg(short, long, value_name = "FILEPATH", help = "Path to generated file")]
        outfile: Option<PathBuf>,

        #[arg(short, long, help = "Overwrite the output file if it exists")]
        force: bool,

        #[arg(
            long = "skip-seed-phrase-validation",
            help = "Skip validation of seed phrases. Use this if your phrase does not use the BIP39 official English word list"
        )]
        skip_seed_phrase_validation: bool,
    },
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

fn grind_validator_starts_and_ends_with(v: &str) -> Result<(), String> {
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
