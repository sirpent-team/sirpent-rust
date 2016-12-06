extern crate ansi_term;
extern crate sirpent;
extern crate rand;
extern crate uuid;
#[macro_use(chan_select)]
extern crate chan;
extern crate rayon;

use ansi_term::Colour::*;
use std::thread;
use std::str;
use std::time;
use std::net::TcpStream;
use std::result::Result;
use std::sync::{Arc, RwLock};
use rand::os::OsRng;

use sirpent::*;

fn main() {
    println!("{}", Yellow.bold().paint("Sirpent"));

    let mut osrng = OsRng::new().unwrap();
    let grid = Grid { radius: 15 };
    let mut game_state = GameState::new(grid);
    game_state.food.insert(grid.random_cell(&mut osrng));

    let game_engine = Arc::new(RwLock::new(GameEngine::new(osrng, game_state)));

    // -----------------------------------------------------------------------

    let game_engine2 = game_engine.clone();
    thread::spawn(move || {
        let plain_server = SirpentServer::plain("0.0.0.0:5513").unwrap();
        plain_server.listen(move |stream: TcpStream| {
            // @TODO: New logic for accepting/rejecting/queueing new players.
            if true {
                // game_engine2.read().unwrap().player_connections.is_accepting() {
                let (player, player_connection) = player_handshake_handler(stream, grid.clone())
                    .unwrap();
                game_engine2.write().unwrap().add_player(player, player_connection);
            }
        });
    });

    thread::sleep(time::Duration::from_millis(5000));

    // @TODO: New logic for accepting/rejecting/queueing new players.
    // game_engine
    //     .write()
    //     .unwrap()
    //     .player_connections
    //     .close();
    game_engine.write()
        .unwrap()
        .player_connections
        .broadcast(Command::NewGame {});

    loop {
        game_engine.write().unwrap().ask_for_moves();
        // Advance turn.
        game_engine.write().unwrap().simulate_next_turn();
    }
}

fn player_handshake_handler(stream: TcpStream,
                            grid: Grid)
                            -> Result<(Player, PlayerConnection), ProtocolError> {
    // @TODO: Prevent memory exhaustion: stop reading from string after 1MiB.
    // @TODO @DEBUG: Need to reset this for each new message communication.

    let mut player_connection = PlayerConnection::new(stream, None)
        .expect("Could not produce new PlayerConnection.");

    player_connection.write(&Command::version()).expect("Could not write Command::version().");

    player_connection.write(&Command::Server {
            grid: grid,
            timeout: None,
        })
        .expect("Could not write Command::Server.");

    let player = match player_connection.read() {
        Ok(Command::Hello { player }) => {
            println!("Player {:?}", player);
            player
        }
        Ok(_) => {
            player_connection.write(&Command::Error {}).unwrap_or(());
            return Err(ProtocolError::UnexpectedCommand);
        }
        Err(e) => {
            return Err(e);
        }
    };
    return Ok((player, player_connection));
}
