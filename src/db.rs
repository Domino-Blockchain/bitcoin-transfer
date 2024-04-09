use std::fs::File;
use std::io::BufReader;

use base64::prelude::*;
use bitcoin::hashes::hex::ToHex;
use bitcoin::PublicKey;
use futures::TryStreamExt;
use kms_sign::parse_asn_pubkey;
use mongodb::bson::{self, doc, Document};
use mongodb::client_encryption::{ClientEncryption, MasterKey};
use mongodb::error::Result;
use mongodb::mongocrypt::ctx::{Algorithm, KmsProvider};
use mongodb::results::UpdateResult;
use mongodb::{options::ClientOptions, Client};
use mongodb::{Collection, Namespace};
use primitive_types::U256;
use tokio::fs::read_to_string;

#[allow(dead_code)]
pub struct DB {
    client: Client,
    client_decryption: Client,
    client_encryption: ClientEncryption,
    keys_collection: Collection<Document>,
}

const URI: &str = "mongodb://localhost:27017";

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

    pub async fn new() -> Self {
        // Parse a connection string into an options struct.
        let mut client_options = ClientOptions::parse("mongodb://localhost:27017")
            .await
            .unwrap();

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

        let (client_decryption, client_encryption) = DB::get_clients().await.unwrap();
        let keys_collection = client_decryption
            .database("btc")
            .collection::<Document>("keys");

        Self {
            client,
            client_decryption,
            client_encryption,
            keys_collection,
        }
    }

    #[allow(dead_code)]
    pub async fn test() -> Result<()> {
        // This must be the same master key that was used to create
        // the encryption key.
        // let mut key_bytes = vec![0u8; 96];
        // rand::thread_rng().fill(&mut key_bytes[..]);

        let key_path_owned = std::env::var("MONGODB_MASTER_KEY_PATH").unwrap();
        let key_path: &str = &shellexpand::tilde(&key_path_owned); // Expand '~' -> homedir
        let key_data = read_to_string(key_path)
            .await
            .map_err(|e| (e, key_path))
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
            ClientOptions::parse(URI).await?,
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
            .key_alt_names(["encryption_btc".to_string()])
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
        let unencrypted_coll = Client::with_uri_str(URI)
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

    #[allow(dead_code)]
    pub async fn get_address(&self) -> String {
        let db = self.client.database("btc");
        let collection = db.collection::<Document>("keys");

        // collection.drop(None).await.unwrap();
        // collection
        //     .insert_one(
        //         doc! {
        //             "address": "tb1qwv8vw6ym7dm76dzthnaglxysqsdtqy5940tram"
        //         },
        //         None,
        //     )
        //     .await
        //     .unwrap();

        let cursor = collection.find(None, None).await.unwrap();

        cursor
            .deserialize_current()
            .unwrap()
            .get("address")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string()
    }

    async fn get_clients() -> Result<(Client, ClientEncryption)> {
        let key_path_owned = std::env::var("MONGODB_MASTER_KEY_PATH").unwrap();
        // Expand '~' -> homedir
        let key_path: &str = &shellexpand::tilde(&key_path_owned);
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
            ClientOptions::parse(URI).await?,
            key_vault_namespace.clone(),
            kms_providers.clone(),
        )?
        .bypass_auto_encryption(true)
        .build()
        .await?;
        // let coll = client.database("test").collection::<Document>("coll");
        // // !!! Clear old data
        // coll.drop(None).await?;

        // // Set up the key vault (key_vault_namespace) for this example.
        // let key_vault = client
        //     .database(&key_vault_namespace.db)
        //     .collection::<Document>(&key_vault_namespace.coll);
        // key_vault.drop(None).await?;

        let client_encryption = ClientEncryption::new(
            // The MongoClient to use for reading/writing to the key vault.
            // This can be the same MongoClient used by the main application.
            client.clone(),
            key_vault_namespace.clone(),
            kms_providers.clone(),
        )?;
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
                "encryption_btc".to_string(),
                Algorithm::AeadAes256CbcHmacSha512Deterministic,
            )
            .run()
            .await?;
        to_save.insert("private_key_00", encrypted_field);

        keys_collection.insert_one(to_save, None).await?;

        // use futures::TryStreamExt;
        // let mut cursor = keys_collection.find(None, None).await.unwrap();
        // while let Some(document) = cursor.try_next().await.unwrap() {
        //     dbg!(document);
        // }

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

    pub async fn find_by_mint_address(&self, mint_address: &str) -> Result<Option<Document>> {
        let DB {
            keys_collection, ..
        } = self;

        // Dbg all mint addresses
        let mut cursor = keys_collection.find(None, None).await.unwrap();
        let mut mint_addresess = vec![];
        while let Some(document) = cursor.try_next().await.unwrap() {
            if let Some(a) = document.get("mint_address") {
                mint_addresess.push(a.clone());
            }
        }
        dbg!(mint_addresess);

        let meta = keys_collection
            .find_one(
                Some(doc! {
                    "mint_address": mint_address,
                }),
                None,
            )
            .await?;
        Ok(meta)
    }

    pub async fn update_by_deposit_address(
        &self,
        deposit_address: &str,
        update: Document,
    ) -> Result<UpdateResult> {
        let DB {
            keys_collection, ..
        } = self;
        keys_collection
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

    pub async fn get_kms_pubkey(&self, hash: U256) -> (String, String, String) {
        let file = File::open("kms_keys.json").unwrap();
        let reader = BufReader::new(file);
        let keys: serde_json::Value = serde_json::de::from_reader(reader).unwrap();
        let keys_list = keys.as_array().unwrap();

        let index: U256 = hash.checked_rem(keys_list.len().into()).unwrap();
        let index: usize = index.try_into().unwrap();

        let key = &keys_list[index];
        let key_name = key["AliasName"].as_str().unwrap().to_string();
        let key_arn = key["KeyArn"].as_str().unwrap().to_string();
        let pubkey_str = key["PublicKey"].as_str().unwrap().to_string();

        let compressed_pubkey = get_compressed_pubkey(&pubkey_str);
        // let pubkeys = ["02002c5c77d7951eaa1818a7b409181b2e4a81e93e6eb44c6fe92c637c492725bb"];
        // pubkeys[index].to_string()

        (key_name, key_arn, compressed_pubkey)
    }

    pub async fn get_all_multisig_addresses(&self) -> Vec<String> {
        let DB {
            keys_collection, ..
        } = self;
        let mut cursor = keys_collection
            .find(Some(doc! { "multi_address": { "$exists": true } }), None)
            .await
            .unwrap();
        let mut multisig_addresses = Vec::new();
        while let Some(document) = cursor.try_next().await.unwrap() {
            multisig_addresses.push(
                document
                    .get("multi_address")
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_owned(),
            );
        }
        multisig_addresses
    }
}

pub fn get_compressed_pubkey(pubkey_asn_str: &str) -> String {
    let pubkey_asn_bytes = BASE64_STANDARD.decode(pubkey_asn_str).unwrap();
    let pubkey_bytes = parse_asn_pubkey(&pubkey_asn_bytes).unwrap();
    let pubkey = PublicKey::from_slice(pubkey_bytes).unwrap();
    let compressed_pubkey = pubkey.inner.serialize().to_hex();
    compressed_pubkey
}
