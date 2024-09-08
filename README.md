# Chrono Crank

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


## Help

### Create a token

```bash
spl-token create-token
```

## Resources
- https://github.com/jito-foundation/restaking
