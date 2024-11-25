use modules::{
    database::{database::DbConnection, types::*},
    web_client::server::WebServer,
};
use std::sync::mpsc::{channel, Receiver, Sender};

mod modules;

fn main() {
    let (
        (id_sender, id_receiver),
        (network_sender, network_receiver),
        (channel_sender, channel_receiver),
    ) = create_channels();
    let mut database = DbConnection::new(network_receiver, id_receiver, channel_sender);
    let mut server = WebServer::new(id_sender, network_sender, channel_receiver);
    std::thread::spawn(move || {
        server.update();
    });
    std::thread::spawn(move || {
        database.update();
    });
    loop {}
}

fn create_channels() -> (
    (Sender<String>, Receiver<String>),
    (Sender<NetworkMessage>, Receiver<NetworkMessage>),
    (
        Sender<Receiver<DatabaseMessage>>,
        Receiver<Receiver<DatabaseMessage>>,
    ),
) {
    let id_channel = channel();
    let network_channel = channel();
    let channels_channel = channel();
    (id_channel, network_channel, channels_channel)
}
