use crate::modules::{
    database::types::*,
    web_client::types::{Message, UserInfo, WebMessage},
};
use sha2::Digest;
use sqlite::Connection;
use std::{
    collections::HashMap,
    sync::mpsc::{channel, Receiver, Sender},
};

enum ValidationError {
    InvalidCredentials,
    TokenExpired,
}

pub struct DbConnection {
    connection: Connection,
    senders: HashMap<String, Sender<DatabaseMessage>>,
    email_senders: HashMap<String, HashMap<String, Sender<DatabaseMessage>>>,
    receiver: Receiver<NetworkMessage>,
    nreceiver: Receiver<String>,
    receiver_sender: Sender<Receiver<DatabaseMessage>>,
}

impl DbConnection {
    pub fn new(
        receiver: Receiver<NetworkMessage>,
        nreceiver: Receiver<String>,
        receiver_sender: Sender<Receiver<DatabaseMessage>>,
    ) -> Self {
        Self {
            receiver,
            nreceiver,
            receiver_sender,
            senders: HashMap::new(),
            email_senders: HashMap::new(),
            connection: Connection::open("database").unwrap(),
        }
    }

    pub fn update(&mut self) {
        loop {
            self.receive_new_connections();
            self.receive_messages();
        }
    }

    fn receive_new_connections(&mut self) {
        if let Ok(id) = self.nreceiver.try_recv() {
            let (sender, receiver) = channel();
            self.senders.insert(id, sender);
            let _ = self.receiver_sender.send(receiver);
        }
    }

    fn receive_messages(&mut self) {
        while let Ok(message) = self.receiver.try_recv() {
            match message {
                NetworkMessage::LoginRequest(ref id, ref email, ref password_hash) => {
                    let sender = self.senders.get(id).unwrap();
                    let message = if let Some(token) =
                        self.validate_connection(email, password_hash)
                    {
                        let name = self.get_user_name(email);
                        if let Some(name) = name {
                            DatabaseMessage::UserInfo(UserInfo::new(email.to_string(), name, token))
                        } else {
                            DatabaseMessage::Err
                        }
                    } else {
                        DatabaseMessage::Err
                    };
                    let _ = sender.send(message);
                }
                NetworkMessage::TokenValidation(ref id, ref token) => {
                    let sender = self.senders.get(id).unwrap();
                    let email = self.validate_token(token);
                    if let Some(email) = email {
                        let _ = sender.send(DatabaseMessage::Email(email.clone()));
                        let entry = self.email_senders.entry(email).or_insert(HashMap::new());
                        entry.insert(id.to_string(), sender.clone());
                    } else {
                        let _ = sender.send(DatabaseMessage::Err);
                    }
                }
                NetworkMessage::NewChat(ref _id, ref email) => {
                    let chat_id = self.new_chat(email);
                    if let Some(senders) = self.email_senders.get(email) {
                        for (_rid, sender) in senders {
                            let _ = sender.send(DatabaseMessage::NewChat(chat_id.clone()));
                        }
                    }
                }
                NetworkMessage::ChatRequest(ref id, ref token, ref chat_id) => {
                    let sender = self.senders.get(id).unwrap();
                    let email = self.validate_token(token);
                    if let Some(ref email) = email {
                        let messages = self.get_chat_messages(email, chat_id);
                        let _ =
                            sender.send(DatabaseMessage::Messages(chat_id.to_string(), messages));
                    } else {
                        let _ = sender.send(DatabaseMessage::Err);
                    }
                }
                NetworkMessage::NewMessage(
                    ref id,
                    ref token,
                    ref chat_sender,
                    ref chat_id,
                    ref content,
                    ref message_id,
                ) => {
                    let sender = self.senders.get(id).unwrap();
                    let email = self.validate_token(token);
                    if let Some(ref email) = email {
                        let timestamp =
                            self.new_chat_message(email, chat_sender, chat_id, content, message_id);
                        let _ = sender.send(DatabaseMessage::Timestamp(timestamp));
                        if let Some(senders) = self.email_senders.get(email) {
                            for (rid, sender) in senders {
                                if rid != id {
                                    let _ =
                                        sender.send(DatabaseMessage::WebMessage(WebMessage::new(
                                            Message::new(chat_sender, content),
                                            timestamp,
                                            message_id.to_string(),
                                        )));
                                }
                            }
                        }
                    } else {
                        let _ = sender.send(DatabaseMessage::Err);
                    }
                }
                NetworkMessage::GetChats(ref id, ref email) => {
                    let sender = self.senders.get(id).unwrap();
                    let chats = self.get_chats(email);
                    let _ = sender.send(DatabaseMessage::Chats(chats));
                }
                NetworkMessage::DeleteChat(ref id, ref token, ref chat_id) => {
                    let sender = self.senders.get(id).unwrap();
                    let email = self.validate_token(token);
                    if let Some(ref email) = email {
                        self.delete_chat(email, chat_id);
                        self.delete_messages(email, chat_id);
                        let _ = sender.send(DatabaseMessage::Deleted(chat_id.clone()));
                        if let Some(senders) = self.email_senders.get(email) {
                            for (rid, sender) in senders {
                                if rid != id {
                                    let _ = sender.send(DatabaseMessage::Deleted(chat_id.clone()));
                                }
                            }
                        }
                    } else {
                        let _ = sender.send(DatabaseMessage::Err);
                    }
                }
                NetworkMessage::RegisterUser(ref id, ref name, ref email, ref password) => {
                    let sender = self.senders.get(id).unwrap();
                    if self.user_exists(email) {
                        let _ = sender.send(DatabaseMessage::Err);
                        return;
                    }
                    self.register_user(name, email, password);
                    let token = self.create_token(email);
                    let _ = sender.send(DatabaseMessage::Token(token));
                }
                NetworkMessage::GetMessage(ref id, ref message_id) => {
                    let sender = self.senders.get(id).unwrap();
                    let result = self.get_message(message_id);
                    if let Some(ref message) = result {
                        let _ = sender.send(DatabaseMessage::Message(Message::new("", message)));
                        return;
                    }
                    let _ = sender.send(DatabaseMessage::Err);
                }
                NetworkMessage::GetAudioPath(ref id, ref message_id) => {
                    let sender = self.senders.get(id).unwrap();
                    let result = self.get_audio_path(message_id);
                    if let Some(path) = result {
                        let _ = sender.send(DatabaseMessage::AudioPath(path));
                        return;
                    }
                    let _ = sender.send(DatabaseMessage::Err);
                }
                NetworkMessage::RecordAudioPath(ref message_id, ref path) => {
                    self.record_audio_path(message_id, path)
                }
            }
        }
    }

