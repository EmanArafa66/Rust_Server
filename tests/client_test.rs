use embedded_recruitment_task::{
    message::{client_message, server_message, AddRequest, ClientMessage, EchoMessage},
    server::Server,
};
use std::{
    io,
    net::TcpStream,
    sync::Arc,
    thread::{self, JoinHandle},
    time::Duration,
};

mod client;

// Spawns a new thread to run the server and returns the thread handle
fn setup_server_thread(server: Arc<Server>) -> JoinHandle<()> {
    thread::spawn(move || {
        server.run().expect("Server encountered an error");
    })
}

// Creates and initializes a new server instance
pub fn create_server() -> Arc<Server> {
    let server = Server::new().expect("Failed to start server");
    let port = server.get_port().expect("Failed to retrieve server port");
    println!("Server created on port {}", port);
    Arc::new(server)
}

// Sends a message to the server and verifies the response
fn send_and_receive_message(
    client: &mut client::Client,
    message: embedded_recruitment_task::message::client_message::Message,
    expected_content: Option<impl Into<String>>,
) {
    let client_message = ClientMessage {
        message: Some(message),
    };

    // Send the message to the server
    assert!(client.send(client_message).is_ok(), "Failed to send message");

    // Receive and validate the response
    let response = client.receive();
    assert!(response.is_ok(), "Failed to receive response for message");

    match response.unwrap().message {
        Some(server_message::Message::EchoMessage(echo)) => {
            if let Some(content) = expected_content {
                assert_eq!(
                    echo.content, content.into(),
                    "Echoed message content does not match"
                );
            }
        }
        Some(server_message::Message::AddResponse(add_response)) => {
            if let Some(expected) = expected_content {
                let expected_add = expected.into();
                assert_eq!(
                    add_response.result,
                    expected_add.parse::<i32>().unwrap(),
                    "AddResponse result does not match"
                );
            }
        }
        _ => panic!("Unexpected message received from the server"),
    }
}

// Waits for the server to start by trying to connect multiple times
fn wait_for_server(server_port: u16, max_retries: u32) -> bool {
    let mut retries = 0;
    while retries < max_retries {
        match TcpStream::connect(("127.0.0.1", server_port)) {
            Ok(_) => {
                println!("Server is ready on port {}", server_port);
                return true;
            }
            Err(e) if e.kind() == io::ErrorKind::ConnectionRefused => {
                retries += 1;
                println!(
                    "Server not ready, retrying {}/{}...",
                    retries, max_retries
                );
                std::thread::sleep(Duration::from_millis(500));
            }
            Err(e) => {
                println!(
                    "Unexpected error waiting for server on port {}: {}",
                    server_port, e
                );
                return false;
            }
        }
    }
    println!("Server did not start after {} retries", max_retries);
    false
}

// Finds an available port by binding to an ephemeral port and returning it
fn find_available_port() -> u16 {
    use std::net::TcpListener;
    TcpListener::bind("127.0.0.1:0")
        .expect("Failed to bind to find an available port")
        .local_addr()
        .unwrap()
        .port()
}

// Creates a server instance bound to a specific port
fn create_server_with_port(port: u16) -> Arc<Server> {
    let server = Server::new_with_port(port).expect("Failed to start server on the specified port");
    Arc::new(server)
}

// Test: Connect and disconnect a client to ensure basic connectivity
#[test]
fn test_client_connect_disconnect() {
    let port = find_available_port();
    let server = create_server_with_port(port);
    let handle = setup_server_thread(server.clone());

    assert!(wait_for_server(port, 20), "Server did not start in time");

    let mut client = client::Client::new("127.0.0.1", port.into(), 5000);
    assert!(client.connect().is_ok(), "Failed to connect to server");
    assert!(client.disconnect().is_ok(), "Failed to disconnect from server");

    server.stop();
    handle.join().unwrap();
}

// Test: Send an echo message and verify the response
#[test]
fn test_client_echo_message() {
    let port = find_available_port();
    let server = create_server_with_port(port);
    let handle = setup_server_thread(server.clone());

    assert!(wait_for_server(port, 20), "Server did not start in time");

    let mut client = client::Client::new("127.0.0.1", port.into(), 10000);
    assert!(client.connect().is_ok(), "Failed to connect to the server");

    let mut echo_message = EchoMessage::default();
    echo_message.content = "Hello, World!".to_string();
    let message = client_message::Message::EchoMessage(echo_message.clone());

    send_and_receive_message(
        &mut client,
        message,
        Some(echo_message.content.clone()),
    );

    assert!(client.disconnect().is_ok(), "Failed to disconnect from the server");
    server.stop();
    handle.join().unwrap();
}

