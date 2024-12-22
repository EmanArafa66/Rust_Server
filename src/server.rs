use crate::message::{client_message, server_message, AddResponse, ClientMessage, EchoMessage, ServerMessage};
use log::{error, info, warn};
use prost::Message;
use std::{
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
use std::sync::atomic::{AtomicBool, Ordering};

// Represents a connected client
struct Client {
    stream: TcpStream,
}

impl Client {
    // Creates a new client instance from a TCP stream
    pub fn new(stream: TcpStream) -> Self {
        Client { stream }
    }

    // Handles communication with the client
    pub fn handle(&mut self, is_running: &Arc<AtomicBool>) -> io::Result<()> {
        let mut buffer = [0; 1024]; // Buffer to store incoming data

        // Continuously read and process data while the server is running
        while is_running.load(Ordering::SeqCst) {
            match self.stream.read(&mut buffer) {
                Ok(bytes_read) if bytes_read == 0 => {
                    // Client disconnected
                    info!("Client disconnected.");
                    break;
                }
                Ok(bytes_read) => {
                    // Successfully read data from the client
                    info!("Received {} bytes from client", bytes_read);

                    // Decode the received message
                    if let Ok(message) = ClientMessage::decode(&buffer[..bytes_read]) {
                        if let Some(payload) = message.message {
                            match payload {
                                // Handle EchoMessage: Respond with the same content
                                client_message::Message::EchoMessage(echo) => {
                                    let response = ServerMessage {
                                        message: Some(server_message::Message::EchoMessage(EchoMessage {
                                            content: echo.content,
                                        })),
                                    };
                                    self.stream.write_all(&response.encode_to_vec())?;
                                    info!("Sent EchoMessage response");
                                }
                                // Handle AddRequest: Respond with the sum of `a` and `b`
                                client_message::Message::AddRequest(add) => {
                                    let response = ServerMessage {
                                        message: Some(server_message::Message::AddResponse(AddResponse {
                                            result: add.a + add.b,
                                        })),
                                    };
                                    self.stream.write_all(&response.encode_to_vec())?;
                                    info!("Sent AddResponse");
                                }
                            }
                        }
                    }
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // Non-blocking mode: No data available, sleep briefly
                    thread::sleep(Duration::from_millis(10));
                }
                Err(e) => {
                    // Error occurred while reading from the client
                    error!("Error reading from client: {}", e);
                    break;
                }
            }
        }
        Ok(())
    }
}

// Represents the server that listens for client connections
pub struct Server {
    listener: TcpListener,                    // Listener for incoming connections
    is_running: Arc<AtomicBool>,              // Atomic flag to track server state
    clients: Arc<Mutex<Vec<TcpStream>>>,      // List of connected clients
}

impl Server {
    // Creates a new server on any available port
    pub fn new() -> Result<Self, io::Error> {
        let port = 0; // Use 0 to let the OS assign an available port
        Self::new_with_port(port)
    }

    // Creates a new server on the specified port
    pub fn new_with_port(port: u16) -> Result<Self, io::Error> {
        let listener = TcpListener::bind(("127.0.0.1", port))?;
        Ok(Self {
            listener,
            is_running: Arc::new(AtomicBool::new(true)),
            clients: Arc::new(Mutex::new(Vec::new())),
        })
    }

    // Starts the server and listens for incoming client connections
    pub fn run(&self) -> io::Result<()> {
        self.is_running.store(true, Ordering::SeqCst); // Mark the server as running

        // Set the listener to non-blocking mode
        self.listener.set_nonblocking(true)?;

        while self.is_running.load(Ordering::SeqCst) {
            match self.listener.accept() {
                Ok((stream, addr)) => {
                    // New client connection accepted
                    info!("New client connected: {}", addr);

                    // Add the client stream to the list of connected clients
                    let mut clients = self.clients.lock().unwrap();
                    clients.push(stream.try_clone()?);
                    drop(clients); // Release the mutex before spawning a thread

                    // Spawn a new thread to handle the client
                    let is_running = Arc::clone(&self.is_running);
                    let _ = thread::spawn(move || {
                        let mut client = Client::new(stream);
                        if let Err(e) = client.handle(&is_running) {
                            error!("Error handling client: {}", e);
                        }
                    });
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // Non-blocking mode: No incoming connections, sleep briefly
                    thread::sleep(Duration::from_millis(100));
                }
                Err(e) => {
                    // Error occurred while accepting a connection
                    warn!("Error accepting connection: {}", e);
                }
            }
        }

        info!("Server stopped.");
        Ok(())
    }

    // Stops the server and disconnects all clients
    pub fn stop(&self) {
        self.is_running.store(false, Ordering::SeqCst); // Mark the server as stopped

        // Disconnect all clients
        for client in self.clients.lock().unwrap().drain(..) {
            if let Err(e) = client.shutdown(std::net::Shutdown::Both) {
                warn!("Error shutting down client: {}", e);
            }
        }

        info!("Server shutting down...");
    }

    // Retrieves the port the server is listening on
    pub fn get_port(&self) -> Result<u16, io::Error> {
        self.listener.local_addr().map(|addr| addr.port())
    }
}
