use std::str::FromStr;

use solana_sdk::pubkey::Pubkey;

pub mod create_token_metadata;
pub mod init_config;
pub mod init_vault;

pub const JITO_VAULT_PROGRAM_ID: &str = "AE7fSUJSGxMzjNxSPpNTemrz9cr26RFue4GwoJ1cuR6f";
pub const JITO_RESTAKING_PROGRAM_ID: &str = "E5YF9Um1mwQWHffqaUEUwtwnhQKsbMEt33qtvjto3NDZ";

pub fn jito_vault_program_id() -> Pubkey {
    Pubkey::from_str(&JITO_VAULT_PROGRAM_ID).expect("Fail to read jito vault program_id")
}

pub fn jito_restaking_program_id() -> Pubkey {
    Pubkey::from_str(&JITO_RESTAKING_PROGRAM_ID).expect("Fail to read jito restaking program_id")
}
