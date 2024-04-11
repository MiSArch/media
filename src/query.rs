use async_graphql::{Context, Error, Object, Result};
use bson::Uuid;
use s3::Bucket;

pub static URL_EXPIRATION_TIME: u32 = 86400;

/// Describes GraphQL invoice queries.
pub struct Query;

#[Object]
impl Query {
    /// Returns an URL for a media of a specific UUID.
    async fn get_media_url<'a>(&self, ctx: &Context<'a>, id: Uuid) -> Result<String> {
        let media_data_bucket = ctx.data::<Bucket>()?;
        let mut list_bucket_results = media_data_bucket.list(id.to_string(), None).await?;
        let message = format!("Media file of UUID: `{}` not found.", id);
        let mut list_bucket_result = list_bucket_results
            .pop()
            .ok_or(Error::new(message.clone()))?;
        let media_file_path = list_bucket_result
            .contents
            .pop()
            .ok_or(Error::new(message))?
            .key;
        let media_file_url =
            media_data_bucket.presign_get(media_file_path, URL_EXPIRATION_TIME, None)?;
        Ok(media_file_url)
    }
}
