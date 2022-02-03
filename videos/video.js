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
    var titlediv = document.getElementsByClassName(updarr[0] + "-" + updarr[1]);
    var postdiv = document.getElementsByClassName(msgidx);
    for (var i = 0; i <= titlediv.length - 1; i++) {
        titlediv[i].contentEditable = true;
    }
    for (var j = 0; j <= postdiv.length - 1; j++) {
        postdiv[j].contentEditable = true;
    }
}
function savedom(msgidx) {
    var updarr = msgidx.split('-');
    var titlediv = document.getElementsByClassName(updarr[0] + "-" + updarr[1]);
    var postdiv = document.getElementsByClassName(msgidx);
    var key = titlediv[titlediv.length - 1].textContent;
    var value = postdiv[postdiv.length - 1].textContent;
    var kv = `post-${updarr[1]}-{"title"_"${key}"|"post"_"${value}"}`;
    var xhr = new XMLHttpRequest();
    xhr.open("POST", uricreate + '/' + kv, true);
    xhr.send(kv);
    text.value = '';
}
function message(data) {
    var line = document.createElement('p');
    postslength = data.split(',').length - 1;
    for (var i = 0; i < postslength; i++) {
        var msgstr = data.split(',')[i];
        var msgidx = msgstr.split('::')[0];
        if (i !== 0) {
            var pmsg = msgstr.split('-');
            var pjmsg = pmsg[2].replaceAll('|', ',').replaceAll('_', ':');
            var pobj = JSON.parse(pjmsg);
            pobj.idx = pmsg[0] + "-" + pmsg[1];
            var btnidx = pobj.idx + "-" + pobj.title;
            line.innerHTML +=
                "<div id='" + pobj.idx + "' class='" + pobj.idx + "'>" +
                pobj.title +
                "</div>" +
                "<div id='" + btnidx + "' class='" + btnidx + "'>" +
                pobj.post +
                "</div>" +
                "<div><button onclick='removedom(\"" + btnidx + "\")'>Delete</button>" +
                "<button onclick='updatedom(\"" + btnidx + "\")'>Update</button>" +
                "<button onclick='savedom(\"" + btnidx + "\")'>Save</button></div>";
        }
        else {
            //var msgstrtrim = msgstr.split('::')[1];
            line.innerHTML += "<div id='" + msgidx + "' class='divstyle'><div>" + msgidx + "</div></div>";
            chat.appendChild(line);
        }
    }
    window.scrollByPages(1);
}
sse.onopen = function () {
    chat.innerHTML = "<p><em>Connected!</em></p>";
}
var user_id;
sse.addEventListener("user", function (msg) {
    user_id = msg.data;
});
sse.onmessage = function (msg) {
    message(msg.data);
};
document.getElementById('status').onclick = function () {
    var msg = text.value;
    var xhr = new XMLHttpRequest();
    xhr.open("POST", uristat + '/' + user_id, true);
    xhr.send(msg);
    text.value = '';
    message('<You>: ' + msg);
};
document.getElementById('create').onclick = function () {
    var key = inkey.value;
    var value = inval.value;
    var idx = zeroPad(postslength, 3);
    var kv = `post-${idx}-{"title"_"${key}"|"post"_"${value}"}`;
    var xhr = new XMLHttpRequest();
    xhr.open("POST", uricreate + '/' + kv, true);
    xhr.send(kv);
    text.value = '';
};
send.onclick = function () {
    var msg = text.value;
    var xhr = new XMLHttpRequest();
    xhr.open("POST", uri + '/' + user_id, true);
    xhr.send(msg);
    text.value = '';
    message('<You>: ' + msg);
};