//! Example chat application.
//!
//! Run with
//!
//! ```not_rust
//! cd examples && cargo run -p example-chat
//! ```
mod qa_structs;
use qa_structs::*;
mod query_helper;
use query_helper::*;
use axum_macros::*;
use axum::{
    extract::{
        Path,
        Query,
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    body::Body,
    extract,
    // handler::post,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Router, Json, http::StatusCode,
    Error
};
use serde_json::{self, Value};
use serde::{Serialize, Deserialize};


use futures::{sink::SinkExt, stream::StreamExt};

use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    sync::{Arc, Mutex}, ops::Shr, os::macos::raw::stat,
};
use tokio::sync::broadcast;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use rand::{self, thread_rng};


// Our shared state
struct AppState {
    user_set: Mutex<HashSet<String>>,
    tx: broadcast::Sender<String>,
    qa_list: Vec<ShrekQA>,
    count_ready: Mutex<u32>
}

// struct RoomState {
//     user_set: <HashSet<String,
//     tx: broadcast::Sender<String>,
//     qa_list: Vec<ShrekQA>,
//     count_ready: Mutex<u32>
// }

struct AppStateAll {
    rooms: Vec<Arc<AppState>>
}

struct RoomStateTest{
    user_set: HashSet<String>,
    tx: broadcast::Sender<String>,
    qa_list: Vec<ShrekQA>,
    count_ready: u32
}
impl RoomStateTest {
    fn new() -> Self{
        Self{
            user_set: HashSet::new(),
            tx: broadcast::channel(100).0,
            qa_list: extract_csv().unwrap(),
            count_ready: 0
        }
        
    }
}
struct AppStateController{
    rooms: Mutex<HashMap<String, RoomStateTest>>
}


enum SMType{
    Message,
    Answer, //could use custom enums here
    Ready,
    QA_list,
    Join

}

