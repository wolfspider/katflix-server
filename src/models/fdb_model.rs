
use std::borrow::Cow;

use std::thread;
use std::str;

use futures::prelude::*;
use rand::{rngs::ThreadRng, seq::SliceRandom};

use foundationdb::tuple::{pack, unpack, Subspace};
use foundationdb::{Database, FdbError, RangeOption, TransactError, TransactOption, Transaction, KeySelector};

type Result<T> = std::result::Result<T, Error>;
enum Error {
    FdbError(FdbError),
}

impl From<FdbError> for Error {
    fn from(err: FdbError) -> Self {
        Error::FdbError(err)
    }
}

impl TransactError for Error {
    fn try_into_fdb_error(self) -> std::result::Result<FdbError, Self> {
        match self {
            Error::FdbError(err) => Ok(err),
        }
    }
}

const POOLSZ: usize = 10;

const WORKSZ: usize = 1;

const POSTS: &[&str] = &[
    r#"post-001-{"title"_"introduction""#,
    r#"post-002-{"title"_"stickies""#,
    r#"post-003-{"title"_"howtos""#,
    r#"post-004-{"title"_"beginner""#,
    r#"post-005-{"title"_"intermediate""#,
    r#"post-006-{"title"_"advanced""#,
    r#"post-007-{"title"_"FAQS""#,
    r#"post-008-{"title"_"contacts""#,
    r#"post-009-{"title"_"help""#,
    r#"post-010-{"title"_"about""#,
];

const BODIES: &[&str] = &[
    r#""post"_"welcome to the forum"}"#,
    r#""post"_"common threads"}"#,
    r#""post"_"how do I do this"}"#,
    r#""post"_"intro code for beginners"}"#,
    r#""post"_"know enough beyond beginner"}"#,
    r#""post"_"I am sharing tips"}"#,
    r#""post"_"frequently asked questions"}"#,
    r#""post"_"useful contacts"}"#,
    r#""post"_"forum development"}"#,
    r#""post"_"contact information"}"#,
];

