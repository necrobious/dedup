use lambda_http::{run, service_fn, tracing, Body, Error, Request, RequestExt, Response};
use regex::Regex;
use aws_config;
use aws_sdk_dynamodb::{
    operation::{
        batch_write_item::BatchWriteItemError, delete_item::DeleteItemError,
        put_item::PutItemError, query::QueryError, scan::ScanError,
    },
    primitives::Blob,
    types::{AttributeValue, DeleteRequest, WriteRequest},
    Client,
};

/// This is the main body for the function.
/// Write your code inside it.
/// There are some code example in the following URLs:
/// - https://github.com/awslabs/aws-lambda-rust-runtime/tree/main/examples
async fn function_handler(client: &Client, event: Request) -> Result<Response<Body>, Error> {
    let re = Regex::new(r"^/(?<uuid>[A-Za-z0-9]{8}-[A-Za-z0-9]{4}-4[A-Za-z0-9]{3}-[AaBb98][A-Za-z0-9]{3}-[A-Za-z0-9]{12})$").unwrap();
    let Some(caps) = re.captures(event.raw_http_path()) else {
        return Ok(Response::builder()
        .status(400)
        .header("content-type", "text/plain")
        .body(format!("Bad Request").into())
        .map_err(Box::new)?)
    };
    let key = &caps["uuid"];

    let message = format!("Key {key}.");

    // Extract some useful information from the request
//    let who = event
//        .query_string_parameters_ref()
//        .and_then(|params| params.first("name"))
//        .unwrap_or("world");


    // Return something that implements IntoResponse.
    // It will be serialized to the right response event automatically by the runtime
    let resp = Response::builder()
        .status(200)
        .header("content-type", "text/html")
        .body(message.into())
        .map_err(Box::new)?;
    Ok(resp)
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing::init_default_subscriber();

    let config = aws_config::load_defaults(aws_config::BehaviorVersion::v2023_11_09()).await;
    let client = Client::new(&config);

    run(service_fn(async |event: Request| {
        function_handler(&client, event).await
    })).await
}
