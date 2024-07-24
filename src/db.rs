use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::str::FromStr;

use base64::prelude::*;
use bitcoin::hashes::hex::ToHex;
use bitcoin::PublicKey;
use domichain_sdk::pubkey::Pubkey;
use futures::TryStreamExt;
use kms_sign::parse_asn_pubkey;
use mongodb::bson::{self, doc, Bson, Document};
use mongodb::client_encryption::{ClientEncryption, MasterKey};
use mongodb::error::Result;
use mongodb::mongocrypt::ctx::{Algorithm, KmsProvider};
use mongodb::results::{InsertOneResult, UpdateResult};
use mongodb::{options::ClientOptions, Client};
use mongodb::{Collection, Namespace};
use primitive_types::U256;
use serde::Deserialize;
use tokio::fs::read_to_string;
use tracing::info;

const DATAKEY_NAME: &str = "encryption_btc";

#[allow(dead_code)]
pub struct DB {
    client: Client,
    client_decryption: Client,
    client_encryption: ClientEncryption,
    keys_collection: Collection<Document>,
    transactions_collection: Collection<Document>,
}

impl DB {
    pub async fn print_db_structure(client: &Client) {
        let horiz_t = '├';
        let horiz_t_end = '└';
        let hline = '─';
        println!("Databases:");
        for db_name in client.list_database_names(None, None).await.unwrap() {
            println!("{db_name}");
            let db = client.database(&db_name);
            let list_collection_names = db.list_collection_names(None).await.unwrap();
            for (i, collection_name) in list_collection_names.iter().enumerate() {
                let horiz_t = if i + 1 != list_collection_names.len() {
                    horiz_t
                } else {
                    horiz_t_end
                };
                println!("{horiz_t}{hline}{collection_name}");
            }
        }
    }

    pub async fn new(mongodb_uri: &str, mongodb_master_key_path: &Path) -> Self {
        // Parse a connection string into an options struct.
        let mut client_options = ClientOptions::parse(&mongodb_uri).await.unwrap();

        // Manually set an option.
        client_options.app_name = Some("BTC app".to_string());

        // Get a handle to the deployment.
        let client = Client::with_options(client_options).unwrap();

        Self::print_db_structure(&client).await;

        // // Get a handle to a collection in the database.
        // let collection = db.collection::<Document>("keys");
        // let mut cursor = collection.find(None, None).await.unwrap();

        // // Iterate over the results of the cursor.
        // while let Some(data) = cursor.try_next().await.unwrap() {
        //     println!("data: {data:?}");
        // }

        let (client_decryption, client_encryption) =
            DB::get_clients(&mongodb_uri, mongodb_master_key_path)
                .await
                .unwrap();
        let keys_collection = client_decryption
            .database("btc")
            .collection::<Document>("keys");
        let transactions_collection = client_decryption
            .database("btc")
            .collection::<Document>("transactions");

        Self {
            client,
            client_decryption,
            client_encryption,
            keys_collection,
            transactions_collection,
        }
    }

