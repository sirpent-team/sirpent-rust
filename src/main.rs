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
use std::io::{Error, ErrorKind};
use std::net::TcpStream;
use std::io::Result;
use std::sync::{Arc, RwLock};
use std::ops::Deref;
use rand::os::OsRng;

use sirpent::*;

fn main() {
    println!("{}", Yellow.bold().paint("Sirpent"));

    let osrng = OsRng::new().unwrap();
    let grid = Grid { radius: 15 };
    let food0 = grid.random_cell(osrng);

    let game_state = Arc::new(RwLock::new(GameState::new(grid, true)));
    game_state.write().unwrap().context.food.insert(food0);

    // let snake = Snake::new(vec![Vector { x: 3, y: 8 }]);
    game_state.write().unwrap().add_player(Player::new("abserde".to_string()));

    // -----------------------------------------------------------------------

    let player_connections = Arc::new(RwLock::new(PlayerConnections::new()));

    let game_state2 = game_state.clone();
    let player_connections2 = player_connections.clone();
    thread::spawn(move || {
        let plain_server = SirpentServer::plain("0.0.0.0:5513").unwrap();
        plain_server.listen(move |stream: TcpStream| {
            if player_connections2.read().unwrap().is_accepting() {
                let (player, player_connection) = player_handshake_handler(stream, grid.clone())
                    .unwrap();
                player_connections2.write()
                    .unwrap()
                    .add_player(player.name.clone(), player_connection);
                game_state2.write().unwrap().add_player(player);
            }
        });
    });

    thread::sleep(time::Duration::from_millis(5000));

    player_connections.write()
        .unwrap()
        .close();
    player_connections.write()
        .unwrap()
        .broadcast(Command::NewGame {});

    loop {
        // Issue notifications of Turn to each player.
        {
            let game_state_read = game_state.read().unwrap();

            // Print result of previous turn (here so 0th is printed).
            println!("{:?} {:?}",
                     game_state_read.turn_number,
                     game_state_read.deref());
            println!("removed snakes {:?}", game_state_read.snakes_to_remove);
            // @DEBUG: Wait before advancing.
            thread::sleep(time::Duration::from_millis(500));

            // @TODO: Broadcast game state and recieve moves in parallel.
            // Broadcast request for moves.
            let turn_command = Command::Turn { game: game_state_read.context.clone() };
            player_connections.write()
                .unwrap()
                .broadcast(turn_command);
            player_connections.write().unwrap().broadcast(Command::MakeAMove {});
        }

        // Aggregate move responses.
        for (player_name, command_result) in player_connections.write().unwrap().collect() {
            if let Ok(Command::Move { direction }) = command_result {
                game_state.write().unwrap().snake_plans.insert(player_name, Ok(direction));
            }
        }

        // Advance turn.
        game_state.write().unwrap().simulate_next_turn();
    }
}

fn player_handshake_handler(stream: TcpStream, grid: Grid) -> Result<(Player, PlayerConnection)> {
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
        Ok(Command::Hello { player, secret }) => {
            println!("Player {:?} with secret {:?}", player, secret);
            player
        }
        Ok(command) => {
            player_connection.write(&Command::Error {}).unwrap_or(());
            return Err(Error::new(ErrorKind::Other,
                                  format!("Unexpected command {:?}", command)));
        }
        Err(e) => {
            return Err(e);
        }
    };
    return Ok((player, player_connection));
}
