use std::{borrow::Cow, path::Path, sync::Arc, time::Instant};

use bdk::{bitcoin::Network, FeeRate};
use futures::Future;
use serde::{Deserialize, Serialize};
use serde_json::from_value;
use tokio::fs::remove_dir_all;
use tracing::info;

use crate::{
    bdk_cli::{exec_with_json_output, try_exec_with_json_output, WALLET_DIR_PERMIT},
    estimate_fee::get_vbytes,
};

#[derive(Debug)]
pub struct BdkCli {
    pub network: Network,
    pub cli_path: Arc<Path>,
    pub cli_path_patched: Arc<Path>,
    pub temp_wallet_dir: Option<Arc<Path>>,
    pub descriptor: Option<String>,
}

#[allow(dead_code)]
impl BdkCli {
    pub async fn new(
        network: Network,
        cli_path: Arc<Path>,
        cli_path_patched: Arc<Path>,
        temp_wallet_dir: Option<Arc<Path>>,
        descriptor: Option<String>,
    ) -> Self {
        assert!(tokio::fs::try_exists(&cli_path).await.unwrap());
        assert!(tokio::fs::try_exists(&cli_path_patched).await.unwrap());

        if let Some(temp_wallet_dir) = &temp_wallet_dir {
            // Not exists
            assert!(!tokio::fs::try_exists(temp_wallet_dir).await.unwrap());
        }

        Self {
            network,
            cli_path,
            cli_path_patched,
            temp_wallet_dir,
            descriptor,
        }
    }

    pub async fn generate_key(&self) -> CliGenerateKeyResult {
        let result = exec_with_json_output(
            &["--network", &self.network.to_string(), "key", "generate"],
            self.cli_path.as_ref(),
        )
        .await;
        let GenerateKeyInnerResult {
            fingerprint,
            mnemonic,
            xprv,
        } = from_value(result).unwrap();

        let xpub = self.get_pubkey(&xprv).await;

        CliGenerateKeyResult {
            fingerprint,
            mnemonic,
            xprv,
            xpub,
        }
    }

    // export XPUB_00=$(bdk-cli key derive --xprv $XPRV_00 --path "m/84'/1'/0'/0" | jq -r ".xpub")
    pub async fn get_pubkey(&self, xprv: &str) -> String {
        let result = exec_with_json_output(
            &[
                "--network",
                &self.network.to_string(),
                "key",
                "derive",
                "--xprv",
                xprv,
                "--path",
                "m/84'/1'/0'/0",
            ],
            self.cli_path.as_ref(),
        )
        .await;
        let result: GetPubkeyInnerResult = from_value(result).unwrap();
        result.xpub
    }

    pub async fn get_multi_descriptor(
        &self,
        xprv_00: &str,
        xpub_01: &str,
        xpub_02: &str,
        xpub_03: &str,
    ) -> String {
        let descriptor_00 = format!("{xprv_00}/84h/1h/0h/0/*");
        // let _descriptor_02 = format!("{xprv_02}/84h/1h/0h/0/*");

        // export MULTI_DESCRIPTOR_00=$(bdk-cli compile "thresh(3,pk($DESCRIPTOR_00),pk($XPUB_01),pk($XPUB_02))" | jq -r '.descriptor')
        let desc_00 =
            format!("thresh(3,pk({descriptor_00}),pk({xpub_01}),pk({xpub_02}),pk({xpub_03}))");
        let multi_descriptor_00_ = exec_with_json_output(
            &["--network", &self.network.to_string(), "compile", &desc_00],
            self.cli_path.as_ref(),
        )
        .await;
        let multi_descriptor_00 = multi_descriptor_00_["descriptor"]
            .as_str()
            .unwrap()
            .to_owned();
        multi_descriptor_00
    }

    pub async fn get_pub_multi_descriptor(
        &self,
        xpub_00: &str,
        xpub_01: &str,
        xpub_02: &str,
        xpub_03: &str,
    ) -> String {
        // export MULTI_DESCRIPTOR_01=$(bdk-cli compile "thresh(3,pk($XPUB_00),pk($XPUB_01),pk($XPUB_02))" | jq -r '.descriptor')
        let desc_01 = format!("thresh(3,pk({xpub_00}),pk({xpub_01}),pk({xpub_02}),pk({xpub_03}))");
        let multi_descriptor_01_ = exec_with_json_output(
            &["--network", &self.network.to_string(), "compile", &desc_01],
            self.cli_path.as_ref(),
        )
        .await;
        let multi_descriptor_01 = multi_descriptor_01_["descriptor"]
            .as_str()
            .unwrap()
            .to_owned();
        multi_descriptor_01
    }

