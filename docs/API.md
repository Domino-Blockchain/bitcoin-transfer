# Endpoints

Get multisig address:
```
POST /get_address_from_db {domi_address: string} -> string
```

Get estimated fee of BTC transaction:
```
POST /estimate_fee
{
    mint_address: string,
    withdraw_address: string, // BTC
    withdraw_amount: string,
} -> {
    status: "ok",
    fee: number,
    fee_rate: number, // floating point
    vbytes: number,
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
    vbytes?: number,
} -> {
    status: "ok",
    thirdsig_psbt: string, // Finalized PSBT
    tx_id: string,
    tx_link: string, // Link to transaction on `mempool.space`
}
```