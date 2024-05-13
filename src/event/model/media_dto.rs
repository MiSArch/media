use bson::Uuid;
use serde::Serialize;

/// DTO of a media.
#[derive(Debug, Serialize)]
pub struct MediaDTO {
    pub id: Uuid,
}