    #[allow(dead_code)]
    pub async fn test(mongodb_uri: &str, mongodb_master_key_path: &Path) -> Result<()> {
        // This must be the same master key that was used to create
        // the encryption key.
        // let mut key_bytes = vec![0u8; 96];
        // rand::thread_rng().fill(&mut key_bytes[..]);

        let key_data = read_to_string(mongodb_master_key_path)
            .await
            .map_err(|e| (e, mongodb_master_key_path))
            .unwrap();
        let key_bytes = BASE64_STANDARD.decode(key_data).unwrap();

        let local_master_key = bson::Binary {
            subtype: bson::spec::BinarySubtype::Generic,
            bytes: key_bytes,
        };
        let kms_providers = vec![(KmsProvider::Local, doc! { "key": local_master_key }, None)];

        // The MongoDB namespace (db.collection) used to store
        // the encryption data keys.
        let key_vault_namespace = Namespace::new("keyvault", "datakeys");

        // `bypass_auto_encryption(true)` disables automatic encryption but keeps
        // the automatic _decryption_ behavior. bypass_auto_encryption will
        // also disable spawning mongocryptd.
        let client = Client::encrypted_builder(
            ClientOptions::parse(&mongodb_uri).await?,
            key_vault_namespace.clone(),
            kms_providers.clone(),
        )?
        .bypass_auto_encryption(true)
        .build()
        .await?;
        let coll = client.database("test").collection::<Document>("coll");
        // !!! Clear old data
        coll.drop(None).await?;

        // Set up the key vault (key_vault_namespace) for this example.
        let key_vault = client
            .database(&key_vault_namespace.db)
            .collection::<Document>(&key_vault_namespace.coll);
        key_vault.drop(None).await?;

        let client_encryption = ClientEncryption::new(
            // The MongoClient to use for reading/writing to the key vault.
            // This can be the same MongoClient used by the main application.
            client,
            key_vault_namespace.clone(),
            kms_providers.clone(),
        )?;

        // Create a new data key for the encryptedField.
        let data_key_id = client_encryption
            .create_data_key(MasterKey::Local)
            .key_alt_names([DATAKEY_NAME.to_string()])
            .run()
            .await?;

        // Explicitly encrypt a field:
        let encrypted_field = client_encryption
            .encrypt(
                "123456789",
                data_key_id,
                Algorithm::AeadAes256CbcHmacSha512Deterministic,
            )
            .run()
            .await?;
        coll.insert_one(doc! { "encryptedField": encrypted_field }, None)
            .await?;

        // Automatically decrypts any encrypted fields.
        let doc = coll.find_one(None, None).await?.unwrap();
        println!(
            "Decrypted document: {:?}",
            doc.get("encryptedField").unwrap().as_str().unwrap()
        );
        let unencrypted_coll = Client::with_uri_str(&mongodb_uri)
            .await?
            .database("test")
            .collection::<Document>("coll");
        println!(
            "Encrypted document: {:?}",
            unencrypted_coll
                .find_one(None, None)
                .await?
                .unwrap()
                .get("encryptedField")
                .unwrap()
        );

        Ok(())
    }

    async fn get_clients(mongodb_uri: &str, key_path: &Path) -> Result<(Client, ClientEncryption)> {
        let key_data = read_to_string(key_path)
            .await
            .map_err(|e| (e, key_path))
            .unwrap();
        let key_bytes = BASE64_STANDARD.decode(key_data).unwrap();

        let local_master_key = bson::Binary {
            subtype: bson::spec::BinarySubtype::Generic,
            bytes: key_bytes,
        };
        let kms_providers = vec![(
            KmsProvider::Local,
            doc! {
                "key": local_master_key,
            },
            None,
        )];

        // The MongoDB namespace (db.collection) used to store
        // the encryption data keys.
        let key_vault_namespace = Namespace::new("keyvault", "datakeys");

        // `bypass_auto_encryption(true)` disables automatic encryption but keeps
        // the automatic _decryption_ behavior. bypass_auto_encryption will
        // also disable spawning mongocryptd.
        let client = Client::encrypted_builder(
            ClientOptions::parse(mongodb_uri).await?,
            key_vault_namespace.clone(),
            kms_providers.clone(),
        )?
        .bypass_auto_encryption(true)
        .build()
        .await?;

        // Set up the key vault (key_vault_namespace)
        let key_vault = client
            .database(&key_vault_namespace.db)
            .collection::<Document>(&key_vault_namespace.coll);
        let datakey = key_vault.find_one(None, None).await.unwrap();

        let client_encryption = ClientEncryption::new(
            // The MongoClient to use for reading/writing to the key vault.
            // This can be the same MongoClient used by the main application.
            client.clone(),
            key_vault_namespace.clone(),
            kms_providers.clone(),
        )?;

        if datakey.is_none() {
            // Create a new data key for the encrypted fields
            let data_key_id = client_encryption
                .create_data_key(MasterKey::Local)
                .key_alt_names([DATAKEY_NAME.to_string()])
                .run()
                .await?;
            info!("Created datakey: {data_key_id}");
        }

        Ok((client, client_encryption))
    }

