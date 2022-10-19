// I have no clue what i am doing
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

// use fmt::{Display};
use serde_json::{self, Value};
use serde::{Serialize, Deserialize};


use futures::{sink::SinkExt, stream::StreamExt};

use std::{
    fmt,
    collections::{HashMap, HashSet},
    net::SocketAddr,
    sync::{Arc, Mutex}, ops::Shr, os::macos::raw::stat,
};
use tokio::sync::{broadcast, RwLock};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use rand::{self, thread_rng};

// Our shared state

struct RoomState{
    user_set: HashSet<String>,
    tx: broadcast::Sender<String>,
    qa_list: Vec<ShrekQA>,
    count_ready: HashSet<String>
}

impl RoomState {
    fn new() -> Self{
        Self{
            user_set: HashSet::new(),
            tx: broadcast::channel(100).0,
            qa_list: extract_csv().unwrap(),
            count_ready: HashSet::new()
        }
    }
}
struct AppStateController{
    rooms: RwLock<HashMap<String, RoomState>>
}

impl AppStateController{

    fn new()->Self{

        let hm = RwLock::new(HashMap::from([("1".to_owned(), RoomState::new())]));
        Self{
            rooms: hm
        }
    }
}


#[derive(Debug)]
enum SMType{
    Message,
    Answer, //could use custom enums here
    Ready,
    QA_list,
    Join
}

impl fmt::Display for SMType{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
        // or, alternatively:
        // fmt::Debug::fmt(self, f)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct SocketMessage{
    sm_type: String,
    username: String,
    payload: String
}

