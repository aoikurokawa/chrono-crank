# JSM CLI

## Vault

### Deploy the program

```bash
solana program deploy target/sbf-solana-solana/release/jito_vault_program.so --program-id ~/.config/solana/vault.json
```

### CLI

1. init-config

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