    pub async fn get_multi_address(&self, multi_descriptor_00: &str) -> String {
        // Clear temporary bdk cache
        let multi_address_ = self
            .with_temp_wallet_dir(|| async {
                exec_with_json_output(
                    self.wallet_args(multi_descriptor_00, &["get_new_address"])
                        .iter(),
                    self.cli_path.as_ref(),
                )
                .await
                // bdk_cli_wallet_inner(multi_descriptor_00, &["get_new_address"], &self.cli_path)
                //     .await
            })
            .await;
        // let multi_address_ =
        //     bdk_cli_wallet_temp_inner(multi_descriptor_00, &["get_new_address"], &self.cli_path)
        //         .await;
        let multi_address = multi_address_["address"].as_str().unwrap().to_owned();
        multi_address
    }

    fn wallet_args(&self, descriptor: &str, args: &[&str]) -> Vec<String> {
        let network = self.network.to_string();
        let mut cli_args = vec![
            "--network",
            &network,
            "wallet",
            // "--wallet",
            // "wallet_name_temp", // TODO: config
            "--descriptor",
            descriptor,
        ];
        cli_args.extend_from_slice(args);
        cli_args.into_iter().map(|s| s.to_owned()).collect()
    }

    fn wallet_args_online(&self, descriptor: &str, args: &[&str]) -> Vec<String> {
        let is_electrum = true;

        let server = if is_electrum {
            // cargo install bdk-cli --features compiler,electrum
            match self.network {
                Network::Bitcoin => "ssl://electrum.blockstream.info:50002",
                Network::Testnet => "ssl://electrum.blockstream.info:60002",
                Network::Signet => todo!(),
                Network::Regtest => todo!(),
                _ => todo!(),
            }
        } else {
            // cargo install bdk-cli --features compiler,esplora-ureq
            // https://github.com/Blockstream/esplora/blob/master/API.md
            match self.network {
                Network::Bitcoin => "https://blockstream.info/api",
                Network::Testnet => "https://blockstream.info/testnet/api",
                Network::Signet => todo!(),
                Network::Regtest => todo!(),
                _ => todo!(),
            }
        };

        let network = self.network.to_string();
        let mut cli_args = vec![
            "--network",
            &network,
            "wallet",
            "--server",
            server,
            // "--stop_gap", // Slows down electrum
            // "1",
            // "--conc",
            // "8",
            "--timeout",
            "15",
            // "--wallet",
            // "wallet_name_temp", // TODO: config
            "--descriptor",
            descriptor,
        ];
        cli_args.extend_from_slice(args);
        cli_args.into_iter().map(|s| s.to_owned()).collect()
    }

