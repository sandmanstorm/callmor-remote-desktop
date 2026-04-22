//! MinIO (S3-compatible) storage client.

use anyhow::{Context, Result};
use aws_config::BehaviorVersion;
use aws_credential_types::Credentials;
use aws_sdk_s3::config::Region;
use aws_sdk_s3::Client;

#[derive(Clone)]
pub struct Storage {
    pub client: Client,
    pub bucket_recordings: String,
}

impl Storage {
    pub async fn from_env() -> Result<Self> {
        let endpoint = std::env::var("MINIO_ENDPOINT")
            .unwrap_or_else(|_| "http://127.0.0.1:9000".into());
        let access_key = std::env::var("MINIO_ROOT_USER")
            .context("MINIO_ROOT_USER must be set")?;
        let secret_key = std::env::var("MINIO_ROOT_PASSWORD")
            .context("MINIO_ROOT_PASSWORD must be set")?;

        let creds = Credentials::new(access_key, secret_key, None, None, "callmor-static");

        let config = aws_config::defaults(BehaviorVersion::latest())
            .endpoint_url(endpoint)
            .region(Region::new("us-east-1"))
            .credentials_provider(creds)
            .load()
            .await;

        let s3_config = aws_sdk_s3::config::Builder::from(&config)
            .force_path_style(true) // MinIO requires path-style URLs
            .build();

        let client = Client::from_conf(s3_config);
        let bucket_recordings = "recordings".to_string();

        let storage = Self { client, bucket_recordings };
        storage.ensure_bucket(&storage.bucket_recordings).await?;

        Ok(storage)
    }

    async fn ensure_bucket(&self, name: &str) -> Result<()> {
        match self.client.head_bucket().bucket(name).send().await {
            Ok(_) => Ok(()),
            Err(_) => {
                self.client
                    .create_bucket()
                    .bucket(name)
                    .send()
                    .await
                    .with_context(|| format!("Failed to create bucket {name}"))?;
                tracing::info!("Created MinIO bucket: {name}");
                Ok(())
            }
        }
    }

    /// Upload bytes to the recordings bucket. Returns the object key.
    pub async fn put_recording(
        &self,
        key: &str,
        body: Vec<u8>,
        content_type: &str,
    ) -> Result<()> {
        self.client
            .put_object()
            .bucket(&self.bucket_recordings)
            .key(key)
            .body(body.into())
            .content_type(content_type)
            .send()
            .await
            .context("S3 put_object")?;
        Ok(())
    }

    /// Get object bytes from the recordings bucket.
    pub async fn get_recording(&self, key: &str) -> Result<Vec<u8>> {
        let resp = self
            .client
            .get_object()
            .bucket(&self.bucket_recordings)
            .key(key)
            .send()
            .await
            .context("S3 get_object")?;
        let bytes = resp.body.collect().await.context("read body")?.into_bytes();
        Ok(bytes.to_vec())
    }

    pub async fn delete_recording(&self, key: &str) -> Result<()> {
        self.client
            .delete_object()
            .bucket(&self.bucket_recordings)
            .key(key)
            .send()
            .await
            .context("S3 delete_object")?;
        Ok(())
    }
}
