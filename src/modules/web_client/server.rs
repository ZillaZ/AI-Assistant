use crate::modules::database::types::*;
use crate::modules::web_client::{client::WebClient, http::*, types::*};
use serde_json::json;
use sha2::{Digest, Sha256, Sha512};
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
        let listener = TcpListener::bind("0.0.0.0:8080").expect("Unable to start tcp listener");
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
    web_client: WebClient,
}

impl WebConnection {
    pub fn new(
        stream: TcpStream,
        addr: String,
        sender: Sender<NetworkMessage>,
        receiver: Receiver<DatabaseMessage>,
    ) -> Self {
        let web_client = WebClient::new();
        Self {
            stream,
            addr,
            sender,
            receiver,
            web_client,
        }
    }

    pub fn update(&mut self) {
        let data = read(&mut self.stream);
        let mut headers = [httparse::EMPTY_HEADER; 64];
        let mut req = httparse::Request::new(&mut headers);
        if let Ok(_) = req.parse(&data) {
            self.handle_request(&req, &data);
        } else {
            let mut response = httparse::Response::new(&mut []);
            response.code = Some(400);
            response.reason = Some("Invalid Request");
            let response = response_to_bytes(response, None::<String>);
            let _ = self.stream.write(&response);
        }
    }

    fn handle_request(&mut self, request: &httparse::Request, data: &Vec<u8>) {
        if self.handle_cors(request) {
            return;
        }
        if let Some(path) = request.path {
            let slice = path.split("/").collect::<Vec<&str>>();
            if let Some(path) = slice.get(1) {
                match *path {
                    "chat" => self.get_chat(request),
                    "new_chat" => self.new_chat(request),
                    "new_message" => self.send_message(request, data),
                    "login" => self.login(request, data),
                    "chats" => self.get_chats(request),
                    "delete_chat" => self.delete_chat(request, data),
                    "register" => self.register_user(request, data),
                    "audio" => self.get_audio(request, data),
                    _ => self.handle_invalid_endpoint(),
                }
            } else {
                self.generic_error("Invalid Path".into());
            }
        }
    }

    fn get_audio(&mut self, request: &httparse::Request, data: &Vec<u8>) {
        if let Some(token) = get_header(&request.headers, "Token") {
            let _ = self
                .sender
                .send(NetworkMessage::TokenValidation(self.addr.clone(), token));
            let response = self.receiver.recv().unwrap();
            match response {
                DatabaseMessage::Email(ref email) => {
                    let body = String::from_utf8_lossy(data);
                    let id = get_request_body(body.to_string());
                    let _ = self
                        .sender
                        .send(NetworkMessage::GetMessage(self.addr.clone(), id.clone()));
                    let response = self.receiver.recv().unwrap();
                    match response {
                        DatabaseMessage::Message(ref message) => {
                            let data = self.get_audio_file(id, message);
                            let data_len = data.len().to_string();
                            let headers = vec![
                                ("Access-Control-Allow-Origin", "*"),
                                ("Access-Control-Allow-Headers", "*"),
                                ("Content-Length", data_len.as_str()),
                            ];
                            let mut headers = new_headers(&headers);
                            let mut response = httparse::Response::new(&mut headers);
                            response.code = Some(200);
                            response.reason = Some("OK");
                            let response = response_to_bytes(response, Some(data));
                            let _ = self.stream.write_all(&response);
                        }
                        _ => self.generic_error("Invalid Message ID".into()),
                    }
                }
                _ => self.generic_error("Invalid Token".into()),
            }
        } else {
            self.generic_error("No token provided".into());
        }
    }

    fn get_audio_file(&mut self, id: String, message: &str) -> String {
        let _ = self
            .sender
            .send(NetworkMessage::GetAudioPath(self.addr.clone(), id.clone()));
        let response = self.receiver.recv().unwrap();
        match response {
            DatabaseMessage::AudioPath(ref path) => std::fs::read_to_string(path).unwrap(),
            _ => {
                let audio = self.web_client.new_audio(message.to_string()).unwrap();
                let path = format!("static/{}", &id);
                let _ = self
                    .sender
                    .send(NetworkMessage::RecordAudioPath(id, path.clone()));
                std::fs::write(path, &audio).unwrap();
                audio
            }
        }
    }

