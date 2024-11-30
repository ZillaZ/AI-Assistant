use crate::modules::database::types::*;
use crate::modules::web_client::{client::WebClient, http::*, types::*};
use serde_json::json;
use sha2::Digest;
use std::{
    io::{ErrorKind, Read, Write},
    sync::mpsc::{Receiver, Sender},
};

use websocket::{
    stream::sync::TcpStream,
    sync::{Client, Reader, Server, Writer},
    OwnedMessage,
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
        let mut listener = Server::bind("0.0.0.0:8080").expect("Unable to start WebSockets server");
        while let Ok(stream) = listener.accept() {
            if let Ok(client) = stream.accept() {
                client.set_nonblocking(true).unwrap();
                let (reader, writer) = client.split().unwrap();
                println!("SUCCESS");
                let addr = uuid::Uuid::new_v4().to_string();
                let _ = self.sender.send(addr.to_string());
                let receiver = self.receiver.recv().unwrap();
                let mut web_connection = WebConnection::new(
                    writer,
                    addr.to_string(),
                    self.network_sender.clone(),
                    receiver,
                );
                std::thread::spawn(move || {
                    web_connection.update(reader);
                });
            }
        }
    }
}

pub struct WebConnection {
    writer: Writer<TcpStream>,
    addr: String,
    sender: Sender<NetworkMessage>,
    receiver: Receiver<DatabaseMessage>,
    web_client: WebClient,
}

impl WebConnection {
    pub fn new(
        writer: Writer<TcpStream>,
        addr: String,
        sender: Sender<NetworkMessage>,
        receiver: Receiver<DatabaseMessage>,
    ) -> Self {
        let web_client = WebClient::new();
        Self {
            writer,
            addr,
            sender,
            receiver,
            web_client,
        }
    }

    pub fn update(&mut self, mut reader: Reader<TcpStream>) {
        loop {
            self.receive_messages();
            if let Ok(message) = reader.recv_message() {
                if let OwnedMessage::Text(data) = message {
                    let mut headers = [httparse::EMPTY_HEADER; 64];
                    let req = httparse::Request::new(&mut headers);
                    self.handle_request(&req, &data);
                }
            }
        }
    }

    fn receive_messages(&mut self) {
        while let Ok(message) = self.receiver.try_recv() {
            match message {
                DatabaseMessage::WebMessage(message) => {
                    let message = json!(ServerResponse::Message(message)).to_string();
                    let _ = self.writer.send_message(&OwnedMessage::Text(message));
                }
                DatabaseMessage::Deleted(chat_id) => {
                    let message = json!(ServerResponse::Deleted(chat_id)).to_string();
                    let _ = self.writer.send_message(&OwnedMessage::Text(message));
                }
                DatabaseMessage::NewChat(chat_id) => {
                    let message = json!(ServerResponse::ChatId(chat_id)).to_string();
                    let _ = self.writer.send_message(&OwnedMessage::Text(message));
                }
                message => todo!("{:?}", message),
            }
        }
    }

    fn handle_request(&mut self, request: &httparse::Request, data: &str) {
        if let Ok(message) = serde_json::from_str::<ClientMessage>(data) {
            match message.body {
                ClientMessageKind::Login(login) => {
                    self.login(login);
                }
                ClientMessageKind::NewChat(new_chat) => {
                    self.new_chat(new_chat);
                }
                ClientMessageKind::DeleteChat(delete_chat) => {
                    self.delete_chat(delete_chat);
                }
                ClientMessageKind::NewMessage(new_message) => self.send_message(new_message),
                ClientMessageKind::GetChats(ref token) => self.get_chats(token),
                ClientMessageKind::GetChat(get_chat) => self.get_chat(get_chat),
                ClientMessageKind::Register(register) => self.register_user(register),
                ClientMessageKind::GetAudio(get_audio) => self.get_audio(get_audio),
            }
        }
    }

