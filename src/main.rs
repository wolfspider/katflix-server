#[macro_use]
extern crate lazy_static;

mod models;
use futures_util::{Stream, StreamExt};
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
use std::convert::TryFrom;
use std::thread;
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use console::Style;
use warp::{sse::Event, Filter};
use foundationdb as fdb;

/// global unique user id counter
static NEXT_USER_ID: AtomicUsize = AtomicUsize::new(1);

/// Our state of currently connected users.
///
/// - Key is their id
/// - Value is a sender of `Message`
type Users = Arc<Mutex<HashMap<usize, mpsc::UnboundedSender<Message>>>>;

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
    
    println!("\nRust Warp Server ready at {}", blue.apply_to(&target));
    
    let vid = warp::path("videos").and(warp::fs::dir("./videos/"));

    let userhash: HashMap<usize, tokio::sync::mpsc::UnboundedSender<Message>> = HashMap::new();
    
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
     warp::path("chat")
    .and(warp::post())
    .and(warp::path::param::<usize>())
    .and(warp::body::content_length_limit(500))
    .and(
        warp::body::bytes().and_then(|body: bytes::Bytes| async move {
            std::str::from_utf8(&body)
                .map(String::from)
                .map_err(|_e| warp::reject::custom(NotUtf8))
            }),
        )
    .and(users.clone())
    .and(dbinstance.clone())
    .map(|my_id, msg, users, dbinstance| { 
        get_posts_render(my_id, msg, &users, dbinstance);
    warp::reply()});

    let posts_status = 
     warp::path("status")
    .and(warp::post())
    .and(warp::path::param::<usize>())
    .and(warp::body::content_length_limit(500))
    .and(
        warp::body::bytes().and_then(|body: bytes::Bytes| async move {
            std::str::from_utf8(&body)
                .map(String::from)
                .map_err(|_e| warp::reject::custom(NotUtf8))
            }),
        )
    .and(users.clone())
    .and(dbinstance.clone())
    .map(|my_id, msg, users, dbinstance| { 
        get_posts_status(my_id, msg, &users, dbinstance);
    warp::reply()});

    let post_delete = 
     warp::path("delete")
    .and(warp::post())
    .and(warp::path::param::<usize>())
    .and(warp::body::content_length_limit(500))
    .and(
        warp::body::bytes().and_then(|body: bytes::Bytes| async move {
            std::str::from_utf8(&body)
                .map(String::from)
                .map_err(|_e| warp::reject::custom(NotUtf8))
            }),
        )
    .and(users.clone())
    .and(dbinstance.clone())
    .map(|my_id, msg, users, dbinstance| { 
        delete_post(my_id, msg, &users, dbinstance);
    warp::reply()});

    let post_commit = 
     warp::path("commit")
    .and(warp::post())
    .and(warp::path::param::<usize>())
    .and(warp::body::content_length_limit(500))
    .and(
        warp::body::bytes().and_then(|body: bytes::Bytes| async move {
            std::str::from_utf8(&body)
                .map(String::from)
                .map_err(|_e| warp::reject::custom(NotUtf8))
            }),
        )
    .and(users.clone())
    .and(dbinstance.clone())
    .map(|my_id, msg, users, dbinstance| { 
        get_posts_commit(my_id, msg, &users, dbinstance);
    warp::reply()});

    // GET /chat -> messages stream
    let chat_recv = warp::path("chat").and(warp::get()).and(users).map(|users| {
        // reply using server-sent events
        let stream = user_connected(users);
        warp::sse::reply(warp::sse::keep_alive().stream(stream))
    });

    // GET / -> index html
    let index = warp::path::end().map(|| {
        warp::http::Response::builder()
            .header("content-type", "text/html; charset=utf-8")
            .body(models::fdb_model::INDEX_HTML)
    });

    let routes = index
    .or(chat_recv)
    .or(vid)
    .or(get_posts)
    .or(posts_status)
    .or(post_delete)
    .or(post_commit);

    warp::serve(routes).run(([0, 0, 0, 0], 8000)).await;

    // shutdown the client
    drop(_guard);
}

