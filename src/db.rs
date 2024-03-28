use base64::prelude::*;
use futures::TryStreamExt;
use mongodb::bson::{self, doc, Document};
use mongodb::client_encryption::{ClientEncryption, MasterKey};
use mongodb::error::Result;
use mongodb::mongocrypt::ctx::{Algorithm, KmsProvider};
use mongodb::Namespace;
use mongodb::{options::ClientOptions, Client};
use tokio::fs::read_to_string;

pub struct DB {
    client: Client,
}

const URI: &str = "mongodb://localhost:27017";

impl DB {
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
        // Clear old data
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
            .key_alt_names(["encryption_example_4".to_string()])
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

    pub async fn new() -> Self {
        // Parse a connection string into an options struct.
        let mut client_options = ClientOptions::parse("mongodb://localhost:27017")
            .await
            .unwrap();

        // Manually set an option.
        client_options.app_name = Some("BTC app".to_string());

        // Get a handle to the deployment.
        let client = Client::with_options(client_options).unwrap();

        println!("- db_name");
        // List the names of the databases in that deployment.
        for db_name in client.list_database_names(None, None).await.unwrap() {
            println!("{}", db_name);
        }

        // Get a handle to a database.
        let db = client.database("btc");

        println!("- collection_name");
        // List the names of the collections in that database.
        for collection_name in db.list_collection_names(None).await.unwrap() {
            println!("{}", collection_name);
        }

        // Get a handle to a collection in the database.
        let collection = db.collection::<Document>("keys");
        let mut cursor = collection.find(None, None).await.unwrap();

        // Iterate over the results of the cursor.
        while let Some(data) = cursor.try_next().await.unwrap() {
            println!("data: {data:?}");
        }

        Self { client }
    }

    pub async fn get_address(&self) -> String {
        let db = self.client.database("btc");
        let collection = db.collection::<Document>("keys");

        collection.drop(None).await.unwrap();
        collection
            .insert_one(
                doc! {
                    "address": "tb1qwv8vw6ym7dm76dzthnaglxysqsdtqy5940tram"
                },
                None,
            )
            .await
            .unwrap();

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
}
