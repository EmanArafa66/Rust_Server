use embedded_recruitment_task::message::{ClientMessage, ServerMessage};
use log::{error, info};
use prost::Message;
use std::{
    io::{self, Read, Write},
    net::{SocketAddr, TcpStream, ToSocketAddrs},
    time::Duration,
};

// Represents a TCP client that communicates with the server
pub struct Client {
    ip: String,             // Server IP address
    port: u32,              // Server port
    timeout: Duration,      // Connection timeout duration
    stream: Option<TcpStream>, // Optional TCP stream for communication
}

impl Client {
    // Creates a new client instance with the specified server details and timeout
    pub fn new(ip: &str, port: u32, timeout_ms: u64) -> Self {
        Client {
            ip: ip.to_string(),
            port,
            timeout: Duration::from_millis(timeout_ms),
            stream: None,
        }
    }

    // Connects to the server with the specified IP and port
    pub fn connect(&mut self) -> io::Result<()> {
        let address = format!("{}:{}", self.ip, self.port); // Combine IP and port into an address string
        println!("Attempting to connect to {}", address);

        // Resolve the server address to a list of socket addresses
        let socket_addrs: Vec<SocketAddr> = address.to_socket_addrs()?.collect();

        if socket_addrs.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid IP or port"));
        }

        // Connect to the first resolved address with a timeout
        let stream = TcpStream::connect_timeout(&socket_addrs[0], self.timeout)?;
        self.stream = Some(stream);

        println!("Connected to server at {}", address);
        Ok(())
    }

    // Sends a message to the server
    pub fn send(&mut self, message: ClientMessage) -> io::Result<()> {
        if let Some(ref mut stream) = self.stream {
            // Serialize the message into a buffer
            let mut buffer = Vec::new();
            message.encode(&mut buffer).map_err(|e| {
                error!("Encoding error: {}", e);
                io::Error::new(io::ErrorKind::InvalidData, "Failed to encode the message")
            })?;

            // Write the serialized message to the TCP stream
            stream.write_all(&buffer)?;
            stream.flush()?; // Ensure the message is fully sent
            info!("Sent message: {:?}", message);
            Ok(())
        } else {
            // No active connection to send the message
            error!("No active connection");
            Err(io::Error::new(io::ErrorKind::NotConnected, "No active connection"))
        }
    }

    // Disconnects from the server by closing the TCP stream
    pub fn disconnect(&mut self) -> Result<(), io::Error> {
        if self.stream.is_some() {
            // Take and drop the TCP stream, effectively disconnecting
            self.stream.take();
            println!("Disconnected from server.");
            Ok(())
        } else {
            // No active connection to disconnect
            error!("Disconnect attempted without an active connection.");
            Err(io::Error::new(io::ErrorKind::NotConnected, "No active connection to disconnect"))
        }
    }

    // Receives a message from the server
    pub fn receive(&mut self) -> io::Result<ServerMessage> {
        if let Some(ref mut stream) = self.stream {
            let mut buffer = vec![0u8; 1024]; // Buffer to store received data
            let bytes_read = stream.read(&mut buffer)?; // Read data from the TCP stream

            if bytes_read == 0 {
                // Server closed the connection
                info!("Server disconnected.");
                return Err(io::Error::new(io::ErrorKind::ConnectionAborted, "Server disconnected"));
            }

            info!("Received {} bytes from server", bytes_read);

            // Deserialize the received data into a ServerMessage
            ServerMessage::decode(&buffer[..bytes_read]).map_err(|e| {
                error!("Failed to decode ServerMessage: {}", e);
                io::Error::new(io::ErrorKind::InvalidData, format!("Failed to decode: {}", e))
            })
        } else {
            // No active connection to receive data
            error!("No active connection");
            Err(io::Error::new(io::ErrorKind::NotConnected, "No active connection"))
        }
    }
}