    fn register_user(&mut self, request: &httparse::Request, data: &Vec<u8>) {
        let body = String::from_utf8_lossy(data);
        let credentials = get_request_body(body.to_string());
        let slice = credentials.split("\n").collect::<Vec<&str>>();
        if let Some(email) = slice.get(0) {
            let email = email.trim();
            if let Some(password) = slice.get(1) {
                let password = password.trim();
                if email.trim().len() < 2 || password.trim().len() < 8 {
                    self.generic_error("Invalid Credentials".into());
                    return;
                }
                let _ = self.sender.send(NetworkMessage::RegisterUser(
                    self.addr.clone(),
                    email.to_string(),
                    password.to_string(),
                ));
                let response = self.receiver.recv().unwrap();
                match response {
                    DatabaseMessage::Token(ref token) => {
                        let headers = vec![
                            ("Access-Control-Allow-Origin", "*"),
                            ("Access-Control-Allow-Headers", "*"),
                        ];
                        let mut headers = new_headers(&headers);
                        let mut response = httparse::Response::new(&mut headers);
                        response.code = Some(200);
                        response.reason = Some("OK");
                        let response = response_to_bytes(response, Some(token.to_string()));
                        let _ = self.stream.write_all(&response);
                    }
                    _ => self.generic_error("Invalid Credentials".into()),
                }
            } else {
                self.generic_error("No password is present".into());
            }
        } else {
            self.generic_error("No credentials on request".into());
        }
    }

    fn delete_chat(&mut self, request: &httparse::Request, data: &Vec<u8>) {
        let token = get_header(&request.headers, "Token");
        if let Some(token) = token {
            let chat_id = get_request_body(String::from_utf8_lossy(data).to_string());
            let _ = self.sender.send(NetworkMessage::DeleteChat(
                self.addr.clone(),
                token.to_string(),
                chat_id,
            ));
            let response = self.receiver.recv().unwrap();
            match response {
                DatabaseMessage::Ok => {
                    let headers = vec![
                        ("Access-Control-Allow-Origin", "*"),
                        ("Access-Control-Allow-Headers", "*"),
                    ];
                    let mut headers = new_headers(&headers);
                    let mut response = httparse::Response::new(&mut headers);
                    response.code = Some(200);
                    response.reason = Some("OK");
                    let response = response_to_bytes(response, None::<String>);
                    let _ = self.stream.write_all(&response);
                }
                _ => self.generic_error(None),
            }
        } else {
            self.generic_error(None);
        }
    }

    fn handle_cors(&mut self, request: &httparse::Request) -> bool {
        if let Some(method) = request.method {
            if method != "OPTIONS" {
                return false;
            }
            println!("RECEIVED CORS REQUEST");
            let headers = vec![
                ("Access-Control-Allow-Origin", "*"),
                ("Access-Control-Allow-Headers", "*"),
            ];
            let mut headers = new_headers(&headers);
            let mut response = httparse::Response::new(&mut headers);
            response.code = Some(200);
            response.reason = Some("OK");
            let response = response_to_bytes(response, None::<String>);
            let _ = self.stream.write_all(&response);
            return true;
        }
        false
    }

