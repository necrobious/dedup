use lambda_http::{run, service_fn, tracing, Body, Error, Request, RequestExt, Response};
use regex::Regex;
use std::time::{SystemTime, UNIX_EPOCH};
use aws_config;
use aws_sdk_dynamodb::{
    types::ReturnValue,
    Client,
};
use serde::{ Serialize, Deserialize };

#[derive(Serialize, Deserialize)]
struct Record {
    cnt: u64,
    fst: u64,
    lst: u64,
}

async fn json_response(response: &Record) -> Result<Response<Body>, Error> {
    let resp = match serde_json::to_string(&response) {
        Ok(json_resp) => {
            Response::builder()
                .status(200)
                .header("content-type", "application/json")
                .body(json_resp.into())
                .map_err(Box::new)?
        },
        _ => {
            Response::builder()
                .status(500)
                .header("content-type", "text/plain")
                .body(format!("Internal Server Error: response serialization failed;").into())
                .map_err(Box::new)?
        }
    };

    Ok(resp)
}

async fn get_handler(client: &Client, key:&str) -> Result<Response<Body>, Error> {
    let key_av = serde_dynamo::to_attribute_value(key)?;

    let response = client
        .get_item()
        .table_name("Dedup")
        .key("pk", key_av)
        .projection_expression("cnt,fst,lst")
        .send()
        .await;

    if response.is_err() {
        // TODO: get the error and report it
        return Ok(Response::builder()
            .status(500)
            .header("content-type", "text/plain")
            .body(format!("Internal Server Error: {:?}; key={};", response, key).into())
            .map_err(Box::new)?)
    }

    let Some(item) = response.unwrap().item else {
        return Ok(Response::builder()
            .status(404)
            .header("content-type", "text/plain")
            .body(format!("Key not found: {}", key).into())
            .map_err(Box::new)?)
    };

    let Ok(rec) = serde_dynamo::from_item(item) else {
         return Ok(Response::builder()
            .status(500)
            .header("content-type", "text/plain")
            .body(format!("Internal Server Error: deserialization failure").into())
            .map_err(Box::new)?)
    };

    json_response(&rec).await
}

async fn put_handler(client: &Client, key:&str) -> Result<Response<Body>, Error> {
    let Ok(now) = SystemTime::now().duration_since(UNIX_EPOCH).map(|dur|dur.as_secs()) else {
        return Ok(Response::builder()
            .status(500)
            .header("content-type", "text/plain")
            .body(format!("Internal Server Error: system time is unavailable").into())
            .map_err(Box::new)?)
    };

    let exp = now + 365 * 24 * 60 * 60;

    let key_av = serde_dynamo::to_attribute_value(key)?;
    let cnt_av = serde_dynamo::to_attribute_value(1)?;
    let now_av = serde_dynamo::to_attribute_value(now)?;
    let exp_av = serde_dynamo::to_attribute_value(exp)?;

    let response = client
        .update_item()
        .table_name("Dedup")
        .key("pk", key_av)
        .update_expression("ADD cnt :cnt SET lst = :now, fst = if_not_exists(fst, :now), exp = if_not_exists(exp, :exp)")
        .expression_attribute_values(":cnt", cnt_av)
        .expression_attribute_values(":now", now_av)
        .expression_attribute_values(":exp", exp_av)
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

    let Some(attributes) = response.unwrap().attributes else {
        return Ok(Response::builder()
            .status(500)
            .header("content-type", "text/plain")
            .body(format!("Internal Server Error: missing data after update;").into())
            .map_err(Box::new)?)
    };

    let Ok(rec) = serde_dynamo::from_item(attributes) else {
         return Ok(Response::builder()
            .status(500)
            .header("content-type", "text/plain")
            .body(format!("Internal Server Error: deserialization failure").into())
            .map_err(Box::new)?)
    };

    json_response(&rec).await
}

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

    let m = event.method();
    if m == "PUT" {
        put_handler(client, key).await
    } else if m == "GET" {
        get_handler(client, key).await
    } else {
        return Ok(Response::builder()
            .status(405)
            .header("content-type", "text/plain")
            .body(format!("Method Not Allowed").into())
            .map_err(Box::new)?)
    }
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