impl SMType{
    fn as_str(&self) -> String {
        match self {
            SMType::Message => format!("message"),
            SMType::Answer => format!("answer"), //could use custom enums here
            SMType::Ready => format!("Ready"),
            SMType::QA_list => format!("qa_list"),
            SMType::Join => format!("join")
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct SocketMessage{
    sm_type: String,
    username: String,
    payload: String
}

#[derive(Deserialize, Debug)]
struct User {
    username: String,
}

async fn create_user(extract::Json(payload): extract::Json<User>) {
    // payload is a `CreateUser`
    print!("{:?}", payload);
    print!("recieved user");
}
// const THING: u32 = 0xABAD1DEA;

#[tokio::main]
async fn main() {

    // yoyo();
    // let qalist = make_list();
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "example_chat=trace".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();
    
    let user_set = Mutex::new(HashSet::new());
    let (tx, _rx) = broadcast::channel(100);
    
    let mut question_list: Vec<ShrekQA> = vec![];
    if let Ok(ql) = extract_csv(){
        question_list = ql;
    }
    let app_state = Arc::new(AppState { 
        user_set, 
        tx, 
        qa_list: question_list, 
        count_ready: Mutex::new(0)
    });

    let app_state_all = Arc::new(AppStateController{
        rooms: Mutex::new( HashMap::new())
    });
    // let app_state = Arc::new()
    // let asa: Vec<Arc<RoomState>> = vec![];

    let app = Router::with_state(app_state_all)
        .route("/", get(index))
        // .route("/create_room", get(create_room))
        .route("/websocket/:id", get(join_handler))
        .route("/:id", get(join_handler));
        // .route("/websocket", get(websocket_handler));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

// #[debug_handler]
async fn create_room(
    State(asa): State<Arc<Mutex<AppStateAll>>>
) -> Result <(), &'static str>{
    let user_set = Mutex::new(HashSet::new());
    let (tx, _rx) = broadcast::channel(100);
    
    let mut question_list: Vec<ShrekQA> = vec![];
    if let Ok(ql) = extract_csv(){
        question_list = ql;
    }
    let app_state = Arc::new(AppState { 
        user_set, 
        tx, 
        qa_list: question_list, 
        count_ready: Mutex::new(0)
    });
    let mut asa_lock = asa.lock().unwrap();
    asa_lock.rooms.push(app_state);
    tracing::debug!("New room created! #{}", asa_lock.rooms.len());

    // asa.loc .rooms.push(app_state);
    Ok(())
    // ws.on_upgrade(|socket| websocket(socket, state))
}


// async fn join_room(
//     ws: WebSocketUpgrade,
//     State(state): State<Arc<Mutex<AppStateAll>>>,
// ) -> impl IntoResponse {
//     let room = state.lock().unwrap().rooms[0].clone();
//     ws.on_upgrade(|socket| websocket(socket, room))
// }

async fn extract_params(Query(params): Query<Params>) -> Option<usize> {
    
    match params.room_num {
        Some(p) => {
            tracing::debug!("room_id #{:?} extracted", p);
            // format!("{:?}", p);
            Some(p)
        },
        None => {
            tracing::debug!("Could not read params");
            // format!("{:?}", params);
            None
        }
    }
}

async fn check_room() -> Response{
    (StatusCode::OK).into_response()
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct RoomId{
    rid: u32
}

// #[axum_macros::debug_handler]
async fn join_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppStateController>>,
    Path(rid): Path<u32>
) -> impl IntoResponse {

    let s_clone  = state.clone();
    // let room = rid;
    let room = RoomId{
        rid: 32
    };

    // let mut rooms = s_clone.rooms.lock().unwrap();
    // if let res = rooms.contains_key(&room.rid.to_string()){
    //     tracing::debug!("key {} find", room.rid);
    //     // (format!("does contain key {}", room.rid)).into_response()
    //     // ws.on_upgrade(|socket| websocket(
    //     //     socket, 
    //     //     state
    //     // ))

    // } else {
    //     // (format!("does not contain key {}", room.rid)).into_response()
    // }
    tracing::debug!("Doctor, we have a successful fusion reaction ");
    ( Json(room)).into_response()
    // StatusCode::INTERNAL_SERVER_ERROR.into_response()
    // Error::new("error")
}

// #[debug_handler]
// async fn websocket_handler(
//     ws: WebSocketUpgrade,
//     State(state): State<Arc<AppState>>,
// ) -> impl IntoResponse {
//     ws.on_upgrade(|socket| websocket(socket, state))
// }

// async fn websocket(stream: WebSocket, state: Arc<AppStateController>) {

//     // By splitting we can send and receive at the same time.
//     let (mut sender, mut receiver) = stream.split();
    
//     // Username gets set in the receive loop, if it's valid.
//     let mut username = String::new();
//     // Loop until a text message is found.
//     while let Some(Ok(message)) = receiver.next().await {
//         if let Message::Text(name) = message {
//             // If username that is sent by client is not taken, fill username string.
//             check_username(&state, &mut username, &name);

//             // If not empty we want to quit the loop else we want to quit function.
//             if !username.is_empty() {
//                 // create_user(name);
//                 break;
//             } else {
//                 // Only send our client that username is taken.
//                 let _ = sender
//                     .send(Message::Text(String::from("Username already taken.")))
//                     .await;

//                 return;
//             }
//         }
//     }

//     // Subscribe before sending joined message.
//     let mut rx = state.tx.subscribe();

//     // Clone a subsect of questions
//     let mut qa_list_clone = state.qa_list.clone();

//     // Send joined message to all subscribers.
//     let msg = format!("{} joined.", username);
//     tracing::debug!("{}", msg);
//     let sm = SocketMessage{
//         sm_type: SMType::Join.as_str(),
//         payload: msg,
//         username: username.clone()
//     };
    
//     let _ = state.tx.send(serde_json::to_string(&sm).unwrap());

//     // This task will receive broadcast messages and send text message to our client.
//     let mut send_task = tokio::spawn(async move {
//         while let Ok(text) = rx.recv().await {

//             // In any websocket error, break loop.
//             if sender.send(Message::Text(text)).await.is_err() {
//                 break;
//             }
//         }
//     });

//     // let _ = state.tx.send(serde_json::to_string(&state.qa_list[0]).unwrap());
//     // let qa_list_clone = serde_json::to_string(&state.qa_list).unwrap();
//     // let serialized_user = serde_json::to_string(&user).unwrap();


//     // Clone things we want to pass to the receiving task.
//     let tx = state.tx.clone();
//     let name = username.clone();
//     let mut prog = 0;
    
//     // const l: String = String::from("s");
//     let mut state_clone = state.clone();
//     // This task will receive messages from client and send them to broadcast subscribers.
//     let mut recv_task = tokio::spawn(async move {
//         while let Some(Ok(Message::Text(text))) = receiver.next().await {
            
//             // let Json()
//             // let helpme: User = serde_json::from_str(&text).unwrap();
//             let socket_message: SocketMessage= serde_json::from_str(&text).unwrap();
//             match &socket_message.sm_type[..] {
//                 "message" => {
//                     tracing::debug!("mes");
//                     let m = SocketMessage{
//                         username: name.clone(),
//                         payload: format!(": {}", socket_message.payload) ,
//                         sm_type: SMType::Message.as_str(),
//                     };
//                     let str_mes = serde_json::to_string(&m).unwrap();
//                     let _ = tx.send(str_mes );
//                 },
//                 "ready" => {
//                     tracing::debug!("{name} is ready");
//                     if let check = check_start(&state_clone){
//                         tracing::debug!("Starting game"); 

//                         let sm = SocketMessageQAList{
//                             payload: qa_list_clone.clone(),
//                             sm_type: SMType::QA_list.as_str()
//                         }; 
//                         let _ = tx.send(serde_json::to_string(&sm).unwrap());
//                     };
//                     // check_start(state);
//                 },
//                 "answer" => tracing::debug!("ans"),
//                 // _ if socket_message.sm_type.equals("message") => print!("message"),
//                 _ => print!("none")
//             };

//             // Add username before message.
//             // serde_json::from_str(&text);
//             // let _ = tx.send(format!("{}: {}", name, text));
            
//         }
//     });

//     // If any one of the tasks exit, abort the other.
//     tokio::select! {
//         _ = (&mut send_task) => recv_task.abort(),
//         _ = (&mut recv_task) => send_task.abort(),
//     };

//     // Send user left message.
//     let msg = format!("{} left.", username);
//     tracing::debug!("{}", msg);
//     let _ = state.tx.send(msg);
//     // Remove username from map so new clients can take it.
//     state.user_set.lock().unwrap().remove(&username);
// }


// async fn parse_req( 
//     Json(payload): Json<Answer>,
// ) -> impl IntoResponse {
//     let req_ans= Answer{
//         ans: payload.ans,
//         correct: payload.correct
//     };
//     (StatusCode::CREATED, Json(req_ans))
// } 

// middleware that shows how to consume the request body upfront
// async fn print_request_body(
//     request: Request<BoxBody>,
//     next: Next<BoxBody>,
// ) -> Result<impl IntoResponse, Response> {
//     let request = buffer_request_body(request).await?;

//     Ok(next.run(request).await)
// }



fn check_start(state: &AppState) -> bool{
    let mut user_set = state.user_set.lock().unwrap();
    let mut ready = state.count_ready.lock().unwrap();
    *ready += 1;
    user_set.len()  <= *ready as usize
}

fn check_username(state: &AppState, string: &mut String, name: &str) {
    let mut user_set = state.user_set.lock().unwrap();

    if !user_set.contains(name) {
        user_set.insert(name.to_owned());

        string.push_str(name);
    }
}

// Include utf-8 file at **compile** time.
async fn index() -> Html<&'static str> {
    Html(std::include_str!("../assets/chat.html"))
}
