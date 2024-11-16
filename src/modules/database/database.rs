use crate::modules::database::types::*;
use sqlite::Connection;
use std::{
    collections::HashMap,
    sync::mpsc::{channel, Receiver, Sender},
};

pub struct DbConnection {
    connection: Connection,
    senders: HashMap<String, Sender<DatabaseMessage>>,
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
                    let message = if self.validate_connection(email, password_hash) {
                        DatabaseMessage::Ok
                    } else {
                        DatabaseMessage::Err
                    };
                    let _ = sender.send(message);
                }
                NetworkMessage::TokenValidation(ref id, ref token) => {
                    let sender = self.senders.get(id).unwrap();
                    let email = self.validate_token(token);
                }
                _ => todo!(),
            }
        }
    }

    fn validate_connection(&self, email: &str, password: &str) -> bool {
        let query = "select * from Users where email = ? and password = ?";
        let mut statement = self.connection.prepare(query).unwrap();
        let _ = statement.bind_iter([(1, email), (2, password)]);
        statement.iter().count() == 1
    }

    fn validate_token(&self, token: &str) -> String {
        let query = "select * from Tokens where token = ?";
        let mut statement = self.connection.prepare(query).unwrap();
        let _ = statement.bind((1, token));
        let emails = statement
            .iter()
            .map(|row| row.unwrap())
            .map(|row| row.read::<&str, _>("email").to_string())
            .collect::<Vec<String>>();
        emails[0].clone()
    }
}