    pub async fn save_private_key(
        &self,
        to_save_encrypted: Document,
        mut to_save: Document,
    ) -> Result<()> {
        let DB {
            client_encryption,
            keys_collection,
            ..
        } = self;

        // Explicitly encrypt a field:
        let encrypted_field = client_encryption
            .encrypt(
                serde_json::to_string(&to_save_encrypted).unwrap(),
                DATAKEY_NAME.to_string(),
                Algorithm::AeadAes256CbcHmacSha512Deterministic,
            )
            .run()
            .await?;
        to_save.insert("private_key_00", encrypted_field);

        keys_collection.insert_one(to_save, None).await?;

        Ok(())
    }

    pub async fn find_by_deposit_address(&self, deposit_address: &str) -> Result<Option<Document>> {
        let DB {
            keys_collection, ..
        } = self;
        let meta = keys_collection
            .find_one(
                Some(doc! {
                    "multi_address": deposit_address,
                }),
                None,
            )
            .await?;
        Ok(meta)
    }

    pub async fn find_by_mint_address(
        &self,
        mint_address: &str,
    ) -> Result<Option<(Document, Document)>> {
        let DB {
            transactions_collection,
            keys_collection,
            ..
        } = self;

        let transaction = if let Some(transaction) = transactions_collection
            .find_one(
                Some(doc! {
                    "mint_address": mint_address,
                }),
                None,
            )
            .await?
        {
            transaction
        } else {
            return Ok(None);
        };
        let multi_address = transaction.get("multi_address").unwrap().as_str().unwrap();
        if let Some(key) = keys_collection
            .find_one(
                Some(doc! {
                    "multi_address": multi_address,
                }),
                None,
            )
            .await?
        {
            Ok(Some((transaction, key)))
        } else {
            Ok(None)
        }
    }

    pub async fn update_by_deposit_address(
        &self,
        deposit_address: &str,
        update: Document,
    ) -> Result<UpdateResult> {
        self.keys_collection
            .update_one(
                doc! {
                    "multi_address": deposit_address,
                },
                doc! {
                    "$set": update,
                },
                None,
            )
            .await
    }

    /// Insert a new unique BTC transaction. Checks uniqueness
    pub async fn insert_tx(&self, insert: Document) -> Result<InsertOneResult> {
        // Check that TX hash is unique
        let new_tx_hash = insert.get_str("tx_hash").unwrap();
        let existing_tx = self
            .transactions_collection
            .find_one(Some(doc! {"tx_hash": new_tx_hash}), None)
            .await
            .unwrap();
        assert_eq!(existing_tx, None);

        self.transactions_collection.insert_one(insert, None).await
    }

    pub async fn update_tx(&self, id: Bson, update: Document) -> Result<UpdateResult> {
        self.transactions_collection
            .update_one(
                doc! {
                    "_id": id,
                },
                doc! {
                    "$set": update,
                },
                None,
            )
            .await
    }

    /// Get info about all AWS KMS keys and choose one based on hash
    pub async fn get_aws_kms_pubkey(&self, hash: U256) -> (String, String, String) {
        #[allow(dead_code)]
        #[derive(Deserialize)]
        #[serde(rename_all = "PascalCase")]
        struct AwsKmsKey {
            alias_name: String,
            alias_arn: String,
            target_key_id: String,
            creation_date: String,
            last_updated_date: String,
            key_arn: String,
            public_key: String,
        }

        let file = File::open("aws_kms_keys.json").unwrap();
        let reader = BufReader::new(file);
        let keys: Vec<AwsKmsKey> = serde_json::de::from_reader(reader).unwrap();

        let index: U256 = hash.checked_rem(keys.len().into()).unwrap();
        let index: usize = index.try_into().unwrap();

        let key = keys.into_iter().nth(index).unwrap();
        let key_name = key.alias_name;
        let key_arn = key.key_arn;
        let pubkey_str = key.public_key;

        let compressed_pubkey = get_compressed_pubkey(&pubkey_str);

        (key_name, key_arn, compressed_pubkey)
    }

    /// Get info about all Google KMS keys and choose one based on hash
    pub async fn get_google_kms_pubkey(&self, hash: U256) -> (String, String) {
        #[allow(dead_code)]
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct GoogleKmsKey {
            create_time: String,
            name: String,
            public_key: String,
        }

        let file = File::open("google_kms_keys.json").unwrap();
        let reader = BufReader::new(file);
        let keys: Vec<GoogleKmsKey> = serde_json::de::from_reader(reader).unwrap();

        let index: U256 = hash.checked_rem(keys.len().into()).unwrap();
        let index: usize = index.try_into().unwrap();

        let key = keys.into_iter().nth(index).unwrap();
        let key_name = key.name;
        let pubkey_str = key.public_key;

        let compressed_pubkey = get_compressed_pubkey(&pubkey_str);

        (key_name, compressed_pubkey)
    }