// Test: Send multiple echo messages and verify each response
#[test]
fn test_multiple_echo_messages() {
    let port = find_available_port();
    let server = create_server_with_port(port);
    let handle = setup_server_thread(server.clone());

    assert!(wait_for_server(port, 20), "Server did not start in time");

    let mut client = client::Client::new("127.0.0.1", port.into(), 10000);
    assert!(client.connect().is_ok(), "Failed to connect to the server");

    let messages = vec![
        "Hello, World!".to_string(),
        "How are you?".to_string(),
        "Goodbye!".to_string(),
    ];

    for message_content in messages {
        let mut echo_message = EchoMessage::default();
        echo_message.content = message_content.clone();
        let message = client_message::Message::EchoMessage(echo_message);

        send_and_receive_message(&mut client, message, Some(message_content.clone()));
    }

    assert!(client.disconnect().is_ok(), "Failed to disconnect from the server");
    server.stop();
    handle.join().unwrap();
}

// Test: Handle multiple clients concurrently
#[test]
fn test_multiple_clients() {
    let timeout = Duration::from_secs(30);
    let now = std::time::Instant::now();
    let port = find_available_port();
    let server = create_server_with_port(port);
    let handle = setup_server_thread(server.clone());

    println!("Starting server on port {}", port);
    assert!(wait_for_server(port, 20), "Server did not start in time");

    let mut clients = vec![
        client::Client::new("127.0.0.1", port.into(), 10000),
        client::Client::new("127.0.0.1", port.into(), 10000),
        client::Client::new("127.0.0.1", port.into(), 10000),
    ];

    println!("Connecting clients...");
    for (index, client) in clients.iter_mut().enumerate() {
        assert!(client.connect().is_ok(), "Client {} failed to connect to the server", index + 1);
        println!("Client {} connected", index + 1);
    }

    let messages = vec![
        "Hello, World!".to_string(),
        "How are you?".to_string(),
        "Goodbye!".to_string(),
    ];

    for (msg_index, message_content) in messages.iter().enumerate() {
        let mut echo_message = EchoMessage::default();
        echo_message.content = message_content.clone();
        let message = client_message::Message::EchoMessage(echo_message.clone());

        println!("Broadcasting message {} to all clients: {:?}", msg_index + 1, message_content);

        for (client_index, client) in clients.iter_mut().enumerate() {
            println!("Sending to Client {}: {:?}", client_index + 1, message_content);
            send_and_receive_message(client, message.clone(), Some(message_content.clone()));
            println!("Client {} successfully received the message", client_index + 1);
        }
    }

    println!("Disconnecting clients...");
    for (index, client) in clients.iter_mut().enumerate() {
        assert!(client.disconnect().is_ok(), "Client {} failed to disconnect from the server", index + 1);
        println!("Client {} disconnected", index + 1);
    }

    println!("Stopping server...");
    server.stop();
    assert!(handle.join().is_ok(), "Server thread panicked or failed to join");
    println!("Server stopped successfully");
    assert!(now.elapsed() < timeout, "Test timed out");
}

// Test: Verify AddRequest message functionality
#[test]
fn test_client_add_request() {
    let port = find_available_port();
    let server = create_server_with_port(port);
    let handle = setup_server_thread(server.clone());

    assert!(wait_for_server(port, 20), "Server did not start in time");

    let mut client = client::Client::new("127.0.0.1", port.into(), 10000);
    assert!(client.connect().is_ok(), "Failed to connect to the server");

    let add_request = AddRequest { a: 10, b: 20 };
    let message = ClientMessage {
        message: Some(client_message::Message::AddRequest(add_request.clone())),
    };

    println!("Sending AddRequest: {:?}", add_request);
    assert!(client.send(message).is_ok(), "Failed to send message");

    let response = client.receive();
    println!("Received response: {:?}", response);
    assert!(response.is_ok(), "Failed to receive response for AddRequest");

    match response.unwrap().message {
        Some(server_message::Message::AddResponse(add_response)) => {
            println!("AddResponse: {:?}", add_response);
            assert_eq!(
                add_response.result,
                add_request.a + add_request.b,
                "AddResponse result does not match"
            );
        }
        _ => panic!("Expected AddResponse, but received a different message"),
    }

    assert!(client.disconnect().is_ok(), "Failed to disconnect from the server");
    server.stop();
    handle.join().unwrap();
}
