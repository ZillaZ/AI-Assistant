use crate::modules::database::types::*;
use crate::modules::web_client::{http::*, types::*};
use std::{
    io::{ErrorKind, Read, Write},
    net::{TcpListener, TcpStream},
    sync::mpsc::{Receiver, Sender},
};

pub struct WebServer {
    sender: Sender<String>,
    network_sender: Sender<NetworkMessage>,
    receiver: Receiver<Receiver<DatabaseMessage>>,
}

impl WebServer {
    pub fn new(
        sender: Sender<String>,
        network_sender: Sender<NetworkMessage>,
        receiver: Receiver<Receiver<DatabaseMessage>>,
    ) -> Self {
        Self {
            sender,
            network_sender,
            receiver,
        }
    }

    pub fn update(&mut self) {
        let listener = TcpListener::bind("127.0.0.1:8080").expect("Unable to start tcp listener");
        while let Ok((stream, addr)) = listener.accept() {
            let _ = self.sender.send(addr.to_string());
            let receiver = self.receiver.recv().unwrap();
            let mut web_connection = WebConnection::new(
                stream,
                addr.to_string(),
                self.network_sender.clone(),
                receiver,
            );
            std::thread::spawn(move || {
                web_connection.update();
            });
        }
    }
}

pub struct WebConnection {
    stream: TcpStream,
    addr: String,
    sender: Sender<NetworkMessage>,
    receiver: Receiver<DatabaseMessage>,
}

impl WebConnection {
    pub fn new(
        stream: TcpStream,
        addr: String,
        sender: Sender<NetworkMessage>,
        receiver: Receiver<DatabaseMessage>,
    ) -> Self {
        Self {
            stream,
            addr,
            sender,
            receiver,
        }
    }

    pub fn update(&mut self) {
        let data = read(&mut self.stream);
        let mut headers = [httparse::EMPTY_HEADER; 64];
        let mut req = httparse::Request::new(&mut headers);
        if let Ok(request) = req.parse(&data) {
            let data = String::from_utf8_lossy(&data).to_string();
            let mut flag = false;
            let mut total = Vec::<String>::new();
            for line in data.split("\n") {
                if line.trim().is_empty() {
                    flag = true;
                }
                if flag {
                    total.push(line.trim().to_string());
                }
            }
            let body = total.join("");
            let headers = vec![("Content-Lenght".to_string(), "1".to_string())];
            let mut headers = new_headers(&headers);
            let mut response = httparse::Response::new(&mut headers);
            response.code = Some(200);
            response.reason = Some("OK");
            let response = response_to_bytes(response, None);
            let _ = self.stream.write(&response);
        } else {
            let mut response = httparse::Response::new(&mut []);
            response.code = Some(400);
            response.reason = Some("Invalid Request");
            let response = response_to_bytes(response, None);
            let _ = self.stream.write(&response);
        }
    }

    fn handle_request(&mut self, request: &httparse::Request) {
        if let Some(path) = request.path {
            let slice = path.split("/").collect::<Vec<&str>>();
            match slice[1] {
                "chat" => self.get_chat(request),
                "new_chat" => self.new_chat(request),
                "new_message" => self.send_message(request),
                _ => self.handle_invalid_endpoint(),
            }
        }
    }

    fn new_chat(&mut self, request: &httparse::Request) {
        if let Some(token) = get_header(&request.headers, "Token") {
            let _ = self
                .sender
                .send(NetworkMessage::TokenValidation(self.addr.clone(), token));
            let response = self.receiver.recv().unwrap();
            match response {
                DatabaseMessage::Email(email) => {
                    let message = NetworkMessage::NewChat(self.addr.clone(), email);
                    let _ = self.sender.send(message);
                    let response = self.receiver.recv().unwrap();
                    match response {
                        DatabaseMessage::Messages(_) => {}
                        _ => {}
                    }
                }
                _ => {}
            }
        } else {
        }
    }

    fn get_chat(&mut self, request: &httparse::Request) {}

    fn send_message(&mut self, request: &httparse::Request) {}

    fn handle_invalid_endpoint(&mut self) {}
}

fn read(stream: &mut TcpStream) -> Vec<u8> {
    let mut buffer = vec![0; 1024];
    let mut total_data = Vec::new();
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => {
                total_data.extend_from_slice(&buffer[..n]);
                if n < buffer.len() {
                    break;
                }
            }
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                break;
            }
            Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => panic!("Error while reading from stream: {:?}", e),
        }
    }
    total_data
}