    pub async fn get_all_multisig_addresses(&self) -> Vec<String> {
        self.keys_collection
            .find(Some(doc! { "multi_address": { "$exists": true } }), None)
            .await
            .unwrap()
            .map_ok(|document| document.get_str("multi_address").unwrap().to_string())
            .try_collect()
            .await
            .unwrap()
    }

    pub async fn get_all_mints(&self) -> Vec<Pubkey> {
        self.transactions_collection
            .find(Some(doc! { "mint_address": { "$exists": true } }), None)
            .await
            .unwrap()
            .map_ok(|document| {
                document
                    .get_str("mint_address")
                    .unwrap()
                    .parse::<Pubkey>()
                    .unwrap()
            })
            .try_collect()
            .await
            .unwrap()
    }

    pub async fn get_multisig_address_to_mint_addresses_mapping(
        &self,
    ) -> HashMap<String, Vec<Pubkey>> {
        let DB {
            transactions_collection,
            ..
        } = self;

        let multisig_addresses = self.get_all_multisig_addresses().await;
        // All unique
        assert_eq!(
            multisig_addresses.len(),
            HashSet::<&String>::from_iter(&multisig_addresses).len()
        );

        let mut mapping =
            HashMap::from_iter(multisig_addresses.into_iter().map(|a| (a, Vec::new())));

        for (multisig_address, mint_addresses) in mapping.iter_mut() {
            let txs: Vec<_> = transactions_collection
                .find(
                    Some(doc! {
                        "multi_address": multisig_address,
                        "mint_address": { "$exists": true },
                    }),
                    None,
                )
                .await
                .unwrap()
                .try_collect()
                .await
                .unwrap();
            mint_addresses.extend(
                txs.into_iter()
                    .map(|tx| Pubkey::from_str(tx.get_str("mint_address").unwrap()).unwrap()),
            );
        }

        mapping
    }
}

pub fn get_compressed_pubkey(pubkey_asn_str: &str) -> String {
    let pubkey_asn_bytes = BASE64_STANDARD.decode(pubkey_asn_str).unwrap();
    let pubkey_bytes = parse_asn_pubkey(&pubkey_asn_bytes).unwrap();
    let pubkey = PublicKey::from_slice(pubkey_bytes).unwrap();
    let compressed_pubkey = pubkey.inner.serialize().to_hex();
    compressed_pubkey
}

#[tokio::test]
async fn test_get_multisig_address_to_mint_addresses_mapping() {
    use clap::Parser;
    use std::sync::Arc;

    kms_sign::load_dotenv();
    let v: Vec<String> = vec![];
    let args = crate::Args::parse_from(v);
    assert!(args.mongodb_master_key_path.exists());

    let db = Arc::new(DB::new(&args.mongodb_uri, &args.mongodb_master_key_path).await);

    dbg!(db.get_multisig_address_to_mint_addresses_mapping().await);
}

#[tokio::test]
async fn test_get_all_mints() {
    use clap::Parser;
    use std::sync::Arc;

    kms_sign::load_dotenv();
    let v: Vec<String> = vec![];
    let args = crate::Args::parse_from(v);
    assert!(args.mongodb_master_key_path.exists());

    let db = Arc::new(DB::new(&args.mongodb_uri, &args.mongodb_master_key_path).await);

    dbg!(db.get_all_mints().await);
}

#[tokio::test]
async fn test_get_all_multisig_addresses() {
    use clap::Parser;
    use std::sync::Arc;

    kms_sign::load_dotenv();
    let v: Vec<String> = vec![];
    let args = crate::Args::parse_from(v);
    assert!(args.mongodb_master_key_path.exists());

    let db = Arc::new(DB::new(&args.mongodb_uri, &args.mongodb_master_key_path).await);

    dbg!(db.get_all_multisig_addresses().await);
}