    fn get_user_name(&self, email: &str) -> Option<String> {
        let query = "select name from UserInfo where email = ?";
        let mut statement = self.connection.prepare(query).unwrap();
        statement.bind((1, email)).unwrap();
        for result in statement.into_iter() {
            if let Ok(row) = result {
                let name = row.read::<&str, _>("name");
                return Some(name.to_string());
            }
        }
        None
    }

    fn record_audio_path(&self, message_id: &str, path: &str) {
        let query = "insert into AudioPaths values (?, ?)";
        let mut statement = self.connection.prepare(query).unwrap();
        statement.bind_iter([(1, message_id), (2, path)]).unwrap();
        statement.iter().count();
    }

    fn get_audio_path(&self, message_id: &str) -> Option<String> {
        let query = "select path from AudioPaths where id = ?";
        let mut statement = self.connection.prepare(query).unwrap();
        statement.bind((1, message_id)).unwrap();
        for result in statement.into_iter() {
            if let Ok(row) = result {
                let path = row.read::<&str, _>("path");
                return Some(path.to_string());
            }
        }
        None
    }

    fn get_message(&self, message_id: &str) -> Option<String> {
        println!("{message_id}");
        let query = "select content from Messages where id = ?";
        let mut statement = self.connection.prepare(query).unwrap();
        statement.bind((1, message_id)).unwrap();
        for result in statement.into_iter() {
            if let Ok(row) = result {
                let message = row.read::<&str, _>("content");
                return Some(message.to_string());
            }
        }
        None
    }

    fn delete_messages(&self, email: &str, chat_id: &str) {
        let query = "delete from Messages where email = ? and chat_id = ?";
        let mut statement = self.connection.prepare(query).unwrap();
        statement.bind_iter([(1, email), (2, chat_id)]).unwrap();
        statement.iter().count();
    }

    fn delete_chat(&self, email: &str, chat_id: &str) {
        let query = "delete from Chats where email = ? and chat_id = ?";
        let mut statement = self.connection.prepare(query).unwrap();
        statement.bind_iter([(1, email), (2, chat_id)]).unwrap();
        statement.iter().count();
    }

