# Chrono Crank

## Getting started

Run a cranker

```bash
 cargo r -- --rpc-url {} --keypair {} --vault-program-id {} --restaking-program-id {} --ncn {}
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


## Resources
- https://github.com/jito-foundation/restaking