#[tokio::main]
async fn main() {

    // yoyo();
    // let qalist = make_list();
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "my_chat=trace".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let app = Router::with_state(Arc::new(AppStateController::new()))
        .route("/", get(index))
        // .route("/create_room", get(create_room))
        .route("/websocket/:id", get(join_room))
        .route("/:id", get(check_room));
        // .route("/websocket", get(websocket_handler));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    print!("fuck");
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

// #[debug_handler]
// async fn create_room(
//     State(asa): State<Arc<Mutex<AppStateAll>>>
// ) -> Result <(), &'static str>{
//     let user_set = Mutex::new(HashSet::new());
//     let (tx, _rx) = broadcast::channel(100);
    
//     let mut question_list: Vec<ShrekQA> = vec![];
//     if let Ok(ql) = extract_csv(){
//         question_list = ql;
//     }
//     let app_state = Arc::new(AppState { 
//         user_set, 
//         tx, 
//         qa_list: question_list, 
//         count_ready: Mutex::new(0)
//     });
//     let mut asa_lock = asa.lock().unwrap();
//     asa_lock.rooms.push(app_state);
//     tracing::debug!("New room created! #{}", asa_lock.rooms.len());

//     // asa.loc .rooms.push(app_state);
//     Ok(())
//     // ws.on_upgrade(|socket| websocket(socket, state))
// }


async fn join_room(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppStateController>>,
    Path(rid): Path<String>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| websocket(socket, state, rid))
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct RoomCheck{
    joinable: bool,
    room: String
}

async fn check_room(
    Path(rid): Path<String>,
    State(state): State<Arc<AppStateController>>
) -> impl IntoResponse {
    let state_clone = state.clone();
    let rooms = state_clone.rooms.read().await;
    match rooms.contains_key(&rid) {
        true => {
            let help = RoomCheck{joinable: true, room: rid};
            return (StatusCode::FOUND, Json(help))
        },

        _ => {
            let help = RoomCheck{joinable: false, room: rid};
            return (StatusCode::NOT_FOUND, Json(help))
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct RoomId{
    rid: u32
}

async fn websocket(stream: WebSocket, mut state: Arc<AppStateController>, rid: String) {

    tracing::debug!("websocket room {}", &rid);

    let mut username = String::new();
    // By splitting we can send and receive at the same time.
    let (mut sender, mut receiver) = stream.split();
    
    // We have more state now that needs to be pulled out of the connect loop
    let mut tx = None::<broadcast::Sender<String>>;
    let mut username = String::new();
    let mut channel = String::new();

    while let Some(Ok(message)) = receiver.next().await {
        if let Message::Text(name) = message {

            {   //check if the room or user exists before writing 
                let mut rooms = state.rooms.read().await;
                let mut room = match rooms.get(&rid){
              
                    Some(r) => r,
                    None => {
                        let _ = sender
                            .send(Message::Text(format!("room {} not found", &rid)))
                            .await;
                        tracing::debug!("room {} not found", &rid); 
                        return
                    } 
                }; 

                if room.user_set.contains(&name){
                    // Only send our client that username is taken.
                    let _ = sender
                        .send(Message::Text(format!("Username {} already taken, or sender is not Some", &name)))
                        .await;
                    return
                }
                tx = Some(room.tx.clone());

                // drop to unlock 
            }{

                // write to the AppStateController
                let mut write_rooms = state.rooms.write().await;
                let mut write_room = write_rooms
                    .entry(String::from(&rid))
                    .or_insert(RoomState::new());
                let mut write_user_set = &mut write_room.user_set;
                if !write_user_set.contains(&name) {
                    write_user_set.insert(name.to_owned());
                    username = name.clone();
                }
            }
            // If not empty we want to quit the loop else we want to quit function.
            if tx.is_some() && !username.is_empty() { //not necessary
                break;
            } 
        }
    }

    // We know if the loop exited `tx` is not `None`.
    let tx = tx.unwrap();
    // Subscribe before sending joined message.
    let mut rx = tx.subscribe();

    // Send joined message to all subscribers.
    let msg = format!("{} joined.", username);
    tracing::debug!("{}", msg);
    let m = SocketMessage{
        username: username.clone(),
        payload: msg ,
        sm_type: SMType::Join.to_string(),
    };
    let str_mes = serde_json::to_string(&m).unwrap();
    let _ = tx.send(str_mes );

    // This task will receive broadcast messages and send text message to our client.
    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            // In any websocket error, break loop.
            if sender.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
    });
    
    let my_rid= rid.clone();
    let my_rid2 = rid.clone();

    let state_clone1 = Arc::clone(&state);
    // let state_clone2 = state.clone();
    // We need to access the `tx` variable directly again, so we can't shadow it here.
    // I moved the task spawning into a new block so the original `tx` is still visible later.
    let mut recv_task = {
        // Clone things we want to pass to the receiving task.
        let tx = tx.clone();
        let mut name = username.clone();
        // let state_clone = state.clone();

        // This task will receive messages from client and send them to broadcast subscribers.
        tokio::spawn( async move {
            while let Some(Ok(Message::Text(text))) = receiver.next().await {

                let socket_message: SocketMessage= serde_json::from_str(&text).unwrap();
                match &socket_message.sm_type[..] {
                    "message" => {
                        tracing::debug!("mes");
                        let m = SocketMessage{
                            username: name.clone(),
                            payload: format!(": {}", socket_message.payload) ,
                            sm_type: SMType::Message.to_string(),
                        };
                        let str_mes = serde_json::to_string(&m).unwrap();
                        let _ = tx.send(str_mes );
                    },
                    "ready" => {
                        tracing::debug!("{name} is ready");
                           
                            let my_rid = my_rid.clone();
                            let mut rooms = state_clone1.rooms.write().await;
                            let mut room = rooms.entry(my_rid.to_string()).or_insert(RoomState::new());
                            if !room.count_ready.contains(&name) {
                                room.count_ready.insert(name.to_owned());
                            }

                            let us = &mut room.user_set;
                            if room.count_ready.len()>=us.len(){
                                tracing::debug!("Starting game"); 
                                let rsc = room.qa_list.to_vec();
                                let sm = SocketMessageQAList{
                                    payload: rsc,
                                    sm_type: SMType::QA_list.to_string()
                                }; 
                                let _ = tx.send(serde_json::to_string(&sm).unwrap());
                            }       
                    },
                    "answer" => tracing::debug!("ans"),
                    _ => tracing::debug!("none")
                };
            }
        })
    };

    // If any one of the tasks exit, abort the other.
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort()
    };
    let spls = Arc::clone(&state);
    let mut rooms = spls.rooms.write().await;
    let mut room = rooms.entry(my_rid2).or_insert(RoomState::new());
    let mut uset = &mut room.user_set;
    let help = uset.remove(&username);
    
    // Send user left message.
    let msg = format!("{} left.", username);
    tracing::debug!("{}", msg);
    let _ = tx.send(msg);


    // TODO: Check if the room is empty now and remove the `RoomState` from the map.
}


// fn check_start(room: &mut RoomState, user: &mut String) -> bool{
//     let mut users_ready = room.count_ready;
//     if !users_ready.contains(user) {
//         users_ready.insert(format!("user.to_owned()"));
//         // username = name.clone();
//     }
//     // let mut room = rooms.entry(rid.clone()).or_insert(RoomState::new());
//     // if !room.count_ready.contains(&name) {
//     //     room.count_ready.insert(name.to_owned());
//     //     // username = name.clone();
//     // }
//     users_ready.len()>=room.user_set.len()
// }

// fn check_username(state: &AppState, string: &mut String, name: &str) {
//     let mut user_set = state.user_set.lock().unwrap();

//     if !user_set.contains(name) {
//         user_set.insert(name.to_owned());

//         string.push_str(name);
//     }
// }

// Include utf-8 file at **compile** time.
async fn index() -> Html<&'static str> {
    Html(std::include_str!("../assets/chat.html"))
}



