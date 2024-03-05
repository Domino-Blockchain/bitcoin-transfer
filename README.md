# Bitcoin transfer

```
curl http://0.0.0.0:3000/get_address
curl -X POST http://0.0.0.0:3000/check_balance | jq
curl -X POST http://193.107.109.22:3000/mint_token | jq
```


TODO:

- include spl-token mint as Rust dep
- Write test for sending BTC


- Add DB for user mint requests, track all transfers/mints locally
- Add an entry point for users to get their bitcoins back
- Automate multi-sig BTC TX signing