# fixtures

### SOL / SRM Aggregator Accounts

```shell
solana config set --url https://api.devnet.solana.com

# Pyth product: SOL/USD
solana account 3Mnn2fX6rQyUsyELYms1sBJyChWofzSNRoqYzvgMVz5E --output-file 3Mnn2fX6rQyUsyELYms1sBJyChWofzSNRoqYzvgMVz5E.bin
# Pyth price: SOL/USD
solana account J83w4HKfqxwcq3BEMMkPFSppX3gqekLyLJBexebFVkix --output-file J83w4HKfqxwcq3BEMMkPFSppX3gqekLyLJBexebFVkix.bin
# Pyth product: SRM/USD
solana account 6MEwdxe4g1NeAF9u6KDG14anJpFsVEa2cvr5H6iriFZ8 --output-file 6MEwdxe4g1NeAF9u6KDG14anJpFsVEa2cvr5H6iriFZ8.bin
# Pyth price: SRM/USD
solana account 992moaMQKs32GKZ9dxi8keyM2bUmbrwBZpK4p2K6X5Vs --output-file 992moaMQKs32GKZ9dxi8keyM2bUmbrwBZpK4p2K6X5Vs.bin
```

### Serum market, bid, and ask accounts

```shell
solana config set --url mainnet-beta

# not sure what this market account is for
# market account
solana account C1EuT9VokAKLiW7i2ASnZUvxDoKuKkCpDDeNxAptuNe4 --output-file C1EuT9VokAKLiW7i2ASnZUvxDoKuKkCpDDeNxAptuNe4.bin
# bid account
solana account 2e2bd5NtEGs6pb758QHUArNxt6X9TTC5abuE1Tao6fhS --output-file 2e2bd5NtEGs6pb758QHUArNxt6X9TTC5abuE1Tao6fhS.bin
# ask account
solana account F1tDtTDNzusig3kJwhKwGWspSu8z2nRwNXFWc6wJowjM --output-file F1tDtTDNzusig3kJwhKwGWspSu8z2nRwNXFWc6wJowjM.bin
```


### Test keypairs

- deltafi-owner.json: Admin keypair, pubkey - AAcLDxdg3h5ZPAu3ySrue4yE7XNH33HaTw9PrebMvEDg
