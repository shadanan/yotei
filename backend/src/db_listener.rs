use std::fmt::Debug;

use serde::Deserialize;
use serde::Serialize;

use sqlx::postgres::PgListener;
use sqlx::{Pool, Postgres};

#[derive(Deserialize, Serialize, Debug)]
pub enum Action {
    INSERT,
    UPDATE,
    DELETE,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Payload {
    pub timestamp: String,
    pub table: String,
    pub action: Action,
    pub id: String,
    pub record: String,
    pub old: Option<String>,
}

use futures::stream::Stream;

pub fn start_listening(
    pool: &Pool<Postgres>,
) -> impl Stream<Item = Result<Payload, sqlx::Error>> + '_ {
    let channels: Vec<&str> = vec!["table_update"];
    tracing::debug!("Setting up DB listeners on channels {:?}..", channels);

    use async_stream::try_stream;

    try_stream! {
        tracing::debug!("Creating listenerrrrs..");
        let mut listener: PgListener = PgListener::connect_with(pool).await.unwrap();
        listener.listen_all(channels).await.unwrap();

        tracing::debug!("Waiting for DB notification..");
        loop {
            match listener.try_recv().await? {
                Some(notification) => {
                    tracing::debug!("Yielding notification {:#?}", &notification);
                    match serde_json::from_str::<Payload>(notification.payload()) {
                        Ok(payload) => yield payload,
                        Err(e) => tracing::warn!("Failed to parse payload: {}", e),
                    };
                },
                None => {
                    tracing::debug!("Got None from listener, connection lost; will retry");
                    continue;
                },
            }
        }
    }
}
