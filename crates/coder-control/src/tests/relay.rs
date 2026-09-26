//! Actual authenticated WebSocket fixture, not the production relay.
use futures_util::{SinkExt, StreamExt};
use nostr::domain::Event;
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{Mutex, broadcast},
};
use tokio_tungstenite::{
    accept_async_with_config,
    tungstenite::{Message, protocol::WebSocketConfig},
};

type Events = Arc<Mutex<BTreeMap<String, Event>>>;
fn visible(event: &Event, principal: &str) -> bool {
    event.pubkey == principal || event.tag_values("p").any(|p| p == principal)
}
fn matches(event: &Event, filter: &Value) -> bool {
    let member = |key: &str, value: Value| {
        filter[key]
            .as_array()
            .is_none_or(|items| items.contains(&value))
    };
    member("ids", json!(event.id))
        && member("authors", json!(event.pubkey))
        && member("kinds", json!(event.kind))
        && ["p", "e"].iter().all(|key| {
            filter[format!("#{key}")]
                .as_array()
                .is_none_or(|wanted| event.tag_values(key).any(|v| wanted.contains(&json!(v))))
        })
}
pub async fn start() -> (String, tokio::task::JoinHandle<()>, Events) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    let events: Events = Arc::new(Mutex::new(BTreeMap::new()));
    let keep = events.clone();
    let address = url.clone();
    let (sender, _) = broadcast::channel(64);
    let handle = tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let url = address.clone();
            let events = keep.clone();
            let sender = sender.clone();
            tokio::spawn(async move {
                let _ = serve(stream, &url, events, sender).await;
            });
        }
    });
    (url, handle, events)
}
async fn serve(
    stream: TcpStream,
    url: &str,
    events: Events,
    sender: broadcast::Sender<Event>,
) -> Result<(), String> {
    let mut socket = accept_async_with_config(
        stream,
        Some(WebSocketConfig::default().max_message_size(Some(1024 * 1024))),
    )
    .await
    .map_err(|e| e.to_string())?;
    let challenge = secp256k1::rand::random::<u128>().to_string();
    socket
        .send(Message::Text(json!(["AUTH", challenge]).to_string().into()))
        .await
        .map_err(|e| e.to_string())?;
    let mut principal = None::<String>;
    let mut subscriptions = BTreeMap::<String, Value>::new();
    let mut broadcast = sender.subscribe();
    loop {
        tokio::select! {
            received=broadcast.recv()=>{
                let event=received.map_err(|e|e.to_string())?;
                if principal.as_deref().is_some_and(|p|visible(&event,p)){
                    for (id,filter) in &subscriptions{
                        if matches(&event,filter){socket.send(Message::Text(json!(["EVENT",id,event]).to_string().into())).await.map_err(|e|e.to_string())?;}
                    }
                }
            },
            frame=socket.next()=>{
                let text=match frame{Some(Ok(Message::Text(text)))=>text,Some(Ok(Message::Close(_)))|None=>return Ok(()),_=>return Err("unsupported relay fixture frame".into())};
                let value=nostr::contracts::parse_strict_bounded(text.as_bytes(),1024*1024).map_err(|e|e.to_string())?;
                let response=match value[0].as_str(){
                    Some("AUTH")=>{
                        let e:Event=serde_json::from_value(value[1].clone()).map_err(|e|e.to_string())?;
                        let now=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
                        let good=e.kind==22242 && e.validate_crypto().is_ok() && e.content.is_empty() && e.tags.len()==2
                            && e.tag_values("relay").collect::<Vec<_>>()==[url] && e.tag_values("challenge").collect::<Vec<_>>()==[challenge.as_str()] && e.created_at.abs_diff(now)<=60;
                        if good{principal=Some(e.pubkey.clone());}json!(["OK",e.id,good,if good{""}else{"auth-required: invalid proof"}])
                    },
                    Some("EVENT")=>{
                        let e:Event=serde_json::from_value(value[1].clone()).map_err(|e|e.to_string())?;
                        let good=principal.as_ref()==Some(&e.pubkey) && e.validate_crypto().is_ok() && match e.kind{
                            3188=>nostr::private_artifact::admit(&e).is_ok(),25920|26920|27020=>e.tag_values("p").count()==1,_=>false};
                        if good {events.lock().await.entry(e.id.clone()).or_insert(e.clone());let _=sender.send(e.clone());}
                        json!(["OK",e.id,good,if good{""}else{"restricted: fixture event refused"}])
                    },
                    Some("REQ")=>{
                        let id=value[1].as_str().ok_or("subscription id")?.to_string();
                        let Some(reader)=&principal else{socket.send(Message::Text(json!(["CLOSED",id,"auth-required: authenticate"]).to_string().into())).await.map_err(|e|e.to_string())?;continue;};
                        let filter=value[2].clone();subscriptions.insert(id.clone(),filter.clone());
                        if filter["limit"]!=0{
                            let stored=events.lock().await;
                            for event in stored.values().filter(|e|e.kind==3188 && visible(e,reader) && matches(e,&filter)){
                                socket.send(Message::Text(json!(["EVENT",id,event]).to_string().into())).await.map_err(|e|e.to_string())?;
                            }
                        }
                        json!(["EOSE",id])
                    },
                    Some("CLOSE")=>{subscriptions.remove(value[1].as_str().ok_or("subscription id")?);continue;},
                    _=>return Err("unsupported relay fixture message".into()),
                };
                socket.send(Message::Text(response.to_string().into())).await.map_err(|e|e.to_string())?;
            }
        }
    }
}
