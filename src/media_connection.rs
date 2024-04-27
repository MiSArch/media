use async_graphql::SimpleObject;

use crate::media::Media;

/// A connection of Medias.
#[derive(SimpleObject)]
#[graphql(shareable)]
pub struct MediaConnection {
    /// The resulting entities.
    pub nodes: Vec<Media>,
    /// Whether this connection has a next page.
    pub has_next_page: bool,
    /// The total amount of items in this connection.
    pub total_count: u64,
}