    fn get_audio(&mut self, get_audio: GetAudio) {
        let _ = self.sender.send(NetworkMessage::TokenValidation(
            self.addr.clone(),
            get_audio.token,
        ));
        let response = self.receiver.recv().unwrap();
        match response {
            DatabaseMessage::Email(ref _email) => {
                let _ = self.sender.send(NetworkMessage::GetMessage(
                    self.addr.clone(),
                    get_audio.message_id.to_string(),
                ));
                let response = self.receiver.recv().unwrap();
                match response {
                    DatabaseMessage::Message(ref message) => {
                        let message = message.content.as_ref().unwrap();
                        let data = self.get_audio_file(get_audio.message_id.to_string(), message);
                        let response = ServerResponse::Audio(AudioInfo {
                            message_id: get_audio.message_id.to_string(),
                            content: data,
                        });
                        let response = json!(response).to_string();
                        let _ = self.writer.send_message(&OwnedMessage::Text(response));
                    }
                    _ => self.generic_error(403, "Forbidden"),
                }
            }
            _ => self.generic_error(401, "Unauthorized"),
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

    fn register_user(&mut self, register: Register) {
        let _ = self.sender.send(NetworkMessage::RegisterUser(
            self.addr.clone(),
            register.name.to_string(),
            register.email.to_string(),
            register.password.to_string(),
        ));
        let response = self.receiver.recv().unwrap();
        match response {
            DatabaseMessage::Token(ref token) => {
                let info = UserInfo::new(
                    register.email.to_string(),
                    register.name.to_string(),
                    token.to_string(),
                );
                let response = ServerResponse::UserInfo(info);
                let response = json!(response).to_string();
                let _ = self.writer.send_message(&OwnedMessage::Text(response));
            }
            _ => self.generic_error(401, "Unauthorized"),
        }
    }

    fn delete_chat(&mut self, delete_chat: DeleteChat) {
        let _ = self.sender.send(NetworkMessage::DeleteChat(
            self.addr.clone(),
            delete_chat.token,
            delete_chat.chat_id,
        ));
        let response = self.receiver.recv().unwrap();
        match response {
            DatabaseMessage::Deleted(chat_id) => {
                let response = ServerResponse::Deleted(chat_id);
                let response = json!(response).to_string();
                let _ = self.writer.send_message(&OwnedMessage::Text(response));
            }
            _ => self.generic_error(404, "Not Found"),
        }
    }

    fn get_chats(&mut self, token: &str) {
        let _ = self.sender.send(NetworkMessage::TokenValidation(
            self.addr.clone(),
            token.to_string(),
        ));
        let response = self.receiver.recv().unwrap();
        match response {
            DatabaseMessage::Email(email) => {
                let message = NetworkMessage::GetChats(self.addr.clone(), email);
                let _ = self.sender.send(message);
                let response = self.receiver.recv().unwrap();
                match response {
                    DatabaseMessage::Chats(chats) => {
                        let response = ServerResponse::Chats(chats);
                        let response = json!(response).to_string();
                        let _ = self.writer.send_message(&OwnedMessage::Text(response));
                    }
                    _ => self.generic_error(500, "Internal Server Error"),
                }
            }
            _ => self.generic_error(401, "Unauthorized"),
        }
    }

    fn login(&mut self, login: Login) {
        let mut hasher = sha2::Sha256::new();
        hasher.update(login.password.trim());
        let finalized = &hasher.finalize();
        let password_hash = hex::encode(finalized);

        let _ = self.sender.send(NetworkMessage::LoginRequest(
            self.addr.clone(),
            login.email,
            password_hash.to_string(),
        ));
        let response = self.receiver.recv().unwrap();
        match response {
            DatabaseMessage::UserInfo(info) => {
                let response = ServerResponse::UserInfo(info);
                let response = json!(response).to_string();
                let _ = self.writer.send_message(&OwnedMessage::Text(response));
            }
            _ => self.generic_error(401, "Unauthorized"),
        }
    }

    fn new_chat(&mut self, new_chat: NewChat) {
        let _ = self.sender.send(NetworkMessage::TokenValidation(
            self.addr.clone(),
            new_chat.token,
        ));
        let response = self.receiver.recv().unwrap();
        match response {
            DatabaseMessage::Email(email) => {
                let message = NetworkMessage::NewChat(self.addr.clone(), email);
                let _ = self.sender.send(message);
                let response = self.receiver.recv().unwrap();
                match response {
                    DatabaseMessage::NewChat(id) => {
                        let response = ServerResponse::ChatId(id);
                        let response = json!(response).to_string();
                        let _ = self.writer.send_message(&OwnedMessage::Text(response));
                    }
                    _ => self.generic_error(403, "Forbidden"),
                }
            }
            _ => self.generic_error(401, "Unauthorized"),
        }
    }

    fn generic_error(&mut self, code: u16, reason: &str) {
        let headers = vec![("Access-Control-Allow-Origin", "*")];
        let mut headers = new_headers(&headers);
        let mut response = httparse::Response::new(&mut headers);
        response.code = Some(code);
        response.reason = Some(reason);
        let response = response_to_string(response, None::<String>);
        let _ = self.writer.send_message(&OwnedMessage::Text(response));
    }

    fn get_chat(&mut self, get_chat: GetChat) {
        let _ = self.sender.send(NetworkMessage::ChatRequest(
            self.addr.clone(),
            get_chat.token,
            get_chat.chat_id.to_string(),
        ));
        let response = self.receiver.recv().unwrap();

        match response {
            DatabaseMessage::Messages(ref _id, messages) => {
                let messages = Messages::new(messages).messages;
                let response = ServerResponse::Messages(messages);
                let response = json!(response).to_string();
                let _ = self.writer.send_message(&OwnedMessage::Text(response));
            }
            _ => self.generic_error(403, "Forbidden"),
        }
    }

    fn send_message(&mut self, new_message: NewMessage) {
        if new_message.content.trim().len() < 1 {
            self.generic_error(400, "Bad Request");
            return;
        }
        let _ = self.sender.send(NetworkMessage::NewMessage(
            self.addr.clone(),
            new_message.token.clone(),
            "user".to_string(),
            new_message.chat_id.to_string(),
            new_message.content.clone(),
            uuid::Uuid::new_v4().to_string(),
        ));
        let message = Message::new("user", &new_message.content);
        let response = self.receiver.recv().unwrap();
        match response {
            DatabaseMessage::Timestamp(timestamp) => {
                if &self.web_client.chat_id != &new_message.chat_id {
                    if let Some(messages) =
                        self.retrieve_messages(&new_message.token, &new_message.chat_id)
                    {
                        self.web_client.load_context(messages);
                        self.web_client.chat_id = new_message.chat_id.clone();
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
                        new_message.token,
                        answer.role.as_ref().unwrap().clone(),
                        new_message.chat_id.to_string(),
                        answer.content.as_ref().unwrap().clone(),
                        id.clone(),
                    ));
                    self.receiver.recv().unwrap();
                } else {
                    self.generic_error(502, "Bad Gateway");
                    return;
                }
                let response = ServerResponse::Message(WebMessage::new(copy, timestamp, id));
                let response = json!(response).to_string();
                let _ = self.writer.send_message(&OwnedMessage::Text(response));
            }
            _ => self.generic_error(403, "Forbidden"),
        }
    }

    fn handle_invalid_endpoint(&mut self) {
        self.generic_error(404, "Not Found");
    }

    fn retrieve_messages(&self, token: &str, chat_id: &str) -> Option<Messages> {
        let _ = self.sender.send(NetworkMessage::ChatRequest(
            self.addr.clone(),
            token.to_string(),
            chat_id.to_string(),
        ));
        let response = self.receiver.recv().unwrap();
        match response {
            DatabaseMessage::Messages(_id, messages) => Some(Messages::new(messages)),
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
