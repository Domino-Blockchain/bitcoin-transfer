# Endpoints

Get multisig address:
```
POST /get_address_from_db {domi_address: string} -> string
```

Get estimated fee of BTC transaction:

`fee = fee_rate * vbytes`

```
POST /estimate_fee
{
    mint_address: string,
    withdraw_address: string, // BTC
    withdraw_amount: string
} -> {
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
```

Sign & send BTC transaction:
```
POST /sign_multisig_tx 
{
    mint_address: string,
    withdraw_address: string, // BTC
    withdraw_amount: string,
    fee_rate?: number, // floating point
    vbytes?: number
} -> {
    status: "ok",
    thirdsig_psbt: string, // Finalized PSBT
    tx_id: string,
    tx_link: string // Link to transaction on `mempool.space`
}
```