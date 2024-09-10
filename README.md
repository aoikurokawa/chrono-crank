# Chrono Crank

## Getting started

### Install Rust

```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Install Solana CLI

```
sh -c "$(curl -sSfL https://release.solana.com/v1.18.18/install)"
```

### Clone the repo

```
git clone 
```

## JITO Vault Program
https://docs.restaking.jito.network/

### Devnet 

34X2uqBhEGiWHu43RDEMwrMqXF4CpCPEZNaKdAaUS9jx

### Testnet

34X2uqBhEGiWHu43RDEMwrMqXF4CpCPEZNaKdAaUS9jx

## CLI

1. Start cranker

```bash
cargo r -- init-config -k ~/.config/solana/id.json
```

Compute units consumed: 18.256

2. init-vault

```bash
cargo r -- init-vault --vault-base-keypair-path ./keypairs/vault-base-keypair.json -l ./keypairs/lrt-mint-keypair.json --vault-admin-keypair-path ~/.config/solana/id.json -t 5S1rAwUtzJYh3gygq74GPaYsMHG67rE6tEJCXSpu114W
```

Before Boxing:
Compute units consumed: 61,566

After Boxing:
Compute units consumed: 60,733

## Example

NCN
- BSrXuoMe2N77ztNyWHfSPZpns5LsNdauURQtJPyCXpwq

Operator
- 9ifGFrivr8st3Kg5DvUP5Gc3kXp7PAta75o5ZMAM2eL9


## Help

### Create a token

```bash
spl-token create-token
```

## Resources
- https://github.com/jito-foundation/restaking
