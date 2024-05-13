use async_graphql::{Context, Error, Object, Result, Upload};
use bson::Uuid;
use s3::Bucket;
use tokio::fs::File;
use tokio::io::AsyncReadExt;

use crate::authorization::authorize_user;
use crate::event::model::media_dto::MediaDTO;

/// Describes GraphQL media mutations.
pub struct Mutation;

#[Object]
impl Mutation {
    /// Uploads a media to MinIO.
    ///
    /// Saves the file under a new UUID with its suitable file extension in MinIO.
    /// Sends an event when a media file was uploaded.
    async fn upload_media<'a>(
        &self,
        ctx: &Context<'a>,
        #[graphql(desc = "Media file to upload.")] media_file: Upload,
    ) -> Result<Uuid> {
        authorize_user(&ctx, None)?;
        let media_data_bucket = ctx.data::<Bucket>()?;
        let media_file_value = media_file.value(&ctx)?;
        let missing_content_type_error = Error::new("Content type of file upload does not exist.");
        let media_file_type = media_file_value
            .content_type
            .ok_or(missing_content_type_error.clone())?;
        let media_file_mime = media_file_type.parse::<mime::Mime>()?;
        let media_file_extension = media_file_mime.subtype();
        let mut buffer = Vec::new();
        let mut media_file_content_async = File::from_std(media_file_value.content);
        media_file_content_async.read_to_end(&mut buffer).await?;
        let media_file_id = Uuid::new();
        let media_file_path = format!("{}.{}", media_file_id.to_string(), media_file_extension);
        let response_data = media_data_bucket
            .put_object(media_file_path, &buffer)
            .await?;
        send_media_created_event(media_file_id).await?;
        match response_data.status_code() {
            200 => Ok(media_file_id),
            _ => Err(Error::new("Media file could not be inserted into MinIO.")),
        }
    }
}

/// Sends an `media/media/created` created event containing the media UUID.
///
/// * `media_file_id` - UUID of media file which was created/uploaded.
async fn send_media_created_event(media_file_id: Uuid) -> Result<()> {
    let client = reqwest::Client::new();
    let media_dto = MediaDTO { id: media_file_id };
    client
        .post("http://localhost:3500/v1.0/publish/pubsub/media/media/created")
        .json(&media_dto)
        .send()
        .await?;
    Ok(())
}
