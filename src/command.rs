// use std::path::PathBuf;
// 
// use clap::{Parser, Subcommand};
// 
// #[derive(Parser, Debug)]
// #[command(author, version, about, long_about = None)]
// struct Cli {
//     #[command(subcommand)]
//     command: Command,
// }
// 
// #[derive(Debug, Subcommand)]
// enum Command {
//     Pubkey {},
//     New {
//         #[arg(short, long, value_name = "FILEPATH", help = "Path to generated file")]
//         outfile: PathBuf,
// 
//         #[arg(short, long, help = "Path to generated file")]
//         force: Option<String>,
// 
//         #[arg(
//             short,
//             long,
//             help = "Do not display seed phrase. Useful when piping output to other programs that prompt for user input, like gpg"
//         )]
//         silent: Option<String>,
//     },
//     Recover {},
//     Grind {},
//     Verify {},
// }