    fn new_chat_message(
        &self,
        email: &str,
        sender: &str,
        chat_id: &str,
        content: &str,
        message_id: &str,
    ) -> u64 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let query = "insert into Messages values (?, ?, ?, ?, ?, ?)";
        let mut statement = self.connection.prepare(query).unwrap();
        statement
            .bind_iter([
                (1, email),
                (2, chat_id),
                (3, sender),
                (4, content),
                (5, &now.to_string()),
                (6, message_id),
            ])
            .expect("coulndt fit now");
        println!("{}", statement.iter().count());
        now
    }

    fn get_chat_messages(&self, email: &str, chat_id: &str) -> Vec<WebMessage> {
        println!("{email} {chat_id}");
        let query =
            "select * from Messages where email = ? and chat_id = ? order by datetime limit 50";
        let mut statement = self.connection.prepare(query).unwrap();
        statement.bind_iter([(1, email), (2, chat_id)]).unwrap();
        let mut messages = Vec::new();
        for result in statement.into_iter() {
            if let Ok(row) = result {
                let sender = row.read::<&str, _>("sender");
                let content = row.read::<&str, _>("content");
                let timestamp = row.read::<i64, _>("datetime");
                let id = row.read::<&str, _>("id");
                let message = Message::new(sender, content);
                messages.push(WebMessage::new(message, timestamp as u64, id.to_string()));
            } else {
                println!("is not ok");
            }
        }
        messages
    }

    fn new_chat(&self, email: &str) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let query = "insert into Chats values (?, ?)";
        let mut statement = self.connection.prepare(query).unwrap();
        statement.bind_iter([(1, email), (2, id.as_str())]).unwrap();
        statement.iter().count();
        id
    }

    fn validate_connection(&self, email: &str, password: &str) -> Option<String> {
        let query = "select * from Users where email = ? and password = ?";
        let mut statement = self.connection.prepare(query).unwrap();
        let _ = statement.bind_iter([(1, email), (2, password)]);
        if statement.into_iter().count() == 1 {
            let query = "select * from Tokens where email = ?";
            let mut statement = self.connection.prepare(query).unwrap();
            statement.bind((1, email)).unwrap();
            if statement.iter().count() == 1 {
                for result in statement.into_iter() {
                    if let Ok(row) = result {
                        let expire = row.read::<i64, _>("expire");
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs();
                        if now as i64 <= expire {
                            return Some(row.read::<&str, _>("token").to_string());
                        } else {
                            return Some(self.create_token(email));
                        }
                    }
                }
            } else {
                Some(self.create_token(email));
            }
        }
        None
    }

    fn validate_token(&self, token: &str) -> Option<String> {
        let query = "select * from Tokens where token = ?";
        let mut statement = self.connection.prepare(query).unwrap();
        statement.bind((1, token)).unwrap();
        for result in statement.into_iter() {
            if let Ok(row) = result {
                let expire = row.read::<i64, _>("expire");
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                if now as i64 <= expire {
                    return Some(row.read::<&str, _>("email").to_string());
                } else {
                    return None;
                }
            }
        }
        None
    }

    fn create_token(&self, email: &str) -> String {
        self.delete_tokens(email);
        let mut seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        seconds += 60 * 60 * 24;
        let query = "insert into Tokens values (?, ?, ?)";
        let token = uuid::Uuid::new_v4().to_string();
        let mut statement = self.connection.prepare(query).unwrap();
        statement
            .bind_iter([(1, token.as_str()), (2, email), (3, &seconds.to_string())])
            .unwrap();
        statement.iter().count();
        token
    }

    fn delete_tokens(&self, email: &str) {
        let query = "delete from Tokens where email = ?";
        let mut statement = self.connection.prepare(query).unwrap();
        statement.bind((1, email)).unwrap();
        statement.iter().count();
    }

    fn get_chats(&self, email: &str) -> Vec<String> {
        let query = "select chat_id from Chats where email = ?";
        let mut statement = self.connection.prepare(query).unwrap();
        statement.bind((1, email)).unwrap();
        let mut chats = Vec::new();
        for result in statement.into_iter() {
            if let Ok(row) = result {
                let chat_id = row.read::<&str, _>("chat_id");
                chats.push(chat_id.to_string());
            }
        }
        chats
    }

    fn user_exists(&self, email: &str) -> bool {
        let query = "select * from Users where email = ?";
        let mut statement = self.connection.prepare(query).unwrap();
        statement.bind((1, email)).unwrap();
        statement.iter().count() == 1
    }

    fn register_user(&self, name: &str, email: &str, password: &str) {
        let mut sha = sha2::Sha256::new();
        sha.update(password);
        let password_hash = sha.finalize();
        let hash = hex::encode(password_hash);
        let query = "insert into Users values (?, ?)";
        let mut statement = self.connection.prepare(query).unwrap();
        statement.bind_iter([(1, email), (2, &hash)]).unwrap();
        statement.iter().count();
        let query = "insert into UserInfo values (?, ?)";
        let mut statement = self.connection.prepare(query).unwrap();
        statement.bind_iter([(1, email), (2, name)]).unwrap();
        statement.iter().count();
    }
}
