#![feature(custom_derive, plugin)]
#![plugin(serde_macros)]

extern crate ansi_term;
extern crate sirpent;
extern crate rand;
extern crate uuid;
extern crate serde;
extern crate serde_json;
extern crate openssl;
#[cfg(test)]
extern crate quickcheck;

use ansi_term::Colour::*;
use uuid::Uuid;
use std::collections::HashMap;
use std::net::TcpStream;
use std::thread;
use std::io::{Write, Read, BufReader, BufWriter};
use openssl::ssl::{SslContext, SslMethod, SslStream, MaybeSslStream};
use std::str;
use std::time;
use std::path::PathBuf;

use sirpent::*;

// fn tick<V: Vector>(game: Game<V>) {
// @TODO: Use lifetimes to avoid looping over Clone-d game.players, and cloning in general.
// for (player_name, player) in game.clone().players {
// println!("Ticking on Player name={}", player_name);
//
// let request_move = RequestMove::new(player, game.clone());
// println!("request_move json={}",
// serde_json::to_string_pretty(&request_move).unwrap());
// player.send(request_move);
// player.recv(player_move);
// }
// }

fn main() {
    println!("{}", Yellow.bold().paint("Sirpent"));

    let world = World::HexagonGrid(HexagonGrid { radius: 5 });
    let state = GameState {
        food: HexagonVector { x: 9, y: 13 },
        snakes: HashMap::new(),
    };
    let mut game = Game {
        uuid: Uuid::new_v4(),
        world: world,
        players: HashMap::new(),
        state: state,
    };

    let segments = vec![HexagonVector { x: 3, y: 8 }];
    let snake = Snake::new(segments);
    let player = Player::new("abserde".to_string(), None, snake.uuid.clone());

    game.players.insert(player.name.clone(), player);
    game.state.snakes.insert(snake.uuid.clone(), snake);

    // tick::<HexagonVector>(game);

    // -----------------------------------------------------------------------

    thread::spawn(move || {
        let plain_server = SirpentServer::plain("0.0.0.0:5513").unwrap();
        plain_server.listen(&player_connection_handler, None)
    });

    // -----------------------------------------------------------------------

    let cert = PathBuf::from("cert.pem");
    let key = PathBuf::from("key.pem");
    thread::spawn(move || {
        let tls_server = SirpentServer::tls(cert, key, "0.0.0.0:5514").unwrap();
        tls_server.listen(&player_connection_handler, None)
    });

    // -----------------------------------------------------------------------

    thread::sleep(time::Duration::from_millis(500));
    thread::spawn(move || tell_player_to_unsecured());
    thread::sleep(time::Duration::from_millis(500));
    thread::spawn(move || tell_player_to_ssl());

    loop {
        thread::sleep(time::Duration::from_millis(500));
    }
}

// @TODO: Get a competent review of the decoding code, and move into a type-parametric
// read function.
fn player_connection_handler(_: MaybeSslStream<TcpStream>,
                             reader: BufReader<MaybeSslStream<TcpStream>>,
                             _: BufWriter<MaybeSslStream<TcpStream>>) {

    // Prevent memory exhaustion: stop reading from string after 1MiB.
    // @TODO @DEBUG: Need to reset this for each new message communication.
    // let mut take = reader.clone().take(0xfffff);

    let mut message_iter = serde_json::StreamDeserializer::new(reader.bytes());
    let player: Message<HexagonVector>;
    player = message_iter.next().unwrap().ok().unwrap();
    println!("{:?}", player);
}

pub fn tell_player_to_unsecured() {
    let stream = TcpStream::connect("127.0.0.1:5513").unwrap();

    let player = Player {
        name: "daenerys".to_string(),
        secret: Some("DeagOLmol3105764438410301265454621913800982laskhdasdj".to_string()),
        snake_uuid: None,
    };
    let message: Message<HexagonVector> = Message::new(Command::HELLO { player: player });

    println!("{:?}", serde_json::to_string(&message).unwrap());

    let mut bw = BufWriter::new(stream);
    bw.write(serde_json::to_string(&message).unwrap().as_bytes()).unwrap();
    bw.flush().unwrap();
}

pub fn tell_player_to_ssl() {
    let stream = TcpStream::connect("127.0.0.1:5514").unwrap();
    let ssl = ssl_to_io(SslContext::new(SslMethod::Tlsv1)).unwrap();
    let ssl_stream = ssl_to_io(SslStream::connect(&ssl, stream)).unwrap();

    let player = Player {
        name: "daenerys".to_string(),
        secret: Some("DeagOLmol3105764438410301265454621913800982laskhdasdj".to_string()),
        snake_uuid: None,
    };
    let message: Message<HexagonVector> = Message::new(Command::HELLO { player: player });

    println!("{:?}", serde_json::to_string(&message).unwrap());

    let mut bw = BufWriter::new(ssl_stream);
    bw.write(serde_json::to_string(&message).unwrap().as_bytes()).unwrap();
    bw.flush().unwrap();
}