fn get_posts_render(my_id: usize, msg: String, users: &Users, dbinstance: Arc<fdb::Database>)  {
    let mut new_msg = format!("User#{}: {},", my_id, msg);
    
    //param example
    let vecstr = futures::executor::block_on(models::fdb_model::run_query_posts(&dbinstance, msg));

    for fdbstr in vecstr {
        let compstr = format!("{} ,", &fdbstr);
        new_msg.push_str(&compstr);
    }
    
    // New message from this user, send it to everyone else (except same uid)...
    //
    // We use `retain` instead of a for loop so that we can reap any user that
    // appears to have disconnected.
    users.lock().unwrap().retain(|uid, tx| {
        /*
        if my_id == *uid {
            // don't send to same user, but do retain
            true
        } else {*/
            // If not `is_ok`, the SSE stream is gone, and so don't retain
            tx.send(Message::Reply(new_msg.clone())).is_ok()
        //}
    });
}

fn get_posts_status(my_id: usize, msg: String, users: &Users, dbinstance: Arc<fdb::Database>)  {
    let mut new_msg = format!("User#{}: {},", my_id, msg);
    
    
    let vecstr = futures::executor::block_on(models::fdb_model::render_posts(&dbinstance));
    
    for fdbstr in vecstr {
        let compstr = format!("{} ,", &fdbstr);
        new_msg.push_str(&compstr);
    }

    // New message from this user, send it to everyone else (except same uid)...
    //
    // We use `retain` instead of a for loop so that we can reap any user that
    // appears to have disconnected.
    users.lock().unwrap().retain(|uid, tx| {
        /*
        if my_id == *uid {
            // don't send to same user, but do retain
            true
        } else {*/
            // If not `is_ok`, the SSE stream is gone, and so don't retain
            tx.send(Message::Reply(new_msg.clone())).is_ok()
        //}
    });
}

fn delete_post(my_id: usize, msg: String, users: &Users, dbinstance: Arc<fdb::Database>)  {
    let mut new_msg = format!("User::User#{}: {},", my_id, msg);
    
    println!("Id deleted is: {}",msg);

    let delid = msg.parse::<i32>().unwrap();

    let delidus = usize::try_from(delid).unwrap();
    
    let vecstr = futures::executor::block_on(models::fdb_model::delete_post_query(&dbinstance, delidus));
    
    for fdbstr in vecstr {
        let compstr = format!("{} ,", &fdbstr);
        new_msg.push_str(&compstr);
    }

    // New message from this user, send it to everyone else (except same uid)...
    //
    // We use `retain` instead of a for loop so that we can reap any user that
    // appears to have disconnected.
    users.lock().unwrap().retain(|uid, tx| {
        /*
        if my_id == *uid {
            // don't send to same user, but do retain
            true
        } else {*/
            // If not `is_ok`, the SSE stream is gone, and so don't retain
            tx.send(Message::Reply(new_msg.clone())).is_ok()
        //}
    });
}

fn get_posts_commit(my_id: usize, msg: String, users: &Users, dbinstance: Arc<fdb::Database>)  {
    let mut new_msg = format!("User::User#{}: {},", my_id, msg);
    
    let vecstr = futures::executor::block_on(models::fdb_model::commit_posts_query(&dbinstance));
    
    for fdbstr in vecstr {
        let compstr = format!("{} ,", &fdbstr);
        new_msg.push_str(&compstr);
    }

    // New message from this user, send it to everyone else (except same uid)...
    //
    // We use `retain` instead of a for loop so that we can reap any user that
    // appears to have disconnected.
    users.lock().unwrap().retain(|uid, tx| {
        /*
        if my_id == *uid {
            // don't send to same user, but do retain
            true
        } else {*/
            // If not `is_ok`, the SSE stream is gone, and so don't retain
            tx.send(Message::Reply(new_msg.clone())).is_ok()
        //}
    });
}


fn user_connected(users: Users) -> impl Stream<Item = Result<Event, warp::Error>> + Send + 'static {
    // Use a counter to assign a new unique ID for this user.
    let my_id = NEXT_USER_ID.fetch_add(1, Ordering::Relaxed);

    eprintln!("new chat user: {}", my_id);

    // Use an unbounded channel to handle buffering and flushing of messages
    // to the event source...
    let (tx, rx) = mpsc::unbounded_channel();
    let rx = UnboundedReceiverStream::new(rx);

    tx.send(Message::UserId(my_id))
        // rx is right above, so this cannot fail
        .unwrap();

    // Save the sender in our list of connected users.
    users.lock().unwrap().insert(my_id, tx);

    // Convert messages into Server-Sent Events and return resulting stream.
    rx.map(|msg| match msg {
        Message::UserId(my_id) => Ok(Event::default().event("user").data(my_id.to_string())),
        Message::Reply(reply) => Ok(Event::default().data(reply)),
    })
}


