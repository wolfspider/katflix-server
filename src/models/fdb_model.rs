
use std::borrow::Cow;
use std::str;
use foundationdb::{Database, FdbError, RangeOption, TransactError, Transaction, KeySelector};

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
        width: 33%;
    }
    </style>
    <body>
        <h1>Warp SSE FDB Example</h1>
        <div id="chat">
            <p><em>Connecting...</em></p>
        </div>
        <div id="add-fields" style="background-color: #88a2b9; width: 33%;">
        <p>Title</p><input type="text" id="inkey" />
        <p>Post</p><input type="text" id="inval" />
        <button type="button" id="create">Create</button>
        </div>
        <div id="ctrl-fields">
        <p>Chat</p>
        <input type="text" id="text" />
        <button type="button" id="send">Send</button>
        <button type="button" id="status">Get</button>
        </div>
        <script type="text/javascript">
        var uri = 'http://' + location.host + '/chat';
        var uristat = 'http://' + location.host + '/status';
        var uridel = 'http://' + location.host + '/delete';
        var uricommit = 'http://' + location.host + '/commit';
        var uricreate = 'http://' + location.host + '/create';
        var postslength = 0;
        const zeroPad = (num, places) => String(num).padStart(places, '0');
        var sse = new EventSource(uri);
        function removedom(msgidx) { 
            var delarr = msgidx.split('-');
            var key = `post-${delarr[1]}-{"title"_"${delarr[2]}"`;
            var xhr = new XMLHttpRequest();
            xhr.open("POST", uridel + '/' + key, true);
            xhr.send(msgidx);
        }
        function updatedom(msgidx) { 
            var updarr = msgidx.split('-');
            var titlediv = document.getElementById(updarr[0]+"-"+updarr[1]);
            var postdiv = document.getElementById(msgidx);
            titlediv.contentEditable = true;
            postdiv.contentEditable = true;
        }
        function savedom(msgidx) { 
            var updarr = msgidx.split('-');
            var titlediv = document.getElementById(updarr[0]+"-"+updarr[1]);
            var postdiv = document.getElementById(msgidx);
            var key = titlediv.textContent;
            var value = postdiv.textContent;
            var kv = `post-${updarr[1]}-{"title"_"${key}"|"post"_"${value}"}`;
            var xhr = new XMLHttpRequest();
            xhr.open("POST", uricreate + '/' + kv, true);
            xhr.send(kv);
            text.value = '';
        }
        function message(data) {
            var line = document.createElement('p');
            postslength = data.split(',').length - 1;
            for(var i = 0; i < postslength; i++) 
            {
                var msgstr = data.split(',')[i];
                var msgidx = msgstr.split('::')[0];
                if(i !== 0) {
                   var pmsg = msgstr.split('-');
                    var pjmsg = pmsg[2].replaceAll('|',',').replaceAll('_',':');
                    var pobj = JSON.parse(pjmsg);
                    pobj.idx = pmsg[0]+"-"+pmsg[1];
                    var btnidx = pobj.idx+"-"+pobj.title;
                   line.innerHTML += 
                    "<div id='"+pobj.idx+"' class='divstyle'>"+
                    pobj.title+
                    "</div>"+
                    "<div id='"+btnidx+"' class='poststyle'>"+
                    pobj.post+
                    "</div>"+
                    "<div><button onclick='removedom(\""+btnidx+"\")'>Delete</button>"+
                    "<button onclick='updatedom(\""+btnidx+"\")'>Update</button>"+
                    "<button onclick='savedom(\""+btnidx+"\")'>Save</button></div>";
                }
                else {
                    //var msgstrtrim = msgstr.split('::')[1];
                    line.innerHTML += "<div id='"+msgidx+"' class='divstyle'><div>"+msgidx+"</div></div>";
                    chat.appendChild(line);
                }
            }            
            window.scrollByPages(1);
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
        document.getElementById('create').onclick = function() {
            var key = inkey.value;
            var value = inval.value;
            var idx = zeroPad(postslength, 3);
            var kv = `post-${idx}-{"title"_"${key}"|"post"_"${value}"}`;
            var xhr = new XMLHttpRequest();
            xhr.open("POST", uricreate + '/' + kv, true);
            xhr.send(kv);
            text.value = '';
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

fn all_posts() -> Vec<String> {
    
    let mut post_names: Vec<String> = Vec::new();
    for post in POSTS {
        post_names.push(format!("{}:{}", 
        post, 
        BODIES[POSTS.iter().position(|x| x == post).unwrap()]));    
    }
    post_names
}

pub async fn delete_post_async(db: &Database, key: String) -> foundationdb::FdbResult<String> {
    
    let trx = db.create_trx().expect("could not create transaction");

    let k = key.clone();

    trx.clear(k.as_bytes());
    
    trx.commit().await.expect("failed to commit deletion");

    Ok(String::from("Deleted posts"))
}

pub async fn init(db: &Database, all_posts: &[String]) {
    
    let trx = db.create_trx().expect("could not create transaction");
    
    let key_begin = "post-";
    let key_end = "post.";

    trx.clear_range(key_begin.as_bytes(), key_end.as_bytes());

    init_posts(&trx, all_posts);

    trx.commit().await.expect("failed to initialize data");
}


pub async fn get_available_posts(db: &Database) -> Vec<String> {
    
    let trx = db.create_trx().expect("could not create transaction");

    let key_begin = "post-";

    let key_end = "post.";

    let begin = KeySelector::first_greater_or_equal(Cow::Borrowed(key_begin.as_bytes()));
    let end = KeySelector::first_greater_than(Cow::Borrowed(key_end.as_bytes()));
    let opt = RangeOption::from((begin, end));

    let got_range = trx.get_range(&opt, 1_024, false).await;

    let mut available_posts = Vec::<String>::new();        
    
    for key_values in &got_range {
        let count: usize = key_values.len();

        for key_value in key_values {
            if count > 0 {
                
                let k = String::from(str::from_utf8(key_value.key()).unwrap());
                let v = String::from(str::from_utf8(key_value.value()).unwrap());

                let postcomp = format!("{}|{}", k, v);
                available_posts.push(postcomp);
            }
        }
    }

    available_posts
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

pub async fn create_post_async(db: &Database, key: String, value: String) -> foundationdb::FdbResult<String> {
    
    // write a value
    let trx = db.create_trx()?;

    let k = key.clone();

    let v = value.clone();

    trx.set(k.as_bytes(), v.as_bytes()); // errors will be returned in the future result
    trx.commit().await?;
    
    // read a value
    let trx = db.create_trx()?;
    let maybe_value = trx.get(k.as_bytes(), false).await?;
    let value = maybe_value.unwrap(); // unwrap the option

    let outval = str::from_utf8(&value.as_ref()).unwrap();
    
    Ok(String::from(outval))
}

