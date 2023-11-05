use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
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
    },

    /// Generate new keypair file from a random seed phrase and optional BIP39 passphrase
    New {
        #[arg(short, long, value_name = "FILEPATH", help = "Path to generated file")]
        outfile: Option<PathBuf>,

        #[arg(short, long, help = "Path to generated file")]
        force: Option<String>,

        #[arg(
            short,
            long,
            help = "Do not display seed phrase. Useful when piping output to other programs that prompt for user input, like gpg"
        )]
        silent: Option<String>,
    },

    /// Grind for a verify keypairs
    Grind {
        #[arg(long, help = "Perform case insensitive matches")]
        ignore_case: Option<bool>,
    },

    /// Display the pubkey from a keypair file
    Pubkey {
        #[arg(
            long,
            value_name = "PREFIX:COUNT",
            help = "Saves specified number of keypairs whos public key starts with the indicated prefix\nExample: --starts-with sol:4\nPREFIX type is Base58\nCOUNT type is u64"
        )]
        starts_with: String,

        #[arg(
            long,
            value_name = "SUFFIX:COUNT",
            help = "Saves specified number of keypairs whos public key ends with the indicated suffix\nExample: --ends-with ana:4\nSUFFIX type is Base58\nCOUNT type is u64"
        )]
        ends_with: String,
    },

    /// Recover keypair from seed phrase and optional BIP39 passphrase
    Recover {},
}