    pub async fn estimate_fee(
        &self,
        multi_descriptor_00: &str,
        to_address: &str,
        amount: &str,
        fee_rate: FeeRate,
    ) -> Result<(u64, u64), &'static str> {
        let estimate_fee_result = self
            .with_temp_wallet_dir(|| async {
                // bdk-cli wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 sync
                let start_sync = Instant::now();
                let sync_output = exec_with_json_output(
                    self.wallet_args_online(&multi_descriptor_00, &["sync"])
                        .iter(),
                    self.cli_path.as_ref(),
                )
                .await;
                info!("sync output: {sync_output:?}");
                info!("sync took: {:?}", start_sync.elapsed());

                // bdk-cli wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 get_balance | jq
                let get_balance_result = exec_with_json_output(
                    self.wallet_args(&multi_descriptor_00, &["get_balance"])
                        .iter(),
                    self.cli_path.as_ref(),
                )
                .await;
                info!("get_balance_result: {get_balance_result:?}");

                let confirmed = get_balance_result["satoshi"]["confirmed"]
                    .as_number()
                    .unwrap()
                    .as_u64()
                    .unwrap();
                if confirmed == 0 {
                    return Err("Confirmed balance is zero");
                }
                let amount: u64 = amount.parse().unwrap();
                if amount > confirmed {
                    return Err("Confirmed balance less than withdraw amount");
                }

                // export CHANGE_ID=$(bdk-cli wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 policies | jq -r ".external.id")
                let change_id_ = exec_with_json_output(
                    self.wallet_args(&multi_descriptor_00, &["policies"]).iter(),
                    self.cli_path.as_ref(),
                )
                .await;
                let change_id = change_id_["external"]["id"].as_str().unwrap();

                // Trying to calculate `send_amount`. Creating test transaction to figure out fees.
                // Then deduct the fees from the provided amount.
                let test_fees_full_amount = try_exec_with_json_output(
                    self.wallet_args(
                        &multi_descriptor_00,
                        &[
                            "create_tx",
                            "--to",
                            &format!("{to_address}:{amount}"),
                            "--external_policy",
                            &format!("{{\"{change_id}\": [0,1,3]}}"),
                            "--fee_rate",
                            &format!("{}", fee_rate.as_sat_per_vb()),
                        ],
                    )
                    .iter(),
                    self.cli_path.as_ref(),
                )
                .await;
                // Could fail with error message
                let fee = match test_fees_full_amount {
                    Ok(output_json) => {
                        let details = &output_json["details"];
                        let fee = details["fee"].as_number().unwrap().as_u64().unwrap();
                        let sent = details["sent"].as_number().unwrap().as_u64().unwrap();
                        let received = details["received"].as_number().unwrap().as_u64().unwrap();
                        let output = sent.checked_sub(received).unwrap();
                        assert_eq!(output, amount + fee);
                        fee
                    }
                    Err(output) => {
                        let stderr = String::from_utf8_lossy(&output.stderr);

                        if stderr.contains("Output below the dust limit") {
                            return Err("Output below the dust limit");
                        }

                        // Insufficient funds: 5000 sat available of 5147 sat needed
                        let pattern = " sat available of ";
                        let (before, after) = stderr.split_once(pattern).unwrap();
                        let (_rest, sat_available) = before.rsplit_once(' ').unwrap();
                        let (sat_needed, _rest) = after.split_once(' ').unwrap();
                        let sat_available: u64 = sat_available.parse().unwrap();
                        let sat_needed: u64 = sat_needed.parse().unwrap();
                        if sat_available == 0 {
                            return Err("Fee exceeds the available balance");
                        }
                        assert_eq!(sat_available, confirmed);

                        let fee = sat_needed - amount;
                        fee
                    }
                };

                let vbytes = get_vbytes(fee, fee_rate);

                Ok((fee, vbytes))
            })
            .await;

