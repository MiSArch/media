use std::{env, ffi::OsStr, path::Path};

use async_graphql::{ComplexObject, Context, Error, Result, SimpleObject};
use bson::Uuid;
use once_cell::sync::Lazy;
use s3::Bucket;
use url::Url;

/// Parses `PATH_EXPIRATION_TIME` environment variable.
/// Defaults to pre-signed URL expiration time of 1d.
/// Uses lazy evaluation.
pub static PATH_EXPIRATION_TIME: Lazy<u32> = Lazy::new(|| {
    env::var("PATH_EXPIRATION_TIME")
        .ok()
        .and_then(|path_expiration_time| path_expiration_time.parse::<u32>().ok())
        .unwrap_or(86400)
});

/// Parses `PROXY_PATH` environment variable.
/// Defaults to `/api/media`.
/// Path under which MinIO is available through a reserve proxy, for example Nginx.
/// This should be defined in the reverse proxies configuration.
pub static PROXY_PATH: Lazy<String> =
    Lazy::new(|| env::var("PROXY_PATH").unwrap_or("/api/media".to_string()));

/// Media object with associated path and id.
#[derive(Debug, SimpleObject)]
#[graphql(complex)]
pub struct Media {
    /// Media UUID.
    pub id: Uuid,
}

#[ComplexObject]
impl Media {
    /// Pre-signed path for the media.
    async fn path<'a>(&self, ctx: &Context<'a>) -> Result<String> {
        let media_data_bucket = ctx.data::<Bucket>()?;
        let mut list_bucket_results = media_data_bucket.list(self.id.to_string(), None).await?;
        let message = format!("Media file of UUID: `{}` not found.", self.id);
        let mut list_bucket_result = list_bucket_results
            .pop()
            .ok_or(Error::new(message.clone()))?;
        let media_file_name = list_bucket_result
            .contents
            .pop()
            .ok_or(Error::new(message))?
            .key;
        let media_file_url_string =
            media_data_bucket.presign_get(media_file_name, *PATH_EXPIRATION_TIME, None)?;
        let media_file_url = Url::parse(&media_file_url_string)?;
        let media_file_path = format!(
            "{}{}?{}",
            *PROXY_PATH,
            media_file_url.path(),
            media_file_url.query().unwrap_or("")
        );
        Ok(media_file_path)
    }
}

impl TryFrom<&str> for Media {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let path = Path::new(&value);
        let id_str = path
            .file_stem()
            .and_then(OsStr::to_str)
            .ok_or(Error::new("File in bucket does not have a name."))?;
        log::info!("{:?}", &id_str);
        let id = Uuid::parse_str(id_str)?;
        let media = Media { id: id };
        Ok(media)
    }
}
