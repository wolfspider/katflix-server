#[macro_use]
extern crate lazy_static;

mod models;


use std::convert::Infallible;
use std::str::FromStr;
use std::time::Duration;
use std::sync::Arc;
use console::Style;
use warp::Filter;
use foundationdb as fdb;

#[tokio::main]
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
    println!("\nAsync Function loaded for <= 5 seconds goto http://localhost:8000/5");

    let vid = warp::path("videos").and(warp::fs::dir("./videos/"));

    let dbinstance = Arc::new(db);
    //let routes = end.or(vids).or(post_api)
    let dbinstance = warp::any().map(move || dbinstance.clone());

    let get_posts = 
     warp::path("fdb")
    .and(warp::post())
    .and(dbinstance.clone())
    .map(|dbinstance| { 
        get_posts_render(dbinstance);
    warp::reply()});

    let routes = warp::path::param()
    .and_then(sleepy).or(vid).or(get_posts);

    warp::serve(routes).run(([0, 0, 0, 0], 8000)).await;

    // shutdown the client
    drop(_guard);
}


async fn sleepy(Seconds(seconds): Seconds) -> Result<impl warp::Reply, Infallible> {
    tokio::time::delay_for(Duration::from_secs(seconds)).await;
    Ok(format!("I waited {} seconds!", seconds))
}

fn get_posts_render(dbinstance: Arc<fdb::Database>) {
    futures::executor::block_on(models::fdb_model::run_query(&dbinstance, 10, 10));
}

/// A newtype to enforce our maximum allowed seconds.
struct Seconds(u64);

impl FromStr for Seconds {
    type Err = ();
    fn from_str(src: &str) -> Result<Self, Self::Err> {
        src.parse::<u64>().map_err(|_| ()).and_then(|num| {
            if num <= 5 {
                Ok(Seconds(num))
            } else {
                Err(())
            }
        })
    }
}