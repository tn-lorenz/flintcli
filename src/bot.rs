use azalea::prelude::*;
use anyhow::Result;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use parking_lot::RwLock;

#[derive(Clone, Component)]
struct State {
    client_handle: Arc<RwLock<Option<Client>>>,
    in_game: Arc<AtomicBool>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            client_handle: Arc::new(RwLock::new(None)),
            in_game: Arc::new(AtomicBool::new(false)),
        }
    }
}

pub struct TestBot {
    client: Option<Arc<RwLock<Option<Client>>>>,
    in_game: Option<Arc<AtomicBool>>,
}

impl TestBot {
    pub fn new() -> Self {
        Self {
            client: None,
            in_game: None,
        }
    }

    pub async fn connect(&mut self, server: &str) -> Result<()> {
        let account = Account::offline("FlintMC_TestBot");

        tracing::info!("Connecting to server: {}", server);

        let state = State::default();
        let client_handle = state.client_handle.clone();
        let in_game = state.in_game.clone();

        // Spawn the bot in a background task
        let server_owned = server.to_string();
        tokio::spawn(async move {
            async fn handler(bot: Client, event: Event, state: State) -> anyhow::Result<()> {
                match event {
                    Event::Init => {
                        *state.client_handle.write() = Some(bot.clone());
                        tracing::info!("Bot initialized");
                    }
                    Event::Login => {
                        // Login event means we're fully in the game state
                        state.in_game.store(true, Ordering::SeqCst);
                        tracing::info!("Bot in game state");
                    }
                    _ => {}
                }
                Ok(())
            }

            let result = ClientBuilder::new()
                .set_handler(handler)
                .set_state(state)
                .start(account, server_owned.as_str())
                .await;

            if let Err(e) = result {
                tracing::error!("Bot connection error: {}", e);
            }
        });

        // Wait for client to initialize
        for _ in 0..50 {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            if client_handle.read().is_some() {
                break;
            }
        }

        if client_handle.read().is_none() {
            anyhow::bail!("Failed to initialize bot connection");
        }

        // Wait for bot to be in game state
        tracing::info!("Waiting for bot to enter game state...");
        for _ in 0..100 {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            if in_game.load(Ordering::SeqCst) {
                break;
            }
        }

        if !in_game.load(Ordering::SeqCst) {
            anyhow::bail!("Bot failed to enter game state within timeout");
        }

        self.client = Some(client_handle);
        self.in_game = Some(in_game);
        tracing::info!("Connected successfully and in game state");

        // Give a small amount of extra time for world data to sync
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        Ok(())
    }

    pub async fn send_command(&self, command: &str) -> Result<()> {
        if let Some(client_handle) = &self.client {
            if let Some(client) = client_handle.read().as_ref() {
                // Add "/" prefix if not present
                let command_with_slash = if command.starts_with('/') {
                    command.to_string()
                } else {
                    format!("/{}", command)
                };
                tracing::debug!("Sending command: {}", command_with_slash);
                client.chat(&command_with_slash);
                Ok(())
            } else {
                anyhow::bail!("Bot not initialized")
            }
        } else {
            anyhow::bail!("Bot not connected")
        }
    }

    pub async fn get_block(&self, pos: [i32; 3]) -> Result<Option<String>> {
        if let Some(client_handle) = &self.client {
            if let Some(client) = client_handle.read().as_ref() {
                let block_pos = azalea::BlockPos::new(pos[0], pos[1], pos[2]);
                let world_lock = client.world();
                let world = world_lock.read();
                let block_state = world.get_block_state(block_pos);

                if let Some(state) = block_state {
                    // Return block state as debug string
                    let state_str = format!("{:?}", state);
                    Ok(Some(state_str))
                } else {
                    Ok(None)
                }
            } else {
                anyhow::bail!("Bot not initialized")
            }
        } else {
            anyhow::bail!("Bot not connected")
        }
    }

    pub async fn get_block_state_property(&self, pos: [i32; 3], property: &str) -> Result<Option<String>> {
        if let Some(client_handle) = &self.client {
            if let Some(client) = client_handle.read().as_ref() {
                let block_pos = azalea::BlockPos::new(pos[0], pos[1], pos[2]);
                let world_lock = client.world();
                let world = world_lock.read();
                let block_state = world.get_block_state(block_pos);

                if let Some(state) = block_state {
                    // For now, return the full state string representation
                    // The property API has changed in newer versions
                    let state_str = format!("{:?}", state);

                    // Simple string matching for common properties
                    if state_str.contains(&format!("{}: ", property)) {
                        // Try to extract the value
                        Ok(Some(state_str))
                    } else {
                        Ok(None)
                    }
                } else {
                    Ok(None)
                }
            } else {
                anyhow::bail!("Bot not initialized")
            }
        } else {
            anyhow::bail!("Bot not connected")
        }
    }
}
