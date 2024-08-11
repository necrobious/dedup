use lambda_http::{run, service_fn, tracing, Body, Error, Request, RequestExt, Response};
use regex::Regex;
use std::time::{SystemTime, UNIX_EPOCH};
use std::collections::hash_map::HashMap;
use std::str::FromStr;
use aws_config;
use aws_sdk_dynamodb::{
    types::{AttributeValue, ReturnValue},
    Client,
};
use serde_json::{ Value };

fn num_value (attributes: &HashMap<String, AttributeValue>, key: &str) -> Option<Value> {
    attributes
        .get(key)
        .filter(|av| av.is_n())
        .and_then(|av| av.as_n().ok())
        .and_then(|s| serde_json::value::Number::from_str(s.as_str()).ok())
        .map(|n| Value::Number(n))
}

fn into_json (attributes: &HashMap<String, AttributeValue>) -> Value {
    let mut map = serde_json::map::Map::new();

    let cnt = num_value(&attributes, "cnt");
    if cnt.is_some() {
        map.insert("cnt".to_string(), cnt.unwrap());
    }

    let lst = num_value(&attributes, "lst");
    if lst.is_some() {
        map.insert("lst".to_string(), lst.unwrap());
    }

    let fst = num_value(&attributes, "fst");
    if fst.is_some() {
        map.insert("fst".to_string(), fst.unwrap());
    }

    serde_json::Value::Object(map)
}

async fn json_response(response: &Value) -> Result<Response<Body>, Error> {
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
    let response = client
        .get_item()
        .table_name("Dedup")
        .key("pk", AttributeValue::S(key.into()))
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

    let obj = into_json(&item);
    json_response(&obj).await
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

    let Some(attributes) = response.unwrap().attributes else {
        return Ok(Response::builder()
            .status(500)
            .header("content-type", "text/plain")
            .body(format!("Internal Server Error: missing data after update;").into())
            .map_err(Box::new)?)
    };

    let obj = into_json(&attributes);
    json_response(&obj).await
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
