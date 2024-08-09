use lambda_http::{run, service_fn, tracing, Body, Error, Request, RequestExt, Response};
use regex::Regex;
use std::time::{SystemTime, Duration, UNIX_EPOCH};
use aws_config;
use aws_sdk_dynamodb::{
    operation::{
        batch_write_item::BatchWriteItemError, delete_item::DeleteItemError,
        put_item::PutItemError, query::QueryError, scan::ScanError,
    },
    primitives::Blob,
    types::{AttributeValue, ReturnValue},
    Client,
};

async fn handler(client: &Client, event: Request) -> Result<Response<Body>, Error> {
    let re = Regex::new(r"^/(?<uuid>[A-Za-z0-9]{8}-[A-Za-z0-9]{4}-4[A-Za-z0-9]{3}-[AaBb98][A-Za-z0-9]{3}-[A-Za-z0-9]{12})$").unwrap();

    let Some(caps) = re.captures(event.raw_http_path()) else {
        return Ok(Response::builder()
            .status(400)
            .header("content-type", "text/plain")
            .body(format!("Bad Request").into())
            .map_err(Box::new)?)
    };
    let key = &caps["uuid"];

    // let message = format!("Key {key}.");

    let Ok(now) = SystemTime::now().duration_since(UNIX_EPOCH).map(|dur|dur.as_secs()) else {
        return Ok(Response::builder()
            .status(500)
            .header("content-type", "text/plain")
            .body(format!("Internal Server Error: system time is unavailable").into())
            .map_err(Box::new)?)
    };

    let exp = now + 365 * 24 * 60 * 60;

    let response = client
        .update_item()
        .table_name("Dedup")
        .key("pk", AttributeValue::S(key.into()))
        .update_expression("ADD cnt :cnt SET lst = :now, fst = if_not_exists(fst, :now), exp = if_not_exists(exp, :exp)")
        .expression_attribute_values(":cnt", AttributeValue::N(1.to_string()))
        .expression_attribute_values(":now", AttributeValue::N(now.to_string()))
        .expression_attribute_values(":exp", AttributeValue::N(exp.to_string()))
        .return_values(ReturnValue::AllNew)
        .send()
        .await;

    if response.is_err() {
        // TODO: get the error and report it
        return Ok(Response::builder()
            .status(500)
            .header("content-type", "text/plain")
            .body(format!("Internal Server Error: {:?}; key={}; exp={}; now={};", response, key, exp, now).into())
            .map_err(Box::new)?)

    }

    let result = response.unwrap().attributes.unwrap(); // TODO: second unwrap is not safe
//    let json = result.into_iter().map(|(k,v)| (k, if v.is_n {} else {} ) ).collect::<Value>(); 
    

    let message = format!("key={}; exp={}; now={}; result={:?}", key, exp, now, result);
    // Extract some useful information from the request
//    let who = event
//        .query_string_parameters_ref()
//        .and_then(|params| params.first("name"))
//        .unwrap_or("world");


    // Return something that implements IntoResponse.
    // It will be serialized to the right response event automatically by the runtime
    let resp = Response::builder()
        .status(200)
        .header("content-type", "text/plain")
        .body(message.into())
        .map_err(Box::new)?;
    Ok(resp)
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing::init_default_subscriber();

    let config = aws_config::load_defaults(aws_config::BehaviorVersion::v2024_03_28()).await;
    let client = Client::new(&config);

    run(service_fn( |event: Request| async {
        handler(&client, event).await
    })).await
}