pub static INDEX_HTML: &str = r#"
<!DOCTYPE html>
<html>
    <head>
        <title>Warp SSE FDB Example</title>
    </head>
    <style>
    .divstyle
    {
        border-style:solid;
        border-color:black;
        border-width:1px;
        font-size: 20px;
        width: 33%;
    }
    .poststyle
    {
        border-style:solid;
        border-color:black;
        border-width:1px;
        font-size: 15px;
        background-color: #cbe2f7;
    }
    </style>
    <body>
        <h1>Warp SSE FDB Example</h1>
        <div id="chat">
            <p><em>Connecting...</em></p>
        </div>
        <input type="text" id="text" />&nbsp;<input type="value" id="value" />
        <button type="button" id="send">Add</button>
        <button type="button" id="status">Status</button>
        <button onclick="removedom('s0')">Delete</button>
        <button type="button" id="commit">Commit</button>
        <script type="text/javascript">
        var uri = 'http://' + location.host + '/chat';
        var uristat = 'http://' + location.host + '/status';
        var uridel = 'http://' + location.host + '/delete';
        var uricommit = 'http://' + location.host + '/commit';
        var sse = new EventSource(uri);
        function removedom(msgidx) { 
            
            console.log(msgidx);
            //var xhr = new XMLHttpRequest();
            //xhr.open("POST", uridel + '/' + msgidx.charAt(1), true);
            //xhr.send(msgidx.charAt(1));
        }
        function message(data) {
            var line = document.createElement('p');
            for(var i = 0; i < data.split(',').length - 1; i++) 
            {
                var msgstr = data.split(',')[i];
                var msgidx = msgstr.split('::')[0];
                if(i !== 0) {
                    var pmsg = msgstr.split('-');
                    var pjmsg = pmsg[2].replaceAll('|',',').replaceAll('_',':');
                    var pobj = JSON.parse(pjmsg);
                    pobj.idx = pmsg[0]+"-"+pmsg[1];
                    console.log(pobj);
                    line.innerHTML += 
                    "<div id='"+pobj.idx+"' class='divstyle'>"+
                    pobj.title+
                    "<div class='poststyle'>"+
                    pobj.post+
                    "<div><button onclick='removedom(\""+pobj.idx+"\")'>Delete</button></div>"+
                    "</div></div>";
                }
                else {
                    //var msgstrtrim = msgstr.split('::')[1];
                    line.innerHTML += "<div id='"+msgidx+"' class='divstyle'><div>"+msgidx+"</div></div>";
                    chat.appendChild(line);
                }
            }            
            
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
        document.getElementById('status').onclick = function() {
            var msg = text.value;
            var xhr = new XMLHttpRequest();
            xhr.open("POST", uristat + '/' + user_id, true);
            xhr.send(msg);
            text.value = '';
            message('<You>: ' + msg);
        };
        document.getElementById('commit').onclick = function() {
            var msg = text.value;
            var xhr = new XMLHttpRequest();
            xhr.open("POST", uricommit + '/' + user_id, true);
            xhr.send(msg);
            text.value = '';
            message('<You>: ' + msg);
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

//Data Model is 1-to-1 post to body

fn init_posts(trx: &Transaction, all_posts: &[String]) {
    //let post_subspace = Subspace::from("post");
    for post in all_posts {
        
        let mut kvs = post.split(":");
        let k = kvs.next();
        let v = kvs.next();
        
        trx.set(&k.unwrap().to_string().as_bytes(), &v.unwrap().to_string().as_bytes());
    }
}

lazy_static! {
    pub static ref ALL_POSTS: Vec<String> = all_posts();
}

// TODO: make these tuples?
fn all_posts() -> Vec<String> {
    let mut post_names: Vec<String> = Vec::new();
    for post in POSTS {
        post_names.push(format!("{}:{}", 
        post, 
        BODIES[POSTS.iter().position(|x| x == post).unwrap()]));    
    }
    post_names
}

async fn get_post_trx(trx: &Transaction, post_key: String, mut post_val: &str) -> Result<String> {

    let key = post_key.as_bytes();
    
    let pval = trx.get(&key, false)
    .await
    .expect("failed to get post");

    let post_value = pval.unwrap();

    post_val = str::from_utf8(&post_value.as_ref()).unwrap();

    Ok(String::from(post_val))
}

async fn set_post_trx(trx: &Transaction, post_key: String, post_val: &str) -> Result<()> {

    let key = post_key.as_bytes();
    
    trx.set(&key, post_val.as_bytes());

    Ok(())
}

async fn create_post_trx(trx: &Transaction, post: &str, body: &str) -> Result<()> {
    
    let post_key = pack(&("post", post, body));
    if trx
        .get(&post_key, true)
        .await
        .expect("get failed")
        .is_some()
    {
        //println!("{} already taking class: {}", student, class);
        
        return Ok(());
    }

    //println!("{} taking class: {}", student, class);
    println!("Created Post: {} {}", post, body);
    trx.set(&post_key, &pack(&""));
    Ok(())
}

async fn commit_post_trx(trx: &Transaction, post: &str, body: &str, db: &Database) -> Result<()> {
    let post_key = pack(&("post", post, body));
    if trx
        .get(&post_key, true)
        .await
        .expect("get failed")
        .is_some()
    {
        //println!("{} already taking class: {}", student, class);
        
        return Ok(());
    }

    let ntrx = db.create_trx().expect("could not create transaction");
    ntrx.set(post.as_bytes(), body.as_bytes());
    println!("Committing Post: {} {}", post, body);
    ntrx.commit().await.expect("failed to commit post data");
    Ok(())
}

async fn create_post(db: &Database, post: String, body: String) -> Result<()> {
    db.transact_boxed_local(
        (post, body),
        |trx, (post, body)| create_post_trx(&trx, post, body).boxed_local(),
        TransactOption::default(),
    )
    .await   
}

async fn get_post(db: &Database, post: String, mut post_val: &str) -> Result<String> {
   
    let trx = db.create_trx().expect("could not create transaction");
    let outstr = get_post_trx(&trx, post.to_string(), post_val).await;
    outstr
}

async fn commit_post(db: &Database, post: String, body: String) -> Result<()> {
    db.transact_boxed_local(
        (post, body, db),
        |trx, (post, body, db)| commit_post_trx(&trx, post, body, db).boxed_local(),
        TransactOption::default(),
    )
    .await
}

async fn delete_post_trx(trx: &Transaction, post: &str, body: &str) {

    let post_key = pack(&("post", post, body));

    if trx
        .get(&post_key, true)
        .await
        .expect("get failed")
        .is_none()
    {
        return;
    }

    println!("Deleting posts...");
    println!("Deleted Post: {} {}", post, body);
    trx.clear(&post_key);
}

async fn delete_post(db: &Database, post: String, body: String) -> Result<()> {
    
    let trx = db.create_trx().expect("could not create transaction");

    let post_key = pack(&("post", post, body));
    
    trx.clear(&post_key);

    trx.commit().await.expect("failed to initialize data");

    println!("Deleting posts...");

    Ok(())
}

pub async fn init(db: &Database, all_posts: &[String]) {
    let trx = db.create_trx().expect("could not create transaction");
    //trx.clear_subspace_range(&"post".into());
    let key_begin = "post-";
    let key_end = "post.";

    trx.clear_range(key_begin.as_bytes(), key_end.as_bytes());

    init_posts(&trx, all_posts);

    trx.commit().await.expect("failed to initialize data");
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Post {
    Add,
    Delete,
    Commit,
    Get,
    //Update,
}

async fn perform_posts_op(
    db: &Database,
    rng: &mut ThreadRng,
    post: Post,
    post_id: &str,
    all_posts: &[String],
    my_posts: &mut Vec<String>,
) -> Result<()> {
    match post {
        Post::Add => {
            let post = all_posts.choose(rng).unwrap();
            create_post(&db, post_id.to_string(), post.to_string()).await?;
            my_posts.push(post.to_string());
        }
        Post::Delete => {
            let post = all_posts.choose(rng).unwrap();
            delete_post(&db, post_id.to_string(), post.to_string()).await?;
            my_posts.retain(|s| s != post);
        }
        Post::Commit => {
            let post = all_posts.choose(rng).unwrap();
            commit_post(&db, post_id.to_string(), post.to_string()).await?;
            my_posts.push(post.to_string());
        }
        Post::Get => {
            //TODO: pick a lane!
            let mut postval = "";
            let pvout = get_post(&db, post_id.to_string(), postval).await?;
            let post = String::from(pvout);
            my_posts.push(post.to_string())
        }
        /*Mood::Switch => {
            let old_class = my_classes.choose(rng).unwrap().to_string();
            let new_class = all_classes.choose(rng).unwrap();
            switch_classes(
                &db,
                student_id.to_string(),
                old_class.to_string(),
                new_class.to_string(),
            )
            .await?;
            my_classes.retain(|s| s != &old_class);
            my_classes.push(new_class.to_string());
        }*/
    }
    Ok(())
}

async fn posts_op(post_id: usize, num_ops: usize) {
    let db = Database::new_compat(None)
        .await
        .expect("failed to get database");

    let post_id = format!("s{}", post_id);

    //1 worker will pick at random 1-10 posts
    let mut rng = rand::thread_rng();

    let mut available_posts = Cow::Borrowed(&*ALL_POSTS);
    let mut my_posts = Vec::<String>::new();

    for _ in 0..num_ops {
        let mut posts = Vec::<Post>::new();

        //Add posts
        posts.push(Post::Add);

        //Commit post
        //posts.push(Post::Commit);
        
        //Choose posts from random collection
        //let post = posts.choose(&mut rng).map(|post| *post).unwrap();

        let post = posts.iter().last().map(|post| *post).unwrap();

        // on errors we recheck for available posts
        if perform_posts_op(
            &db,
            &mut rng,
            post,
            &post_id,
            &available_posts,
            &mut my_posts,
        )
        .await
        .is_err()
        {
            println!("getting available posts");
            available_posts = Cow::Owned(get_available_posts(&db).await);
        }

        
    }


}

async fn posts_op_del(post_id: usize, num_ops: usize, postid: usize) {
    let db = Database::new_compat(None)
        .await
        .expect("failed to get database");

    let post_id = format!("s{}", post_id);

    //1 worker will pick at random 1-10 posts
    let mut rng = rand::thread_rng();

    let mut available_posts = Cow::Borrowed(&*ALL_POSTS);
    let mut my_posts = Vec::<String>::new();

    for _ in 0..num_ops {
        let mut posts = Vec::<Post>::new();

        posts.push(Post::Delete);
        
        let post = posts.iter().last().map(|post| *post).unwrap();

        // on errors we recheck for available posts
        if perform_posts_op(
            &db,
            &mut rng,
            post,
            &post_id,
            &available_posts,
            &mut my_posts,
        )
        .await
        .is_err()
        {
            println!("getting available posts");
            available_posts = Cow::Owned(get_available_posts(&db).await);
        }

        
    }


}

async fn posts_op_get(post_id: usize, num_ops: usize) {
    let db = Database::new_compat(None)
        .await
        .expect("failed to get database");

    let post_id = format!("s{}", post_id);

    //1 worker will pick at random 1-10 posts
    let mut rng = rand::thread_rng();

    let mut available_posts = Cow::Borrowed(&*ALL_POSTS);
    let mut my_posts = Vec::<String>::new();

    for _ in 0..num_ops {
        let mut posts = Vec::<Post>::new();

        //Get posts
        posts.push(Post::Get);

        //Commit post
        //posts.push(Post::Commit);
        
        //Choose posts from random collection
        //let post = posts.choose(&mut rng).map(|post| *post).unwrap();

        let post = posts.iter().last().map(|post| *post).unwrap();

        // on errors we recheck for available posts
        if perform_posts_op(
            &db,
            &mut rng,
            post,
            &post_id,
            &available_posts,
            &mut my_posts,
        )
        .await
        .is_err()
        {
            println!("getting available posts");
            available_posts = Cow::Owned(get_available_posts(&db).await);
        }

        
    }


}

async fn posts_op_commit(post_id: usize, num_ops: usize) {
    let db = Database::new_compat(None)
        .await
        .expect("failed to get database");

    let post_id = format!("s{}", post_id);

    //1 worker will pick at random 1-10 posts
    let mut rng = rand::thread_rng();

    let mut available_posts = Cow::Borrowed(&*ALL_POSTS);
    let mut my_posts = Vec::<String>::new();

    for _ in 0..num_ops {
        let mut posts = Vec::<Post>::new();

        //Commit post
        posts.push(Post::Commit);
        
        //Choose posts from random collection
        //let post = posts.choose(&mut rng).map(|post| *post).unwrap();

        let post = posts.iter().last().map(|post| *post).unwrap();

        // on errors we recheck for available posts
        if perform_posts_op(
            &db,
            &mut rng,
            post,
            &post_id,
            &available_posts,
            &mut my_posts,
        )
        .await
        .is_err()
        {
            println!("getting available posts");
            available_posts = Cow::Owned(get_available_posts(&db).await);
        }

        
    }


}

pub async fn get_available_posts(db: &Database) -> Vec<String> {
    let trx = db.create_trx().expect("could not create transaction");

    
    let key_begin = "post-";

    let key_end = "post.";

    let begin = KeySelector::first_greater_or_equal(Cow::Borrowed(key_begin.as_bytes()));
    let end = KeySelector::first_greater_than(Cow::Borrowed(key_end.as_bytes()));
    let opt = RangeOption::from((begin, end));

    //let opt = RangeOption::from(&Subspace::from("post"));

    let got_range = trx.get_range(&opt, 1_024, false).await;

    //let range = RangeOption::from(&Subspace::from("post"));

    
    let mut available_posts = Vec::<String>::new();        
    
    for key_values in &got_range {
        let count: usize = key_values.len();

        for key_value in key_values {
            if count > 0 {
                
                //let (k, v) = unpack::<(String, String)>(key_value.key()).unwrap();
                
                let k = String::from(str::from_utf8(key_value.key()).unwrap());
                let v = String::from(str::from_utf8(key_value.value()).unwrap());

                let postcomp = format!("{}|{}", k, v);
                available_posts.push(postcomp);
            }
        }
    }

    available_posts
}

pub async fn run_query(db: &Database, poolsize: usize, ops_per_pool: usize) {

    let mut threads: Vec<(usize, thread::JoinHandle<()>)> = Vec::with_capacity(poolsize);

    for i in 0..poolsize {
        // TODO: ClusterInner has a mutable pointer reference, if thread-safe, mark that trait as Sync, then we can clone DB here...
        threads.push((
            i,
            thread::spawn(move || {
                futures::executor::block_on(posts_op(i, ops_per_pool));
            }),
        ));
    }

    for (id, thread) in threads {
        thread.join().expect("failed to join thread");

        let post_id = format!("s{}", id);
        let post_range = RangeOption::from(&("post", &post_id).into());

        for key_value in db
            .create_trx()
            .unwrap()
            .get_range(&post_range, 1_024, false)
            .await
            .expect("post_range failed")
            .iter()
        {
            let (_, s, body) = unpack::<(String, String, String)>(key_value.key()).unwrap();
            assert_eq!(post_id, s);

            println!("has body: {}", body);
        }
    }

    println!("Ran {} transactions", poolsize * ops_per_pool);

}

pub async fn run_query_posts(db: &Database, post_val: String) -> Vec<String>{

    let mut received_posts = Vec::<String>::new();

    let outstr = async_tokio_set(db, post_val).await;

    received_posts.push(outstr.unwrap());

    received_posts
}

pub async fn render_posts(db: &Database) -> Vec<String>{

    let mut posts = Vec::<String>::new();
    
    let available_posts = get_available_posts(&db).await;

    for currentpost in available_posts {
        println!("{}", currentpost);
        posts.push(currentpost.to_owned());
    }

    posts

}

pub async fn async_tokio_get(db: &Database) -> foundationdb::FdbResult<String> {
    
    // write a value
    let trx = db.create_trx()?;
    trx.set(b"thisis", b"tokio"); // errors will be returned in the future result
    
    trx.commit().await?;
    
    // read a value
    let trx = db.create_trx()?;
    let maybe_value = trx.get(b"thisis", false).await?;
    let value = maybe_value.unwrap(); // unwrap the option

    let outval = str::from_utf8(&value.as_ref()).unwrap();

    //assert_eq!(b"tokio", &value.as_ref());
    
    Ok(String::from(outval))
}

pub async fn async_tokio_set(db: &Database, val: String) -> foundationdb::FdbResult<String> {
    
    // write a value
    let trx = db.create_trx()?;
    trx.set(b"thisis", val.as_bytes()); // errors will be returned in the future result
    trx.commit().await?;
    
    // read a value
    let trx = db.create_trx()?;
    let maybe_value = trx.get(b"thisis", false).await?;
    let value = maybe_value.unwrap(); // unwrap the option

    let outval = str::from_utf8(&value.as_ref()).unwrap();

    //assert_eq!(b"tokio", &value.as_ref());
    
    Ok(String::from(outval))
}

pub async fn delete_post_query(db: &Database, postid: usize) -> Vec<String>{

    let mut threads: Vec<(usize, thread::JoinHandle<()>)> = Vec::with_capacity(POOLSZ);

    for i in 0..POOLSZ {
        // TODO: ClusterInner has a mutable pointer reference, if thread-safe, mark that trait as Sync, then we can clone DB here...
        threads.push((
            i,
            thread::spawn(move || {
                futures::executor::block_on(posts_op_del(i, WORKSZ, postid));
            }),
        ));
    }

    let mut received_posts = Vec::<String>::new();

    for (id, thread) in threads {
        thread.join().expect("failed to join thread");

        let post_id = format!("s{}", id);
        let post_range = RangeOption::from(&("post", &post_id).into());

        for key_value in db
            .create_trx()
            .unwrap()
            .get_range(&post_range, 1_024, false)
            .await
            .expect("post_range failed")
            .iter()
        {
            let (_, s, body) = unpack::<(String, String, String)>(key_value.key()).unwrap();
            assert_eq!(post_id, s);

            //println!("has body: {}", body);
            let postcomp = format!("{}::{}", post_id, body);
            received_posts.push(postcomp);
        }
    }

    //println!("Ran {} transactions", poolsize * ops_per_pool);
    received_posts
}

pub async fn commit_posts_query(db: &Database) -> Vec<String>{

    let mut threads: Vec<(usize, thread::JoinHandle<()>)> = Vec::with_capacity(POOLSZ);

    for i in 0..POOLSZ {
        // TODO: ClusterInner has a mutable pointer reference, if thread-safe, mark that trait as Sync, then we can clone DB here...
        threads.push((
            i,
            thread::spawn(move || {
                futures::executor::block_on(posts_op_commit(i, WORKSZ));
            }),
        ));
    }

    let mut received_posts = Vec::<String>::new();

    for (id, thread) in threads {
        thread.join().expect("failed to join thread");

        let post_id = format!("s{}", id);
        let post_range = RangeOption::from(&("post", &post_id).into());

        for key_value in db
            .create_trx()
            .unwrap()
            .get_range(&post_range, 1_024, false)
            .await
            .expect("post_range failed")
            .iter()
        {
            let (_, s, body) = unpack::<(String, String, String)>(key_value.key()).unwrap();
            assert_eq!(post_id, s);

            //println!("has body: {}", body);
            let postcomp = format!("{}::{}", post_id, body);
            received_posts.push(postcomp);
        }
    }

    //println!("Ran {} transactions", poolsize * ops_per_pool);
    received_posts
}

