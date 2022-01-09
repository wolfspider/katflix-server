#[macro_use]
extern crate lazy_static;

mod models;
use futures_util::{Stream, StreamExt};
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
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
    futures::executor::block_on(models::fdb_model::run_query(&db, 10, 10));

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
     warp::path("fdb")
    .and(warp::post())
    .and(dbinstance.clone())
    .map(|dbinstance| { 
        get_posts_render(dbinstance);
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
            .body(INDEX_HTML)
    });

    let routes = index.or(chat_recv).or(vid).or(get_posts);

    warp::serve(routes).run(([0, 0, 0, 0], 8000)).await;

    // shutdown the client
    drop(_guard);
}

fn get_posts_render(dbinstance: Arc<fdb::Database>)  {
    futures::executor::block_on(models::fdb_model::run_query(&dbinstance, 10, 10));
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

static INDEX_HTML: &str = r#"
<!DOCTYPE html>
<html>
    <head>
        <title>Warp Chat</title>
    </head>
    <body>
        <h1>warp chat</h1>
        <div id="chat">
            <p><em>Connecting...</em></p>
        </div>
        <input type="text" id="text" />
        <button type="button" id="send">Send</button>
        <script type="text/javascript">
        var uri = 'http://' + location.host + '/chat';
        var sse = new EventSource(uri);
        function message(data) {
            var line = document.createElement('p');
            line.innerText = data;
            chat.appendChild(line);
        }
        sse.onopen = function() {
            chat.innerHTML = "<p><em>Connected!</em></p>";
        }
        var user_id;
        sse.addEventListener("user", function(msg) {
            user_id = msg.data;
        });
        sse.onmessage = function(msg) {
            message(msg.data);
        };
        send.onclick = function() {
            var msg = text.value;
            var xhr = new XMLHttpRequest();
            xhr.open("POST", uri + '/' + user_id, true);
            xhr.send(msg);
            text.value = '';
            message('<You>: ' + msg);
        };
        </script>
    </body>
</html>
"#;
