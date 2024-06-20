use std::fmt::Debug;

use serde::Deserialize;

use serde::de::DeserializeOwned;
use sqlx::error::Error;
use sqlx::postgres::PgListener;
use sqlx::{Pool, Postgres};

#[derive(Deserialize, Debug)]
pub enum ActionType {
    INSERT,
    UPDATE,
    DELETE,
}

#[derive(Deserialize, Debug)]
pub struct Payload {
    pub table: String,
    pub action_type: ActionType,
    pub id: String,
    pub name: String,
}

pub async fn start_listening<T: DeserializeOwned + Sized + Debug>(
    pool: &Pool<Postgres>,
    channels: Vec<&str>,
    call_back: impl Fn(T),
) -> Result<(), Error> {
    tracing::debug!("Setting up DB listeners..");
    let mut listener = PgListener::connect_with(pool).await.unwrap();
    listener.listen_all(channels).await?;
    loop {
        tracing::debug!("Waiting for DB notification..");
        while let Some(notification) = listener.try_recv().await? {
            tracing::debug!("Getting notification {:#?}", notification);

            match serde_json::from_str::<T>(notification.payload()) {
                Ok(payload) => call_back(payload),
                Err(e) => tracing::warn!(
                    "Failed to parse payload: {} from notification {:#?}",
                    e,
                    notification
                ),
            };
        }
    }
}

pub async fn listen_for_notifications(pool: &Pool<Postgres>) -> Result<(), Error> {
    let call_back = |payload: Payload| {
        match payload.action_type {
            ActionType::INSERT => {
                tracing::debug!("Processing insert event for payload '{:#?}'", payload);
            }
            ActionType::UPDATE => {
                tracing::debug!("Processing update event for payload '{:#?}'", payload);
            }
            ActionType::DELETE => {
                tracing::debug!("Processing delete event for payload '{:#?}'", payload);
            }
        };
    };

    let channels = vec!["table_update"];
    let res = start_listening(&pool, channels, call_back).await;
    tracing::debug!("Finished listening with result {:#?}", res);
    return res;
}
