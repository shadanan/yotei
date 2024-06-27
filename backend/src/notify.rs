use async_stream::try_stream;
use axum::extract::ws::{Message, WebSocket};
use futures::stream::Stream;
use sqlx::{postgres::PgListener, Pool, Postgres};
use std::{borrow::BorrowMut, fmt::Debug, net::SocketAddr, sync::Arc};
use tokio::sync::Mutex;

pub fn start_notifications(
    pool: sqlx::Pool<sqlx::Postgres>,
) -> (ChangeRouter, tokio::task::JoinHandle<()>) {
    // Create a stream of task notifications and a router to fan them out to client routes.
    let change_router = ChangeRouter {
        routes: Arc::new(Mutex::new(Vec::new())),
        new_routes: Arc::new(Mutex::new(Vec::new())),
    };
    let notify_task = tokio::spawn(
        change_router
            .clone()
            .route_from_stream(stream_task_notifications(pool)),
    );
    (change_router, notify_task)
}

#[derive(Clone)]
pub struct ChangeRouter {
    /// Existing routes to notify.
    pub routes: Arc<Mutex<Vec<ClientRoute>>>,
    /// Newly added routes that will be moved to `routes`
    /// when the next notification is processed.
    pub new_routes: Arc<Mutex<Vec<ClientRoute>>>,
}

#[derive(Debug)]
pub struct ClientRoute {
    pub sink: futures::stream::SplitSink<WebSocket, Message>,
    pub who: String,
}

impl ChangeRouter {
    pub async fn add_client(self, socket: WebSocket, who: SocketAddr) {
        use futures::stream::StreamExt;
        let (sender, mut receiver) = socket.split();

        let who = who.to_string();
        // Store the sender side of the socket in the list of client routes.
        tracing::debug!("Adding route for client {who}");
        self.new_routes.lock().await.push(ClientRoute {
            sink: sender,
            who: who.clone(),
        });

        // Listen for messages on the read side of the socket.
        // We don't currently expect any messages other than closures.
        // The idea is to proactively remove clients on closure rather
        // than only sweeping them out while processing a notification.
        tokio::spawn(async move {
            while let Some(Ok(msg)) = receiver.next().await {
                match msg {
                    Message::Close(c) => {
                        if let Some(cf) = c {
                            tracing::debug!(
                                ">>> {who} sent close with code {} and reason `{}`",
                                cf.code,
                                cf.reason
                            );
                        } else {
                            tracing::debug!(
                                ">>> {who} somehow sent close message without CloseFrame"
                            );
                        }
                        // TODO: Close and remove the client route from the change_router.
                        break;
                    }
                    _ => {
                        tracing::debug!(">>> {who} sent unsolicited message: {msg:?}");
                    }
                }
            }
        });
    }

    pub async fn route_from_stream(
        self,
        change_stream: impl Stream<Item = Result<Payload, sqlx::Error>>,
    ) {
        use futures::StreamExt;
        futures::pin_mut!(change_stream);

        loop {
            match change_stream.next().await {
                Some(Ok(payload)) => {
                    // First, move any new routes into the routes list.
                    // Doing this reduces contention while inserting new routes
                    // and fanning out to all routes below.
                    self.routes
                        .lock()
                        .await
                        .append(self.new_routes.lock().await.borrow_mut());

                    let payload = Message::Text(serde_json::to_string(&payload).unwrap());
                    let mut routes = self.routes.lock().await;
                    tracing::debug!(
                        "Notifying {} clients ({}) with: {:?}",
                        routes.len(),
                        routes
                            .iter()
                            .map(|cr| &*cr.who)
                            .collect::<Vec<&str>>()
                            .join(", "),
                        payload
                    );

                    // Route the notification to all clients.
                    // Along the way, prune any dead clients.
                    // TODO: Doing this serially won't scale to many clients.
                    let mut i = 0;
                    while i < routes.len() {
                        let client_route = routes[i].borrow_mut();

                        tracing::debug!("Notifying client {}", client_route.who);
                        use futures::sink::SinkExt;
                        match client_route.sink.send(payload.clone()).await {
                            Ok(_) => {
                                tracing::debug!("Sent to {}", client_route.who);
                                i += 1;
                            }
                            Err(e) => {
                                tracing::debug!(
                                    "Failed to send to {}, removing route: {}",
                                    client_route.who,
                                    e
                                );
                                // TODO: Only remove the route when the error indicates closure.
                                let _ = client_route.sink.close().await;
                                routes.remove(i);
                            }
                        }
                    }
                }
                None => {
                    tracing::debug!("Got None from notify stream; continuing");
                    continue;
                }
                Some(Err(e)) => {
                    // TODO: Make sure all errors should be fatal and prevent the notifier from running further.
                    // On termination, the error `attempted to acquire a connection on a closed pool` is seen here.
                    tracing::debug!("Finishing notifier with error: {}", e);
                    break;
                }
            }
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub enum Action {
    INSERT,
    UPDATE,
    DELETE,
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub struct Payload {
    pub timestamp: String,
    pub table: String,
    pub action: Action,
    pub id: String,
    // TODO: Both record and old are json marshalled by the database.
    // It'd be good to massage this into our domain objects here
    // rather than rely on database entirely.
    pub record: String,
    pub old: Option<String>,
}

/// Creates a stream of insert, update and delete task notifications.
pub fn stream_task_notifications(
    pool: Pool<Postgres>,
) -> impl Stream<Item = Result<Payload, sqlx::Error>> {
    let channels: Vec<&str> = vec!["table_update"];

    try_stream! {
        tracing::debug!("Setting up DB listeners on channels {:?}..", channels);
        let mut listener: PgListener = PgListener::connect_with(&pool).await.unwrap();
        listener.listen_all(channels).await.unwrap();

        loop {
            match listener.try_recv().await? {
                Some(notification) => {
                    tracing::debug!("Yielding notification {:?}", &notification);
                    match serde_json::from_str::<Payload>(notification.payload()) {
                        Ok(payload) => yield payload,
                        Err(e) => tracing::warn!("Discarding unparseable notification ({:?}) due to parse error: {}", notification, e ),
                    };
                },
                None => {
                    tracing::debug!("Notification listener lost database connection. Some notifications may be lost. Reconnecting...");
                    continue;
                },
            }
        }
    }
}