    fn get_chats(&mut self, request: &httparse::Request) {
        if let Some(token) = get_header(&request.headers, "Token") {
            let _ = self
                .sender
                .send(NetworkMessage::TokenValidation(self.addr.clone(), token));
            let response = self.receiver.recv().unwrap();
            match response {
                DatabaseMessage::Email(email) => {
                    let message = NetworkMessage::GetChats(self.addr.clone(), email);
                    let _ = self.sender.send(message);
                    let response = self.receiver.recv().unwrap();
                    match response {
                        DatabaseMessage::Chats(ref chats) => {
                            let chats = chats.join("\n");
                            let chats_len = chats.len().to_string();
                            let headers = vec![
                                ("Access-Control-Allow-Origin", "*"),
                                ("Content-Length", chats_len.as_str()),
                                ("Content-Type", "text/plain"),
                            ];
                            let mut headers = new_headers(&headers);
                            let mut response = httparse::Response::new(&mut headers);
                            response.code = Some(200);
                            response.reason = Some("OK");
                            let response = response_to_bytes(response, Some(chats));
                            let _ = self.stream.write(&response);
                        }
                        _ => self.generic_error("Db response is not messages".into()),
                    }
                }
                _ => self.generic_error("Db response was not email".into()),
            }
        } else {
            self.generic_error("There is no token header".into());
        }
    }