        estimate_fee_result
    }

    pub async fn onesig(
        &self,
        xprv_00: &str,
        xpub_01: &str,
        xpub_02: &str,
        xpub_03: &str,
        to_address: &str,
        amount: &str,
        fee_rate: FeeRate,
    ) -> (String, u64) {
        let multi_descriptor_00 = self
            .get_multi_descriptor(xprv_00, xpub_01, xpub_02, xpub_03)
            .await;

        let onesig_result = self
            .with_temp_wallet_dir(|| async {
                // bdk-cli wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 sync
                let start_sync = Instant::now();
                let sync_output = exec_with_json_output(
                    self.wallet_args_online(&multi_descriptor_00, &["sync"])
                        .iter(),
                    self.cli_path.as_ref(),
                )
                .await;
                info!("sync output: {:?}", sync_output);
                info!("sync took: {:?}", start_sync.elapsed());

                // bdk-cli wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 get_balance | jq
                let get_balance_result = exec_with_json_output(
                    self.wallet_args(&multi_descriptor_00, &["get_balance"])
                        .iter(),
                    self.cli_path.as_ref(),
                )
                .await;
                info!("get_balance_result: {get_balance_result:?}");

                let confirmed = get_balance_result["satoshi"]["confirmed"]
                    .as_number()
                    .unwrap()
                    .as_u64()
                    .unwrap();
                if confirmed == 0 {
                    return Err("Confirmed balance is zero");
                }
                let amount: u64 = amount.parse().unwrap();
                if amount > confirmed {
                    return Err("Confirmed balance less than withdraw amount");
                }

                // export CHANGE_ID=$(bdk-cli wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 policies | jq -r ".external.id")
                let change_id_ = exec_with_json_output(
                    self.wallet_args(&multi_descriptor_00, &["policies"]).iter(),
                    self.cli_path.as_ref(),
                )
                .await;
                let change_id = change_id_["external"]["id"].as_str().unwrap();

                // Trying to calculate `send_amount`. Creating test transaction to figure out fees.
                // Then deduct the fees from the provided amount.
                let test_fees_full_amount = try_exec_with_json_output(
                    self.wallet_args(
                        &multi_descriptor_00,
                        &[
                            "create_tx",
                            "--to",
                            &format!("{to_address}:{amount}"),
                            "--external_policy",
                            &format!("{{\"{change_id}\": [0,1,3]}}"),
                            "--fee_rate",
                            &format!("{}", fee_rate.as_sat_per_vb()),
                        ],
                    )
                    .iter(),
                    self.cli_path.as_ref(),
                )
                .await;
                // Could fail with error message
                let fee = match test_fees_full_amount {
                    Ok(output_json) => {
                        let details = &output_json["details"];
                        let fee = details["fee"].as_number().unwrap().as_u64().unwrap();
                        let sent = details["sent"].as_number().unwrap().as_u64().unwrap();
                        let received = details["received"].as_number().unwrap().as_u64().unwrap();
                        let output = sent.checked_sub(received).unwrap();
                        assert_eq!(output, amount + fee);
                        fee
                    }
                    Err(output) => {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        // Insufficient funds: 5000 sat available of 5147 sat needed
                        let pattern = " sat available of ";
                        let (before, after) = stderr.split_once(pattern).unwrap();
                        let (_rest, sat_available) = before.rsplit_once(' ').unwrap();
                        let (sat_needed, _rest) = after.split_once(' ').unwrap();
                        let sat_available: u64 = sat_available.parse().unwrap();
                        assert_eq!(sat_available, confirmed);

                        let sat_needed: u64 = sat_needed.parse().unwrap();

                        let fee = sat_needed - amount;
                        fee
                    }
                };

                // total_amount = send_amount + fee
                let send_amount = amount - fee;

                // Testing: Hardcoded
                // let send_amount = 3000;

                info!("send_amount: {send_amount}");

                // export UNSIGNED_PSBT=$(bdk-cli wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 create_tx --to $TO_ADDRESS:$AMOUNT --external_policy "{\"$CHANGE_ID\": [0,1]}" | jq -r '.psbt')
                let create_tx_result = exec_with_json_output(
                    self.wallet_args(
                        &multi_descriptor_00,
                        &[
                            "create_tx",
                            "--to",
                            &format!("{to_address}:{send_amount}"),
                            "--external_policy",
                            &format!("{{\"{change_id}\": [0,1,3]}}"),
                            "--fee_rate",
                            &format!("{}", fee_rate.as_sat_per_vb()),
                        ],
                    )
                    .iter(),
                    self.cli_path.as_ref(),
                )
                .await;

                // let details = &create_tx_result["details"];
                // let fee = details["fee"].as_number().unwrap().as_u64().unwrap();
                // let sent = details["sent"].as_number().unwrap().as_u64().unwrap();
                // let received = details["received"].as_number().unwrap().as_u64().unwrap();
                // let output = sent.checked_sub(received).unwrap();
                // assert_eq!(output, send_amount + fee);
                // assert_eq!(output, amount);

                let unsigned_psbt = create_tx_result["psbt"].as_str().unwrap();

                // export ONESIG_PSBT=$(bdk-cli wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 sign --psbt $UNSIGNED_PSBT | jq -r '.psbt')
                let onesig_psbt_ = exec_with_json_output(
                    self.wallet_args(&multi_descriptor_00, &["sign", "--psbt", unsigned_psbt])
                        .iter(),
                    self.cli_path.as_ref(),
                )
                .await;
                let onesig_psbt = onesig_psbt_["psbt"].as_str().unwrap().to_string();

                Ok((onesig_psbt, fee))
            })
            .await;

        onesig_result.unwrap()
    }

    pub async fn secondsig(
        &self,
        xpub_00: &str,
        xpub_01: &str,
        xpub_02: &str,
        xpub_03: &str,
        onesig_psbt: &str,
        key_arn: &str,
    ) -> String {
        // export MULTI_DESCRIPTOR_01=$(cat multi_descriptor_01.json)
        // export ONESIG_PSBT=$(cat onesig_psbt.json)
        // export KEY_ARN="arn:aws:kms:us-east-2:571922870935:key/17be5d9e-d752-4350-bbc1-68993fa25a4f"

        let multi_descriptor_01 = self
            .get_pub_multi_descriptor(xpub_00, xpub_01, xpub_02, xpub_03)
            .await;

        // export SECONDSIG_PSBT=$(./bdk-cli/target/release/bdk-cli wallet --aws_kms $KEY_ARN --wallet wallet_name_msd01 --descriptor $MULTI_DESCRIPTOR_01 sign --psbt $ONESIG_PSBT | jq -r '.psbt')

        let result = self
            .with_temp_wallet_dir(|| async {
                let secondsig_psbt_ = exec_with_json_output(
                    self.wallet_args(
                        &multi_descriptor_01,
                        &["--aws_kms", key_arn, "sign", "--psbt", onesig_psbt],
                    )
                    .iter(),
                    self.cli_path_patched.as_ref(),
                )
                .await;
                let secondsig_psbt = secondsig_psbt_["psbt"].as_str().unwrap();

                if onesig_psbt == secondsig_psbt {
                    return Err("Secondsig don't change PSBT");
                }

                // if !secondsig_psbt_["is_finalized"].as_bool().unwrap() {
                //     return Err("ERROR: Still not finalized after secondsig");
                // }

                Ok(secondsig_psbt.to_string())
            })
            .await;
        result.unwrap()
    }

    pub async fn thirdsig(
        &self,
        xpub_00: &str,
        xpub_01: &str,
        xpub_02: &str,
        xpub_03: &str,
        secondsig_psbt: &str,
        key_name: &str,
    ) -> String {
        // export MULTI_DESCRIPTOR_01=$(cat multi_descriptor_01.json)
        // export ONESIG_PSBT=$(cat onesig_psbt.json)
        // export KEY_ARN="arn:aws:kms:us-east-2:571922870935:key/17be5d9e-d752-4350-bbc1-68993fa25a4f"

        let pub_multi_descriptor = self
            .get_pub_multi_descriptor(xpub_00, xpub_01, xpub_02, xpub_03)
            .await;

        // export SECONDSIG_PSBT=$(./bdk-cli/target/release/bdk-cli wallet --aws_kms $KEY_ARN --wallet wallet_name_msd01 --descriptor $MULTI_DESCRIPTOR_01 sign --psbt $ONESIG_PSBT | jq -r '.psbt')

        let result = self
            .with_temp_wallet_dir(|| async {
                let key_name: Cow<str> = if !key_name.contains("/cryptoKeyVersions/") {
                    let mut key_name = key_name.to_string();
                    key_name.push_str("/cryptoKeyVersions/1");
                    key_name.into()
                } else {
                    key_name.into()
                };

                let thirdsig_psbt_ = exec_with_json_output(
                    self.wallet_args(
                        &pub_multi_descriptor,
                        &["--google_kms", &key_name, "sign", "--psbt", secondsig_psbt],
                    )
                    .iter(),
                    self.cli_path_patched.as_ref(),
                )
                .await;
                let thirdsig_psbt = thirdsig_psbt_["psbt"].as_str().unwrap();

                if secondsig_psbt == thirdsig_psbt {
                    return Err("Thirdsig don't change PSBT");
                }

                if !thirdsig_psbt_["is_finalized"].as_bool().unwrap() {
                    return Err("ERROR: Still not finalized after secondsig");
                }

                Ok(thirdsig_psbt.to_string())
            })
            .await;
        result.unwrap()
    }

    pub async fn send(
        &self,
        xpub_00: &str,
        xpub_01: &str,
        xpub_02: &str,
        xpub_03: &str,
        thirdsig_psbt: &str,
    ) -> String {
        let pub_multi_descriptor = self
            .get_pub_multi_descriptor(xpub_00, xpub_01, xpub_02, xpub_03)
            .await;

        // # broadcast
        // export TX_ID=$(bdk-cli wallet --wallet wallet_name_msd01 --descriptor $MULTI_DESCRIPTOR_01 broadcast --psbt $SECONDSIG_PSBT)
        // echo $TX_ID
        self.with_temp_wallet_dir(|| async {
            let tx_id_ = exec_with_json_output(
                self.wallet_args_online(
                    &pub_multi_descriptor,
                    &["broadcast", "--psbt", thirdsig_psbt],
                )
                .iter(),
                self.cli_path.as_ref(),
            )
            .await;
            info!("broadcast output: {tx_id_:?}");
            let tx_id = tx_id_["txid"].as_str().unwrap().to_string();

            // echo "Check: https://mempool.space/testnet/tx/$(echo $TX_ID | jq -r ".txid")"
            tx_id
        })
        .await
    }

    async fn remove_temp_wallet_dir(&self) {
        // FIXME: enable deletion?
        if let Some(temp_wallet_dir) = &self.temp_wallet_dir {
            let _ = remove_dir_all(temp_wallet_dir).await;
        }
    }

    pub async fn with_temp_wallet_dir<T, F: Future<Output = T>>(&self, f: impl FnOnce() -> F) -> T {
        let wallet_dir_permit = WALLET_DIR_PERMIT.acquire().await.unwrap();
        self.remove_temp_wallet_dir().await;
        let result = f().await;
        self.remove_temp_wallet_dir().await;
        drop(wallet_dir_permit);
        result
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliGenerateKeyResult {
    pub fingerprint: String,
    pub mnemonic: String,
    pub xprv: String,
    pub xpub: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GenerateKeyInnerResult {
    pub fingerprint: String,
    pub mnemonic: String,
    pub xprv: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GetPubkeyInnerResult {
    pub xprv: String,
    pub xpub: String,
}
