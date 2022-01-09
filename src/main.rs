#[macro_use]
extern crate lazy_static;

mod models;
use futures_util::{Stream, StreamExt};
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};

use tokio_stream::wrappers::UnboundedReceiverStream;
use console::Style;
use warp::Filter;
use foundationdb as fdb;

/// global unique user id counter
static NEXT_USER_ID: AtomicUsize = AtomicUsize::new(1);

/// Message variants
#[derive(Debug)]
enum Message {
    UserId(usize),
    Reply(String),
}

#[derive(Debug)]
struct NotUtf8;
impl warp::reject::Reject for NotUtf8 {}


#[tokio::main(flavor = "current_thread")]
async fn main() {
    
    let target: String = "0.0.0.0:8000".parse().unwrap();
    let blue = Style::new()
        .blue();

    let _guard = unsafe { fdb::boot() };
    
    let db = futures::executor::block_on(fdb::Database::new_compat(None))
        .expect("failed to get database");
    futures::executor::block_on(models::fdb_model::init(&db, &*models::fdb_model::ALL_POSTS));
    println!("Initialized");
    futures::executor::block_on(models::fdb_model::run_query(&db, 10, 10));

    println!("\nRust Warp Server ready at {}", blue.apply_to(&target));
    
    let vid = warp::path("videos").and(warp::fs::dir("./videos/"));

    let userhash: HashMap<usize, String> = HashMap::new();
    
    // Turn our "state" into a new Filter...
    let dbinstance = Arc::new(db);

    // Keep track of all connected users, key is usize, value
    // is an event stream sender.
    let users = Arc::new(Mutex::new(userhash));

    // Turn our "state" into a new Filter...
    let users = warp::any().map(move || users.clone());

    //let routes = end.or(vids).or(post_api)
    let dbinstance = warp::any().map(move || dbinstance.clone());

    let get_posts = 
     warp::path("fdb")
    .and(warp::post())
    .and(dbinstance.clone())
    .map(|dbinstance| { 
        get_posts_render(dbinstance);
    warp::reply()});

    let routes = vid.or(get_posts);

    warp::serve(routes).run(([0, 0, 0, 0], 8000)).await;

    // shutdown the client
    drop(_guard);
}

fn get_posts_render(dbinstance: Arc<fdb::Database>)  {
    futures::executor::block_on(models::fdb_model::run_query(&dbinstance, 10, 10));
}
