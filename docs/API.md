# Endpoints

### Service health:
```
GET /health

RESPONSE:
{ status: "ok" }
```

### Get multisig address:
```
POST /get_address_from_db {domi_address: string} -> string
```

### Get estimated fee of BTC transaction:

`fee = fee_rate * vbytes`

```
POST /estimate_fee
{
    mint_address: string,
    withdraw_address: string, // BTC
    withdraw_amount: string
}

SUCCESS RESPONSE:
{
    status: "ok",
    vbytes: number,
    recommended_fee_rates: {
        fastest_fee: number,
        half_hour_fee: number,
        hour_fee: number,
        economy_fee: number,
        minimum_fee: number
    }
}

FAILURE RESPONSE:
{
    status: "error",
    message: string,
}
```

### Sign & send BTC transaction:

`signature` field must be created by signing JSON with all fields except `signature` by key of `domi_address` wallet.

```
POST /sign_multisig_tx
{
    mint_address: string,
    withdraw_address: string, // BTC
    withdraw_amount: string,
    fee_rate: optional number, // floating point
    vbytes: optional number,
    domi_address: string, // Address of Domichain wallet
    block_height: number, // Latest blockheight in Domichain network
    btci_tx_signature: string, // Signature of BTCi transfer transaction
    signature: string // Signature of this POST request by `domi_address` wallet key
}

SUCESS RESPONSE:
{
    status: "ok",
    thirdsig_psbt: string, // Finalized PSBT
    tx_id: string, // BTC TX hash
    tx_link: string // Link to transaction on `mempool.space`
}

FAILURE RESPONSE:
{
    status: "error",
    message: string,
}
```
