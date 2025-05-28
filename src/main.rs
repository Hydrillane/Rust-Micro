#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate diesel;

mod schema;
mod models;

use std::error::Error;
use maud::html;
use models::Message;
use diesel::prelude::*;
use diesel::pg::PgConnection;
use hyper::error::Error as hyperError;

use std::net::{IpAddr,SocketAddr,Ipv4Addr};
extern crate hyper;
extern crate form_urlencoded;
extern crate futures;
extern crate multipart;


extern crate maud;

use std::collections::HashMap;

use std::{env, io};

#[macro_use]
extern crate log;
extern crate env_logger;

use hyper::header::{ContentLength, ContentType};
use hyper::{server::{Request,Response,Service}, Chunk, StatusCode,Method::{Get,Post}};

use futures::{future::{Future, FutureResult}, Stream};



struct Microservices;

struct NewMessage {
    username: String,
    message: String,
}

struct TimeRange {
    before: Option<i64>,
    after: Option<i64>,
}

fn parse_form(form_chunk:Chunk) -> FutureResult<NewMessage,hyperError> {
    let mut form = form_urlencoded::parse(&form_chunk)
        .into_owned()
        .collect::<HashMap<String,String>>();

    if let Some(message) = form.remove("message") {
        let username = form.remove("username").unwrap_or(String::from("anonymous"));
        futures::future::ok(NewMessage {
            username,
            message
        })
    } else {
        futures::future::err(hyperError::from(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Missing field 'message",
        )))
    }

    }


#[macro_use]
extern crate serde_json;

fn make_post_response(
    result: Result<i64,hyper::Error>,
) -> FutureResult<Response, hyper::Error> {
    match result {
        Ok(timestamp) => {
            let payload = json!({"timestamp": timestamp}).to_string();
            let response = Response::new()
                .with_header(ContentLength(payload.len() as u64))
                .with_header(ContentType::json())
                .with_body(payload);
            debug!("{:?}",response);
            futures::future::ok(response)
        }
        Err(error) => make_error_response(error.to_string()),
    }
}

fn make_error_response(error_message:String) -> FutureResult<hyper::Response,hyper::Error> {
    let payload = json!({"error": error_message});
    let payload_str = payload.to_string();
    let response = Response::new()
        .with_status(StatusCode::InternalServerError)
        .with_header(ContentLength(payload_str.len() as u64))
        .with_header(ContentType::json())
        // come back later this might cause some error in the future
        .with_body(payload_str);
    debug!("{:?}",response);
    futures::future::ok(response)
}

const DEFAULT_DATABASE_URL: &'static str = "postgres://mydb@localhost/diesel_demo";

fn connect_to_db() -> Option<PgConnection>  {
    let database_url = env::var("DATABASE_URL").unwrap_or(String::From("DEFAULT_DATABASE_URL"));
    match PgConnection::establish(&database_url) {
        Ok(connection) => Some(connection),
        Err(error) => {
            error!("Error connecting to database: {}", error.description());
                None
        }
    }
}

impl Service for Microservices {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Future = Box<dyn Future<Item = Self::Response, Error = Self::Error>>;


    fn call(&self,request:Request) -> Self::Future {
        let db_connection = match connect_to_db(){
            Some(connection) => connection,
            None => {
                return Box::new(futures::future::ok(Response::new().with_status(StatusCode::InternalServerError)))
            },
        };
        match (request.method(),request.path()) {
            (&Post, "/") => {
                let future = request
                    .body()
                    .concat2()
                    .and_then(parse_form)
                    .and_then(move |new_message| write_to_db(new_message, &db_connection))
                    .then(make_post_response);
                Box::new(future)
            }
            (&Get,"/") => {
                let time_range = match request.query(){
                    Some(query) => parse_query(query),
                    None => Ok(TimeRange {
                        before:None,
                        after: None,
                    }),
                };
                let response = match time_range {
                    Ok(time_range) => make_get_response(query_db(time_range)),
                    Err(error) => make_error_response(error.to_string()),
                };
                Box::new(response)
            }
        }
    }
}



fn query_db(time_range:TimeRange, db_connection:&PgConnection) -> Option<Vec<Message>> {
    use schema::messages;
    let TimeRange{before,after} = time_range;
    let query_result = match(before,after) {
        (Some(before),Some(after)) => {
            messages::table
                .filter(messages::timestamp.lt(before as i64))
                .filter(messages::timestamp.gt(after as i64))
                .load::<Message>(db_connection)
        }
        (Some(before),_) => {
            messages::table
                .filter(messages::timestamp.lt(before as i64))
                .load::<Message>(db_connection)
        }
        (_,Some(after)) => {
            messages::table
                .filter(messages::timestamp(after as i64))
                .load::<Message>(db_connection)
        }
        _ => {
            messages::table.load::<Message>(db_connection)
        }
    };
    match query_result {
        Ok(res) => Some(res),
        Err(err) => {
            error!("Failed querying database!: {}",err);
            None
        }
    }
}




fn parse_query(query:&str) -> Result<TimeRange,String> {
    let args = form_urlencoded::parse(&query.as_bytes())
        .into_owned()
        .collect::<HashMap<String,String>>();

    let before = args.get("before").map(|value| value.parse::<i64>());
    if let Some(ref result) = before {
        if let Err(ref error) = *result {
            return Err(format!("Error parsing 'before': {}",error));
        }
    }
    let after = args.get("after").map(|value| value.parse::<i64>());
    if let Some(ref result) = after {
        if let Err(ref err) = *result {
            return Err(format!("Error parsing 'after': {}",err));
        }
    }
    Ok(TimeRange {
        before:before.map(|b| b.unwrap()),
        after:after.map(|a| a.unwrap()),
    })

}

fn make_get_response(messages: Option<Vec<Message>>) -> FutureResult<hyper::Response, hyper::Error> {
    let response = match messages { 
        Some(messages) => {
            let body = render_page(messages);
            Response::new()
                .with_header(ContentLength(body.len() as u64))
                .with_body(body)
        }
        None => {
            Response::new().with_status(StatusCode::InternalServerError);
        },
    };
    debug!("{:?}", response);
    futures::future::ok(response)

}


fn main(){
    env_logger::init();
    let address : SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),8080);
    let server = hyper::server::Http::new()
        .bind(&address, || Ok(Microservices{}))
        .unwrap();
    info!("Running microservice at {}",address);
    server.run().unwrap();
}


fn write_to_db(
    new_message: NewMessage,
    db_connection: &PgConnection,
) -> FutureResult<i64,hyper::Error> {
    use schema::messages;
    let timestamp = diesel::insert_into(messages::table)
        .values(&new_message)
        .returning(messages::timestamp)
        .get_result(db_connection);

    match timestamp {
        Ok(timestamp) => futures::future::ok(timestamp),
        Err(error) => {
            error!("Error writing to database: {}",error.to_string());
            futures::future::err(hyper::error::Error::from{
                io:Error::new(io::ErrorKind::Other,"service error"),
            })
        }
    }
}




fn render_page(messages: Vec<Message>) -> String {
    (html! {
        head {
            title "microservice"
            style "body {font-family:monospace}"
        } // todo like what the fuck is this getting me so confused
        body {
            ul {
                @for  message in & messages {
                    li {
                        (message.username) " (" (message.timestamp) "): " (message.message)
                    }

                }
            }
        }
}).into_string()

}