    fn login(&mut self, request: &httparse::Request, data: &Vec<u8>) {
        println!("received login request");
        let content = get_request_body(String::from_utf8_lossy(data).to_string());
        let slice = content
            .split("\n")
            .filter(|x| !x.is_empty())
            .collect::<Vec<&str>>();
        if let Some(email) = slice.get(0) {
            if let Some(password) = slice.get(1) {
                let mut hasher = sha2::Sha256::new();
                hasher.update(password.trim());
                let finalized = &hasher.finalize();
                let password_hash = hex::encode(finalized);

                let _ = self.sender.send(NetworkMessage::LoginRequest(
                    self.addr.clone(),
                    email.to_string(),
                    password_hash.to_string(),
                ));
                let response = self.receiver.recv().unwrap();
                match response {
                    DatabaseMessage::Token(token) => {
                        let token = format!("Token={token}");
                        let headers = vec![
                            ("Access-Control-Allow-Origin", "*"),
                            ("Access-Control-Expose-Headers", "*"),
                            ("Cookie", &token),
                        ];
                        let mut headers = new_headers(&headers);
                        let mut response = httparse::Response::new(&mut headers);
                        response.code = Some(200);
                        response.reason = Some("OK");
                        let response = response_to_bytes(response, None::<String>);
                        let _ = self.stream.write_all(&response);
                    }
                    _ => self.generic_error("Invalid Credentials".into()),
                }
            } else {
                self.generic_error("No password".into());
            }
        } else {
            self.generic_error("No email".into());
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
                        DatabaseMessage::Messages(id, _) => {
                            let id_str = id.len().to_string();
                            let headers = vec![
                                ("Content-Length", id_str.as_str()),
                                ("Content-Type", "text/plain"),
                                ("Access-Control-Allow-Origin", "*"),
                            ];
                            let mut headers = new_headers(&headers);
                            let mut response = httparse::Response::new(&mut headers);
                            response.code = Some(200);
                            response.reason = Some("OK");
                            let response = response_to_bytes(response, Some(id));
                            let _ = self.stream.write(&response);
                        }
                        _ => self.generic_error("Db response is not messages".into()),
                    }
                }
                _ => self.generic_error("Db response was not email".into()),
            }
        } else {
            self.generic_error("There is no token header".into());
        }
    }

    fn generic_error(&mut self, reason: Option<&str>) {
        let headers = vec![("Access-Control-Allow-Origin", "*")];
        let mut headers = new_headers(&headers);
        let mut response = httparse::Response::new(&mut headers);
        response.code = Some(400);
        response.reason = reason;
        let response = response_to_bytes(response, None::<String>);
        let _ = self.stream.write_all(&response);
    }

    fn get_chat(&mut self, request: &httparse::Request) {
        let path = request.path.unwrap();
        let slice = path.split("/").collect::<Vec<&str>>();
        if let Some(chat_id) = slice.get(2) {
            let token = get_header(&request.headers, "Token");
            if let Some(token) = token {
                let _ = self.sender.send(NetworkMessage::ChatRequest(
                    self.addr.clone(),
                    token,
                    chat_id.to_string(),
                ));
                let response = self.receiver.recv().unwrap();

                match response {
                    DatabaseMessage::Messages(ref id, messages) => {
                        let messages = Messages::new(messages);
                        let body = json!(messages).to_string();
                        let body_len = body.len().to_string();
                        let headers = Vec::from([
                            ("Access-Control-Allow-Origin", "*"),
                            ("Content-Type", "application/json"),
                            ("Content-Length", body_len.as_str()),
                        ]);
                        let mut headers = new_headers(&headers);
                        let mut response = httparse::Response::new(&mut headers);
                        response.code = Some(200);
                        response.reason = Some("OK");
                        let response = response_to_bytes(response, Some(body));
                        let _ = self.stream.write_all(&response);
                    }
                    _ => self.generic_error("Invalid chat id".into()),
                }
            } else {
                self.generic_error("Unauthorized".into());
            }
        } else {
            self.generic_error("Invalid URI".into());
        }
    }

    fn send_message(&mut self, request: &httparse::Request, data: &Vec<u8>) {
        let path = request.path.unwrap();
        let slice = path.split("/").collect::<Vec<&str>>();
        if let Some(chat_id) = slice.get(2) {
            let token = get_header(&request.headers, "Token");
            if let Some(token) = token {
                let content = get_request_body(String::from_utf8_lossy(data).to_string());
                if content.trim().len() < 1 {
                    self.generic_error("Empty message".into());
                    return;
                }
                let _ = self.sender.send(NetworkMessage::NewMessage(
                    self.addr.clone(),
                    token.clone(),
                    "user".to_string(),
                    chat_id.to_string(),
                    content.clone(),
                    uuid::Uuid::new_v4().to_string(),
                ));
                let message = Message::new("user", &content);
                let response = self.receiver.recv().unwrap();
                match response {
                    DatabaseMessage::Timestamp(timestamp) => {
                        if &self.web_client.chat_id != chat_id {
                            if let Some(messages) = self.retrieve_messages(&token, chat_id) {
                                self.web_client.load_context(messages);
                                self.web_client.chat_id = chat_id.to_string();
                            }
                        }
                        let id = uuid::Uuid::new_v4().to_string();
                        let message = WebMessage::new(message, timestamp, String::new());
                        let mut copy: Message = Message::new("", "");
                        let _ = copy;
                        if let Some(answer) = self.web_client.new_message(message) {
                            copy = answer.clone();
                            let _ = self.sender.send(NetworkMessage::NewMessage(
                                self.addr.clone(),
                                token,
                                answer.role.as_ref().unwrap().clone(),
                                chat_id.to_string(),
                                answer.content.as_ref().unwrap().clone(),
                                id.clone(),
                            ));
                            self.receiver.recv().unwrap();
                        } else {
                            self.generic_error("API request blocked".into());
                            return;
                        }
                        let body = json!(WebMessage::new(copy, timestamp, id)).to_string();
                        let body_len = body.as_bytes().len().to_string();
                        let headers = vec![
                            ("Access-Control-Allow-Origin", "*"),
                            ("Content-Type", "application/json"),
                            ("Content-Length", body_len.as_str()),
                        ];
                        let mut headers = new_headers(&headers);
                        let mut response = httparse::Response::new(&mut headers);
                        response.code = Some(200);
                        response.reason = Some("OK");
                        let response = response_to_bytes(response, Some(body));
                        let _ = self.stream.write_all(&response);
                    }
                    _ => self.generic_error("Invalid chat id".into()),
                }
            } else {
                self.generic_error("Unauthorized".into());
            }
        } else {
            self.generic_error("No chat id provided".into());
        }
    }

    fn handle_invalid_endpoint(&mut self) {
        self.generic_error("Not Found".into());
    }

    fn retrieve_messages(&self, token: &str, chat_id: &str) -> Option<Messages> {
        let _ = self.sender.send(NetworkMessage::ChatRequest(
            self.addr.clone(),
            token.to_string(),
            chat_id.to_string(),
        ));
        let response = self.receiver.recv().unwrap();
        match response {
            DatabaseMessage::Messages(id, messages) => Some(Messages::new(messages)),
            _ => None,
        }
    }
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
