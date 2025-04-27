use std::{env, fs::File, io::Write};

use async_graphql::{
    extensions::Logger, http::GraphiQLSource, EmptySubscription, SDLExportOptions, Schema,
};
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};

use authorization::AuthorizedUserHeader;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{self, IntoResponse},
    routing::get,
    Router,
};
use clap::{arg, command, Parser};
use s3::{creds::Credentials, Bucket, BucketConfiguration, Region};

use log::{info, Level};

use once_cell::sync::Lazy;
use axum_otel_metrics::HttpMetricsLayerBuilder;
use axum_otel_metrics::HttpMetricsLayer;

use opentelemetry::global;
use opentelemetry_sdk::metrics::{PeriodicReader, SdkMeterProvider, Temporality};
use opentelemetry_sdk::Resource;
use opentelemetry_otlp::WithExportConfig;

mod authorization;
mod event;
mod graphql;
use crate::graphql::{mutation::Mutation, query::Query};

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
}

/// Connects to MinIO bucket if existent, otherwise creates bucket `media-data`.
/// Warning: Change credentials before production use.
async fn initialize_minio_media_data_bucket() -> Bucket {
    let bucket_name = "media-data";
    let endpoint = env::var("MINIO_ENDPOINT").unwrap();
    let region = Region::Custom {
        region: "eu-central-1".to_string(),
        endpoint,
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
    simple_logger::init_with_level(Level::Warn).unwrap();

    let args = Args::parse();
    if args.generate_schema {
        let schema = Schema::build(Query, Mutation, EmptySubscription).finish();
        let mut file = File::create("./schemas/media.graphql")?;
        let sdl_export_options = SDLExportOptions::new().federation();
        let schema_sdl = schema.sdl_with_options(sdl_export_options);
        file.write_all(schema_sdl.as_bytes())?;
        info!("GraphQL schema: ./schemas/media.graphql was successfully generated!");
    } else {
        start_service().await;
    }
    Ok(())
}

/// Describes the handler for GraphQL requests.
///
/// Parses the `Authorized-User` header and writes it in the context data of the specfic request.
/// Then executes the GraphQL schema with the request.
///
/// * `schema` - GraphQL schema used by handler.
/// * `headers` - Header map containing headers of request.
/// * `request` - GraphQL request.
async fn graphql_handler(
    State(schema): State<Schema<Query, Mutation, EmptySubscription>>,
    headers: HeaderMap,
    req: GraphQLRequest,
) -> GraphQLResponse {
    let mut req = req.into_inner();
    if let Ok(authenticate_user_header) = AuthorizedUserHeader::try_from(&headers) {
        req = req.data(authenticate_user_header);
    }
    schema.execute(req).await.into()
}

static RESOURCE: Lazy<Resource> = Lazy::new(|| {
    Resource::builder()
        .with_service_name("media")
        .build()
});

/// Initializes OpenTelemetry metrics exporter and sets the global meter provider.
fn init_otlp() -> HttpMetricsLayer {
    let exporter = opentelemetry_otlp::MetricExporter::builder()
        .with_http()
        .with_endpoint("http://otel-collector:4318/v1/metrics")
        .with_temporality(Temporality::default())
        .build()
        .unwrap();

    let reader = PeriodicReader::builder(exporter)
        .with_interval(std::time::Duration::from_secs(5))
        .build();

    let provider = SdkMeterProvider::builder()
        .with_reader(reader)
        .with_resource(RESOURCE.clone())
        .build();

    global::set_meter_provider(provider.clone());

    HttpMetricsLayerBuilder::new()
        .with_provider(provider.clone())
        .build()
}

/// Starts media service on port 8000.
async fn start_service() {
    let media_data_bucket = initialize_minio_media_data_bucket().await;

    let schema = Schema::build(Query, Mutation, EmptySubscription)
        .extension(Logger)
        .data(media_data_bucket)
        .enable_federation()
        .finish();

    let metrics = init_otlp();

    let app = Router::new()
        .route("/", get(graphiql).post(graphql_handler))
        .route("/health", get(StatusCode::OK))
        .with_state(schema)
        .layer(metrics);

    info!("GraphiQL IDE: http://0.0.0.0:8080");

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    axum::serve(listener, app)
        .await
        .unwrap();
}
