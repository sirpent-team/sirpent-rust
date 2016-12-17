extern crate ansi_term;
extern crate sirpent;
extern crate rand;
extern crate uuid;
extern crate rayon;

use ansi_term::Colour::*;
use std::thread;
use std::str;
use std::time;
use std::net::TcpStream;
use std::sync::{Arc, Mutex, RwLock};
use rand::os::OsRng;

use sirpent::*;

fn main() {
    println!("{}", Yellow.bold().paint("Sirpent"));

    let osrng = OsRng::new().unwrap();
    let grid = Grid { radius: 15 };
    let engine = Arc::new(RwLock::new(Engine::new(osrng, grid)));

    let waiting_players = Arc::new(Mutex::new(Vec::new()));

    // -----------------------------------------------------------------------

    let waiting_players2 = waiting_players.clone();
    thread::spawn(move || {
        let plain_server = SirpentServer::plain("0.0.0.0:5513").unwrap();
        plain_server.listen(move |stream: TcpStream| {
            // @TODO: New logic for accepting/rejecting/queueing new players.
            if true {
                // game_engine2.read().unwrap().player_connections.is_accepting() {
                let player_connection = player_handshake_handler(stream);
                waiting_players2.lock().unwrap().push(player_connection);
            }
        });
    });

    thread::sleep(time::Duration::from_millis(5000));

    let mut wpl = waiting_players.lock().unwrap();
    let wp = wpl.drain(..).collect();
    engine.write().unwrap().new_game(wp);

    loop {
        let mut engine_writable = engine.write().unwrap();

        // Advance turn.
        let new_turn = engine_writable.turn();

        // Print result of previous turn (here so 0th is printed).
        println!("TURN {}", new_turn.turn_number);
        println!("Snake casualties: {:?}", new_turn.casualties);
        println!("{:?}", new_turn);
        println!("--------------");

        if let Some(victors) = engine_writable.concluded() {
            println!("{:?} Victors: {:?}", victors.len(), victors);
            break;
        }

        thread::sleep(time::Duration::from_millis(500));
    }
}

fn player_handshake_handler(stream: TcpStream) -> PlayerConnection {
    // @TODO: Prevent memory exhaustion: stop reading from string after 1MiB.
    // @TODO @DEBUG: Need to reset this for each new message communication.

    let protocol_connection = ProtocolConnection::new(stream, None)
        .expect("Could not produce new PlayerConnection.");
    PlayerConnection::new(protocol_connection)
}
