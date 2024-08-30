use futures::stream::StreamExt;
use tokio::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use log::{trace, info, error};
use crate::transformer;
use crate::compression;
use crate::merge;
use crate::LiveEvent;
use crate::LiveState;

async fn process_stream(
    mut stream: impl futures::Stream<Item = client::message::Message> + Unpin,
    tx: Sender<LiveEvent>,
    state: Arc<Mutex<LiveState>>,
) {
    pin_mut!(stream);

    while let Some(message) = stream.next().await {
        match message {
            client::message::Message::Updates(mut updates) => {
                trace!("received update");

                let mut state = state.lock().unwrap();

                for update in updates.iter_mut() {
                    let update = transformer::transform_map(update);

                    if let Some(new_session_name) = update.pointer("/sessionInfo/name") {
                        let current_session_name = state
                            .pointer("/sessionInfo/name")
                            .expect("we always should have a session name");

                        if new_session_name != current_session_name {
                            info!("session name changed, restarting client");
                            return; // Exit the function to restart the stream processing
                        }
                    }

                    let Some(update_compressed) = compression::deflate(update.to_string()) else {
                        error!("failed compressing update");
                        continue;
                    };

                    trace!("update compressed='{}'", update_compressed);

                    match tx.send(LiveEvent::Update(update_compressed)) {
                        Ok(_) => trace!("update sent"),
                        Err(e) => error!("failed sending update: {}", e),
                    };

                    merge(&mut state, update)
                }

                mem::drop(state);
            }
            client::message::Message::Initial(mut initial) => {
                trace!("received initial");
                // Handle initial message
            }
        }
    }
}

async fn start_service(
    stream: impl futures::Stream<Item = client::message::Message> + Unpin,
    tx: Sender<LiveEvent>,
    state: Arc<Mutex<LiveState>>,
) {
    loop {
        process_stream(stream, tx.clone(), state.clone()).await;
        info!("Restarting stream processing due to session change");
    }
}