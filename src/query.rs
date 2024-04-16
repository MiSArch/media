use async_graphql::{Context, Error, Object, Result};
use bson::Uuid;
use s3::Bucket;
use url::Url;

/// Defines pre-signed URL expiration time of 1d.
pub static URL_EXPIRATION_TIME: u32 = 86400;

/// Describes GraphQL invoice queries.
pub struct Query;

#[Object]
impl Query {
    /// Returns an URL for a media of a specific UUID.
    async fn get_media_url<'a>(&self, ctx: &Context<'a>, #[graphql(desc = "UUID of media to retrieve.")] id: Uuid) -> Result<String> {
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
        let adapted_media_file_url = adapt_url_to_rewrite_domain(media_file_url, &ctx)?;
        Ok(adapted_media_file_url)
    }
}

/// Uses the given `rewrite_domain` argument to adapt to the domain of the given URL accordingly.
fn adapt_url_to_rewrite_domain<'a>(url: String, ctx: &Context<'a>) -> Result<String> {
    let mut rewrite_domain = ctx.data::<Url>()?.clone();
    let parsed_url = Url::parse(&url)?;
    let parsed_url_path = parsed_url.path();
    let parsed_query = parsed_url.query();
    rewrite_domain.set_path(parsed_url_path);
    rewrite_domain.set_query(parsed_query);
    Ok(rewrite_domain.to_string())
}
