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
struct AppState {
    user_set: Mutex<HashSet<String>>,
    tx: broadcast::Sender<String>,
    qa_list: Vec<ShrekQA>,
    count_ready: Mutex<u32>
}



struct AppStateAll {
    rooms: Vec<Arc<AppState>>
}


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
            std::env::var("RUST_LOG").unwrap_or_else(|_| "example_chat=trace".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();
    
    let app_state_all = Arc::new(AppStateController{
        rooms: RwLock::new( HashMap::new())
    });

    let app = Router::with_state(app_state_all)
        .route("/", get(index))
        // .route("/create_room", get(create_room))
        .route("/websocket/:id", get(join_room))
        .route("/:id", get(check_room));
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


async fn join_room(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppStateController>>,
    Path(rid): Path<String>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| websocket(socket, state, rid))
}

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
            let help = RoomCheck{joinable: true, room: rid};
            return (StatusCode::NOT_FOUND, Json(help))
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct RoomId{
    rid: u32
}

async fn websocket(stream: WebSocket, state: Arc<AppStateController>, rid: String) {

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

            {
                // let state_clone = state.clone();
                let rid_clone = rid.clone();
                let mut rooms = state.rooms.write().await;
                let room = rooms.entry(rid_clone).or_insert(RoomState::new());
                // room_state = *room;
                tx = Some(room.tx.clone());
                if !room.user_set.contains(&name) {
                    room.user_set.insert(name.to_owned());
                    username = name.clone();
                }
            }
            // If not empty we want to quit the loop else we want to quit function.
            if tx.is_some() && !username.is_empty() {
   
                break;
            } else {
                // Only send our client that username is taken.
                let _ = sender
                    .send(Message::Text(String::from("Username already taken, or sender is not Some")))
                    .await;

                return;
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


    // let mut app_state = state.;

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
    // let my_rid = Arc::new(rid.clone());
    // let app
    // We need to access the `tx` variable directly again, so we can't shadow it here.
    // I moved the task spawning into a new block so the original `tx` is still visible later.
    let mut recv_task = {
        // Clone things we want to pass to the receiving task.
        let tx = tx.clone();
        let mut name = username.clone();
        // let state_clone = state.clone();

        // This task will receive messages from client and send them to broadcast subscribers.
        tokio::spawn(async move {
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
                           
                            let more_rid = &my_rid;
                            let mut rooms = state.rooms.write().await;
                            let mut room = rooms.entry(more_rid.to_string()).or_insert(RoomState::new());
                            // let c = check_start(room, &mut name);
                            if !room.count_ready.contains(&name) {
                                room.count_ready.insert(name.to_owned());
                                // username = name.clone();
                            }
                            if room.count_ready.len()>=room.user_set.len(){
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
                    // _ if socket_message.sm_type.equals("message") => print!("message"),
                    _ => print!("none")
                };
            }
        })
    };

    // If any one of the tasks exit, abort the other.
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => {
            let msg = format!("{} left.", username);
            tracing::debug!("{}", msg);
            let _ = tx.send(msg);
            // let more_rid = my_rid;
            // let mut rooms =  state.rooms.write().await;
            // let mut room = rooms.entry(rid.clone()).or_insert(RoomState::new());
            // room.
            send_task.abort()

        }
    };


    // Send user left message.
    let msg = format!("{} left.", username);
    tracing::debug!("{}", msg);
    let _ = tx.send(msg);

    
    
    
    

    // Remove username from map so new clients can take it.
    // rooms.get_mut(&channel).unwrap().user_set.remove(&username);

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




