use actix::{Actor, StreamHandler, AsyncContext, ActorContext};
use actix_web_actors::ws;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use std::sync::Arc;

use crate::event::Event;
use crate::subscription::Subscription;
use crate::db::Database;

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(60);

pub struct RelayWs {
    hb: Instant,
    subscriptions: HashMap<String, Subscription>,
    event_tx: broadcast::Sender<Event>,
    event_rx: broadcast::Receiver<Event>,
    db: Arc<Database>,
}

impl RelayWs {
    pub fn new(_id: String, event_tx: broadcast::Sender<Event>, db: Arc<Database>) -> Self {
        Self {
            hb: Instant::now(),
            subscriptions: HashMap::new(),
            event_tx: event_tx.clone(),
            event_rx: event_tx.subscribe(),
            db: db,
        }
    }

    fn hb(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                ctx.stop();
                return;
            }
            ctx.ping(b"");
        });
    }

    fn handle_client_message(&mut self, msg: &str, ctx: &mut ws::WebsocketContext<Self>) {
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(msg);
        
        match parsed {
            Ok(value) => {
                if let Some(array) = value.as_array() {
                    if array.is_empty() {
                        return;
                    }

                    match array[0].as_str() {
                        Some("EVENT") => {
                            if let Ok(event_cmd) = serde_json::from_value(value.clone()) {
                                self.handle_event(event_cmd, ctx);
                            }
                        }
                        Some("REQ") => {
                            if let Ok(sub) = serde_json::from_value(value.clone()) {
                                self.handle_subscription(sub, ctx);
                            }
                        }
                        Some("CLOSE") => {
                            if array.len() >= 2 {
                                if let Some(sub_id) = array[1].as_str() {
                                    self.subscriptions.remove(sub_id);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            Err(_) => {
                ctx.text(r#"["NOTICE", "Invalid message format"]"#);
            }
        }
    }

    fn handle_event(&mut self, event: Event, ctx: &mut ws::WebsocketContext<Self>) {
        match event.validate() {
            Ok(()) => {
                // Save event to database
                let db = self.db.clone();
                let event_clone = event.clone();
                actix::spawn(async move {
                    if let Err(e) = db.save_event(&event_clone).await {
                        log::error!("Failed to save event: {}", e);
                    }
                });

                // Broadcast valid event
                if let Err(e) = self.event_tx.send(event.clone()) {
                    ctx.text(format!(r#"["NOTICE", "Error broadcasting event: {}"]"#, e));
                    return;
                }
                
                // Send OK message
                ctx.text(format!(r#"["OK", "{}", "{}"]"#, event.id, true));
            }
            Err(e) => {
                ctx.text(format!(r#"["OK", "{}", "{}", "{}"]"#, event.id, false, e));
            }
        }
    }

    fn handle_subscription(&mut self, sub: Subscription, ctx: &mut ws::WebsocketContext<Self>) {
        let sub_id = sub.id.clone();
        
        // Query historical events
        let db = self.db.clone();
        let sub_clone = sub.clone();
        let addr = ctx.address();
        
        actix::spawn(async move {
            // Extract filter parameters from subscription
            let ids = sub_clone.filters.iter()
                .filter_map(|f| f.ids.clone())
                .next();
            let authors = sub_clone.filters.iter()
                .filter_map(|f| f.authors.clone())
                .next();
            let kinds = sub_clone.filters.iter()
                .filter_map(|f| f.kinds.clone())
                .next();
            let since = sub_clone.filters.iter()
                .filter_map(|f| f.since)
                .next();
            let until = sub_clone.filters.iter()
                .filter_map(|f| f.until)
                .next();
            let limit = sub_clone.filters.iter()
                .filter_map(|f| f.limit)
                .next();
                
            // Collect tag filters
            let tag_filters = sub_clone.filters.iter()
                .flat_map(|f| f.tags.iter())
                .filter(|(k, _)| k.starts_with('#'))
                .map(|(k, v)| (k.chars().nth(1).unwrap(), v.clone().into_iter().collect()))
                .collect::<Vec<_>>();

            match db.get_events_by_filter(
                &ids, &authors, &kinds, &since, &until, &limit, &tag_filters
            ).await {
                Ok(events) => {
                    for event in events {
                        addr.do_send(event);
                    }
                }
                Err(e) => {
                    log::error!("Failed to query events: {}", e);
                }
            }
        });

        self.subscriptions.insert(sub_id.clone(), sub);
        
        // Send EOSE
        ctx.text(format!(r#"["EOSE", "{}"]"#, sub_id));
    }
}

impl Actor for RelayWs {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.hb(ctx);

        // Set up event receiver
        let addr = ctx.address();
        let mut event_rx = self.event_rx.resubscribe();
        actix::spawn(async move {
            while let Ok(event) = event_rx.recv().await {
                addr.do_send(event);
            }
        });
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for RelayWs {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => {
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            Ok(ws::Message::Pong(_)) => {
                self.hb = Instant::now();
            }
            Ok(ws::Message::Text(text)) => {
                self.handle_client_message(&text, ctx);
            }
            Ok(ws::Message::Close(reason)) => {
                ctx.close(reason);
                ctx.stop();
            }
            _ => {}
        }
    }
}

impl actix::Handler<Event> for RelayWs {
    type Result = ();

    fn handle(&mut self, event: Event, ctx: &mut Self::Context) {
        // Check if any subscription is interested in this event
        for sub in self.subscriptions.values() {
            if sub.interested_in_event(&event) {
                if let Ok(event_json) = serde_json::to_string(&event) {
                    ctx.text(format!(r#"["EVENT", "{}", {}]"#, sub.id, event_json));
                }
            }
        }
    }
}
