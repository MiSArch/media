use std::{fs::File, io::Write};

use async_graphql::{
    extensions::Logger, http::GraphiQLSource, EmptySubscription, SDLExportOptions, Schema,
};
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};

use axum::{
    extract::State,
    http::StatusCode,
    response::{self, IntoResponse},
    routing::get,
    Router, Server,
};
use clap::{arg, command, Parser};
use s3::{creds::Credentials, Bucket, BucketConfiguration, Region};
use simple_logger::SimpleLogger;

use log::info;
use url::Url;

use crate::{mutation::Mutation, query::Query};

mod mutation;
mod query;

/// Builds the GraphiQL frontend.
async fn graphiql() -> impl IntoResponse {
    response::Html(GraphiQLSource::build().endpoint("/").finish())
}

/// Command line argument to toggle schema generation instead of service execution.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Generates GraphQL schema in `./schemas/media.graphql`.
    #[arg(long)]
    generate_schema: bool,
    /// Domain to which pre-signed URLs should be rewritten.
    #[arg(long)]
    rewrite_domain: Url,
}

/// Connects to MinIO bucket if existent, otherwise creates bucket `media-data`.
/// Warning: Change credentials before production use.
async fn initialize_minio_media_data_bucket() -> Bucket {
    let bucket_name = "media-data";
    let region = Region::Custom {
        region: "eu-central-1".to_string(),
        endpoint: "http://media-minio:9000".to_string(),
    };
    let credentials = Credentials::new(Some("admin"), Some("password"), None, None, None).unwrap();

    match Bucket::create_with_path_style(
        bucket_name,
        region.clone(),
        credentials.clone(),
        BucketConfiguration::default(),
    )
    .await
    {
        Ok(bucket_response) => bucket_response.bucket,
        Err(_) => Bucket::new(bucket_name, region, credentials).unwrap(),
    }
    .with_path_style()
}

/// Activates logger and parses argument for optional schema generation. Otherwise starts gRPC and GraphQL server.
#[tokio::main]
async fn main() -> std::io::Result<()> {
    SimpleLogger::new().init().unwrap();

    let args = Args::parse();
    if args.generate_schema {
        let schema = Schema::build(Query, Mutation, EmptySubscription).finish();
        let mut file = File::create("./schemas/media.graphql")?;
        let sdl_export_options = SDLExportOptions::new().federation();
        let schema_sdl = schema.sdl_with_options(sdl_export_options);
        file.write_all(schema_sdl.as_bytes())?;
        info!("GraphQL schema: ./schemas/media.graphql was successfully generated!");
    } else {
        start_service(args.rewrite_domain).await;
    }
    Ok(())
}

/// Describes the handler for GraphQL requests.
///
/// Executes the GraphQL schema with the request.
async fn graphql_handler(
    State(schema): State<Schema<Query, Mutation, EmptySubscription>>,
    req: GraphQLRequest,
) -> GraphQLResponse {
    let req = req.into_inner();
    schema.execute(req).await.into()
}

/// Starts media service on port 8000.
async fn start_service(rewrite_domain: Url) {
    let media_data_bucket = initialize_minio_media_data_bucket().await;

    let schema = Schema::build(Query, Mutation, EmptySubscription)
        .extension(Logger)
        .data(media_data_bucket)
        .data(rewrite_domain)
        .enable_federation()
        .finish();

    let app = Router::new()
        .route("/", get(graphiql).post(graphql_handler))
        .route("/health", get(StatusCode::OK))
        .with_state(schema);

    info!("GraphiQL IDE: http://0.0.0.0:8080");
    Server::bind(&"0.0.0.0:8080".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
