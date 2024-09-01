use std::{mem, thread, time::Duration};

use futures::{pin_mut, Stream};
use tokio::{sync::broadcast::Sender, time::sleep};
use tokio_stream::StreamExt;
use tracing::{debug, error, info, trace};

use crate::{LiveEvent, LiveState};

use client;
use data::{compression, merge::merge, transformer};

pub fn manage(tx: Sender<LiveEvent>, state: LiveState) {
    thread::spawn(|| {
        let rt = tokio::runtime::Runtime::new().unwrap();

        rt.block_on(async {
            keep_client_alive(tx, state).await;
        })
    });
}

async fn keep_client_alive(tx: Sender<LiveEvent>, state: LiveState) {
    loop {
        // Check if there are enough connections before proceeding
        if tx.receiver_count() < 2 {
            debug!("No connections yet, waiting...");
            sleep(Duration::from_secs(5)).await;
            continue;
        }

        info!("Starting client...");

        // Attempt to initialize the client
        let stream = match client::init().await {
            Ok(stream) => stream,
            Err(e) => {
                error!("Client setup failed, retrying in 5 seconds: {}", e);
                sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        // Parse the stream from the client
        let parsed_stream = client::parse_stream(stream).await;

        info!("Handling the client stream...");
        handle_stream(parsed_stream, tx.clone(), state.clone()).await;

        // Add log to indicate a restart
        info!("Client stream handling ended, restarting in 5 seconds...");
        sleep(Duration::from_secs(5)).await; // Pause to avoid immediate loop cycling
    }
}

async fn handle_stream(
    stream: impl Stream<Item = client::message::Message>,
    tx: Sender<LiveEvent>,
    state: LiveState,
) {
    pin_mut!(stream);

    // Iterate over the incoming messages from the stream
    while let Some(message) = stream.next().await {
        match message {
            client::message::Message::Updates(mut updates) => {
                trace!("Received update message");

                // Lock the shared state for updating
                let mut state = state.lock().unwrap();

                for update in updates.iter_mut() {
                    let update = transformer::transform_map(update);

                    // Check if the session name has changed
                    if let Some(new_session_name) = update.pointer("/sessionInfo/name") {
                        let current_session_name = state
                            .pointer("/sessionInfo/name")
                            .expect("Session name should always be present");

                        // Restart the client if the session name has changed
                        if new_session_name != current_session_name {
                            info!(
                                "Session name changed from {:?} to {:?}, restarting client",
                                current_session_name, new_session_name
                            );
                            return; // Exit to restart the client loop
                        }
                    }

                    // Compress the update
                    let Some(update_compressed) = compression::deflate(update.to_string()) else {
                        error!("Failed compressing update");
                        continue;
                    };

                    trace!("Compressed update='{}'", update_compressed);

                    // Send the compressed update
                    match tx.send(LiveEvent::Update(update_compressed)) {
                        Ok(_) => trace!("Update sent successfully"),
                        Err(e) => error!("Failed to send update: {}", e),
                    };

                    // Merge the update into the current state
                    merge(&mut state, update);
                }

                mem::drop(state); // Explicitly drop the lock
            }
            client::message::Message::Initial(mut initial) => {
                trace!("Received initial message");

                // Transform the initial state
                transformer::transform(&mut initial);

                // Lock and update the state with the initial message
                let mut state = state.lock().unwrap();
                *state = initial.clone();
                mem::drop(state);

                // Compress the initial state
                let Some(initial_compressed) = compression::deflate(initial.to_string()) else {
                    error!("Failed compressing initial message");
                    continue;
                };

                trace!("Compressed initial='{}'", initial_compressed);

                // Send the initial state
                match tx.send(LiveEvent::Initial(initial_compressed)) {
                    Ok(_) => trace!("Initial message sent successfully"),
                    Err(e) => error!("Failed to send initial message: {}", e),
                };
            }
        }
    }
}
