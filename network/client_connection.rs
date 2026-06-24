use crate::crypto::compute_shared_secret;
use crate::messages::{ContenuPaquet, Paquet, CURRENT_VERSION};
use crate::network_protocol::{create_codec, EncryptedCodec};
use crate::traits::PacketCodec;
use log::{error, info};
use std::process::exit;
use std::sync::Arc;
use std::time::Instant;
use tokio::io::AsyncReadExt;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;

pub struct ClientConnection {
    runtime: Option<Runtime>,
    read_half: Option<OwnedReadHalf>,
    write_half: Option<OwnedWriteHalf>,
    codec: Arc<EncryptedCodec>,
    player_unique_id: u64,
    server_player_id: u64,
    connected: bool,
    sender: Option<mpsc::UnboundedSender<Paquet>>,
    receiver: Option<mpsc::UnboundedReceiver<Paquet>>,
    last_communication: Instant,
}

impl ClientConnection {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            runtime: None,
            read_half: None,
            write_half: None,
            codec: Arc::new(create_codec([0u8; 32])),
            player_unique_id: 0,
            server_player_id: 0,
            connected: false,
            sender: None,
            receiver: None,
            last_communication: Instant::now(),
        })
    }

    pub const fn is_connected(&self) -> bool {
        self.connected
    }

    pub const fn player_id(&self) -> u64 {
        self.server_player_id
    }

    pub const fn player_unique_id(&self) -> u64 {
        self.player_unique_id
    }

    pub const fn get_last_communication(&self) -> Instant {
        self.last_communication
    }

    pub fn connect(&mut self, server_addr: &str) -> Result<(), String> {
        let runtime = Runtime::new().map_err(|e| e.to_string())?;

        let stream = runtime.block_on(TcpStream::connect(server_addr)).map_err(|e| e.to_string())?;

        self.runtime = Some(runtime);
        self.codec = Arc::new(create_codec([0u8; 32]));

        let (read_half, write_half) = stream.into_split();
        self.read_half = Some(read_half);
        self.write_half = Some(write_half);
        Ok(())
    }

    pub fn perform_handshake(&mut self, username: &str, player_unique_id: u64) -> Result<u32, String> {
        let runtime = self.runtime.as_ref().ok_or("No runtime")?;

        let (is_player_id_correct, server_seed, codec) = runtime.block_on(async {
            self.player_unique_id = player_unique_id;

            let read_half = self.read_half.as_mut().ok_or("No read half")?;
            let write_half = self.write_half.as_mut().ok_or("No write half")?;

            let mut server_id_buf = [0u8; 16];
            read_half.read_exact(&mut server_id_buf).await.map_err(|e| e.to_string())?;

            let codec = Arc::new(create_codec(compute_shared_secret(&server_id_buf, b"server")));

            let packet = Paquet::new(
                crate::messages::TypePaquet::Handshake,
                ContenuPaquet::DonneesConnexion {
                    version: CURRENT_VERSION,
                    username: username.to_string(),
                    player_unique_id: self.player_unique_id,
                },
            );

            codec.send_packet(write_half, &packet).await.map_err(|e| e.to_string())?;

            let confirmation_packet = codec.receive_packet(read_half).await.map_err(|e| e.to_string())?;
            let is_player_id_correct = match confirmation_packet.contenu {
                ContenuPaquet::Confirmation {
                    player_id,
                    is_player_id_correct,
                    ..
                } => {
                    self.server_player_id = player_id;
                    is_player_id_correct
                }
                _ => return Err("Failed to receive confirmation packet.".to_string()),
            };

            let seed_packet = codec.receive_packet(read_half).await.map_err(|e| e.to_string())?;
            let server_seed = match seed_packet.contenu {
                ContenuPaquet::ServerSeed { seed } => seed,
                _ => return Err("Failed to receive server seed.".to_string()),
            };
            info!("Server seed received: {}.", server_seed);

            Ok((is_player_id_correct, server_seed as u32, codec))
        })?;

        if !is_player_id_correct {
            error!("Le player ID n'est pas légitime.");
            exit(0);
        }

        self.codec = codec;
        self.connected = true;

        self.start_sender_task();
        self.start_receiver_task();

        Ok(server_seed)
    }

    fn start_sender_task(&mut self) {
        let write_half = self.write_half.take();
        let codec = self.codec.clone();

        let (tx, mut rx) = mpsc::unbounded_channel();
        self.sender = Some(tx);

        if let Some(runtime) = self.runtime.as_ref() {
            runtime.spawn(async move {
                let mut write_half = write_half.expect("Write half already taken");
                while let Some(packet) = rx.recv().await {
                    if let Err(e) = codec.send_packet(&mut write_half, &packet).await {
                        error!("Failed to send packet: {}", e);
                    }
                }
            });
        }
    }

    fn start_receiver_task(&mut self) {
        let read_half = self.read_half.take();
        let codec = self.codec.clone();

        let (tx, rx) = mpsc::unbounded_channel();
        self.receiver = Some(rx);

        if let Some(runtime) = self.runtime.as_ref() {
            runtime.spawn(async move {
                let mut read_half = read_half.expect("Read half already taken");
                while let Ok(packet) = codec.receive_packet(&mut read_half).await {
                    if tx.send(packet).is_err() {
                        break;
                    }
                }
            });
        }
    }

    pub fn send_packet(&mut self, packet: Paquet) -> Result<(), String> {
        if !self.connected {
            return Err("Not connected".to_string());
        }

        if let Some(sender) = &self.sender {
            sender.send(packet).map_err(|e| e.to_string())?;
        }

        self.last_communication = Instant::now();

        Ok(())
    }

    pub fn receive_packet(&mut self) -> Result<Option<Paquet>, String> {
        if !self.connected {
            return Err("Not connected".to_string());
        }

        if let Some(receiver) = &mut self.receiver {
            match receiver.try_recv() {
                Ok(packet) => {
                    self.last_communication = Instant::now();
                    Ok(Some(packet))
                }
                Err(mpsc::error::TryRecvError::Empty) => Ok(None),
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    self.connected = false;
                    Err("Connection closed by server".to_string())
                }
            }
        } else {
            Err("Receiver not initialized".to_string())
        }
    }
}
