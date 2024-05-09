use std::path::PathBuf;

use bdk::bitcoin::Network;
use futures::Future;
use serde::{Deserialize, Serialize};
use serde_json::from_value;
use tokio::fs::remove_dir_all;

use crate::bdk_cli::{exec_with_json_output, WALLET_DIR_PERMIT};

#[derive(Debug)]
pub struct BdkCli {
    pub network: Network,
    pub cli_path: PathBuf,
    pub cli_path_patched: PathBuf,
    pub temp_wallet_dir: PathBuf,
    pub descriptor: Option<String>,
}

#[allow(dead_code)]
impl BdkCli {
    pub async fn new(
        network: Network,
        cli_path: PathBuf,
        cli_path_patched: PathBuf,
        temp_wallet_dir: PathBuf,
        descriptor: Option<String>,
    ) -> Self {
        assert!(tokio::fs::try_exists(&cli_path).await.unwrap());
        assert!(tokio::fs::try_exists(&cli_path_patched).await.unwrap());

        // Not exists
        assert!(!tokio::fs::try_exists(&temp_wallet_dir).await.unwrap());

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
            &self.cli_path,
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
            &self.cli_path,
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
    ) -> String {
        let descriptor_00 = format!("{xprv_00}/84h/1h/0h/0/*");
        // let _descriptor_02 = format!("{xprv_02}/84h/1h/0h/0/*");

        // export MULTI_DESCRIPTOR_00=$(bdk-cli compile "thresh(2,pk($DESCRIPTOR_00),pk($XPUB_01),pk($XPUB_02))" | jq -r '.descriptor')
        let desc_00 = format!("thresh(2,pk({descriptor_00}),pk({xpub_01}),pk({xpub_02}))");
        let multi_descriptor_00_ = exec_with_json_output(
            &["--network", &self.network.to_string(), "compile", &desc_00],
            &self.cli_path,
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
    ) -> String {
        // export MULTI_DESCRIPTOR_01=$(bdk-cli compile "thresh(2,pk($XPUB_00),pk($XPUB_01),pk($XPUB_02))" | jq -r '.descriptor')
        let desc_01 = format!("thresh(2,pk({xpub_00}),pk({xpub_01}),pk({xpub_02}))");
        let multi_descriptor_01_ = exec_with_json_output(
            &["--network", &self.network.to_string(), "compile", &desc_01],
            &self.cli_path,
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
                    &self.cli_path,
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
            "--wallet",
            "wallet_name_temp", // TODO: config
            "--descriptor",
            descriptor,
        ];
        cli_args.extend_from_slice(args);
        cli_args.into_iter().map(|s| s.to_owned()).collect()
    }

    pub async fn onesig(
        &self,
        xprv_00: &str,
        xpub_01: &str,
        xpub_02: &str,
        to_address: &str,
        amount: &str,
    ) -> String {
        let multi_descriptor_00 = self.get_multi_descriptor(xprv_00, xpub_01, xpub_02).await;
        self.with_temp_wallet_dir(|| async {
            // bdk-cli wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 sync
            let sync_output = exec_with_json_output(
                self.wallet_args(&multi_descriptor_00, &["sync"]).iter(),
                &self.cli_path,
            )
            .await;
            dbg!(sync_output);

            // bdk-cli wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 get_balance | jq
            let get_balance_result = exec_with_json_output(
                self.wallet_args(&multi_descriptor_00, &["get_balance"])
                    .iter(),
                &self.cli_path,
            )
            .await;
            dbg!(get_balance_result);

            // export CHANGE_ID=$(bdk-cli wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 policies | jq -r ".external.id")
            let change_id_ = exec_with_json_output(
                self.wallet_args(&multi_descriptor_00, &["policies"]).iter(),
                &self.cli_path,
            )
            .await;
            let change_id = change_id_["external"]["id"].as_str().unwrap();

            // export UNSIGNED_PSBT=$(bdk-cli wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 create_tx --to $TO_ADDRESS:$AMOUNT --external_policy "{\"$CHANGE_ID\": [0,1]}" | jq -r '.psbt')
            let unsigned_psbt_ = exec_with_json_output(
                self.wallet_args(
                    &multi_descriptor_00,
                    &[
                        "create_tx",
                        "--to",
                        &format!("{to_address}:{amount}"),
                        "--external_policy",
                        &format!("{{\"{change_id}\": [0,1]}}"),
                    ],
                )
                .iter(),
                &self.cli_path,
            )
            .await;
            let unsigned_psbt = unsigned_psbt_["psbt"].as_str().unwrap();

            // export ONESIG_PSBT=$(bdk-cli wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 sign --psbt $UNSIGNED_PSBT | jq -r '.psbt')
            let onesig_psbt_ = exec_with_json_output(
                self.wallet_args(&multi_descriptor_00, &["sign", "--psbt", unsigned_psbt])
                    .iter(),
                &self.cli_path,
            )
            .await;
            let onesig_psbt = onesig_psbt_["psbt"].as_str().unwrap().to_string();

            onesig_psbt
        })
        .await
    }

    pub async fn secondsig(
        &self,
        xpub_00: &str,
        xpub_01: &str,
        xpub_02: &str,
        onesig_psbt: &str,
        key_arn: &str,
    ) -> (String, String) {
        // export MULTI_DESCRIPTOR_01=$(cat multi_descriptor_01.json)
        // export ONESIG_PSBT=$(cat onesig_psbt.json)
        // export KEY_ARN="arn:aws:kms:us-east-2:571922870935:key/17be5d9e-d752-4350-bbc1-68993fa25a4f"

        let multi_descriptor_01 = self
            .get_pub_multi_descriptor(xpub_00, xpub_01, xpub_02)
            .await;

        // export SECONDSIG_PSBT=$(./bdk-cli/target/release/bdk-cli wallet --aws_kms $KEY_ARN --wallet wallet_name_msd01 --descriptor $MULTI_DESCRIPTOR_01 sign --psbt $ONESIG_PSBT | jq -r '.psbt')

        self.with_temp_wallet_dir(|| async {
            let secondsig_psbt_ = exec_with_json_output(
                self.wallet_args(
                    &multi_descriptor_01,
                    &["--aws_kms", key_arn, "sign", "--psbt", onesig_psbt],
                )
                .iter(),
                &self.cli_path_patched,
            )
            .await;
            let secondsig_psbt = secondsig_psbt_["psbt"].as_str().unwrap();

            // if [ "$ONESIG_PSBT" = "$SECONDSIG_PSBT" ]; then
            //     echo "ERROR: Secondsig don't change PSBT"
            //     exit 1
            // fi
            assert_ne!(onesig_psbt, secondsig_psbt);

            (secondsig_psbt.to_string(), multi_descriptor_01)
        })
        .await
    }

    pub async fn send(&self, multi_descriptor_01: &str, secondsig_psbt: &str) -> String {
        // # broadcast
        // export TX_ID=$(bdk-cli wallet --wallet wallet_name_msd01 --descriptor $MULTI_DESCRIPTOR_01 broadcast --psbt $SECONDSIG_PSBT)
        // echo $TX_ID
        self.with_temp_wallet_dir(|| async {
            let tx_id_ = exec_with_json_output(
                self.wallet_args(
                    multi_descriptor_01,
                    &["broadcast", "--psbt", secondsig_psbt],
                )
                .iter(),
                &self.cli_path,
            )
            .await;
            let tx_id = tx_id_["txid"].as_str().unwrap().to_string();

            // echo "Check: https://mempool.space/testnet/tx/$(echo $TX_ID | jq -r ".txid")"
            tx_id
        })
        .await
    }

    async fn remove_temp_wallet_dir(&self) {
        let _ = remove_dir_all(&self.temp_wallet_dir).await;
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
