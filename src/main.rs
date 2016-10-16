#![feature(question_mark)]

extern crate ansi_term;
extern crate sirpent;
extern crate rand;
extern crate uuid;
#[macro_use(chan_select)]
extern crate chan;

use ansi_term::Colour::*;
use uuid::Uuid;
use std::collections::HashMap;
use std::net::TcpStream;
use std::thread;
use std::str;
use std::time;
use chan::{Receiver, Sender};

use sirpent::*;

fn main() {
    println!("{}", Yellow.bold().paint("Sirpent"));

    let mut game = Game {
        uuid: Uuid::new_v4(),
        grid: Grid::hexagon(15),
        players: HashMap::new(),
        food: Vector::hexagon(9, 13),
    };

    let snake = Snake::new(vec![Vector::hexagon(3, 8)]);
    game.add_player(Player::new("abserde".to_string(), Some(snake)));

    // -----------------------------------------------------------------------

    let (game_tx, game_rx) = chan::async();
    let (direction_tx, direction_rx) = chan::async();

    let game_grid = game.grid.clone();
    thread::spawn(move || {
        let plain_server = SirpentServer::plain("0.0.0.0:5513").unwrap();
        plain_server.listen(move |stream: TcpStream| {
                                server_handler(stream,
                                               game_grid.clone(),
                                               game_rx.clone(),
                                               direction_tx.clone());
                            },
                            None);
    });

    thread::spawn(move || {
        loop {
            game_tx.send(game.clone());

            let (player, direction) = direction_rx.recv().unwrap();
            println!("direction_rx: {:?} {:?}", player, direction);
        }
    });

    // -----------------------------------------------------------------------

    loop {
        thread::sleep(time::Duration::from_millis(500));
    }
}

fn server_handler(stream: TcpStream,
                  grid: Grid,
                  game_rx: Receiver<Game>,
                  direction_tx: Sender<(Player, Direction)>) {
    thread::spawn(move || {
        // Prevent memory exhaustion: stop reading from string after 1MiB.
        // @TODO @DEBUG: Need to reset this for each new message communication.
        // let mut take = reader.clone().take(0xfffff);

        let mut player_connection = PlayerConnection::new(stream)
            .expect("Could not produce new PlayerConnection.");

        player_connection.write(&Command::version()).expect("Could not write Command::version().");

        player_connection.write(&Command::Server {
                grid: grid,
                timeout: None,
            })
            .expect("Could not write Command::Server.");

        let player = match player_connection.read()
            .expect("Could not read anything; expected Command::Hello.") {
            Command::Hello { player, secret } => {
                println!("Player {:?} with secret {:?}", player, secret);
                player
            }
            Command::Quit => {
                println!("QUIT");
                return;
            }
            command => {
                player_connection.write(&Command::Error).unwrap_or(());
                panic!(format!("Unexpected {:?}.", command));
            }
        };

        player_connection.write(&Command::NewGame).expect("Could not write Command::NewGame.");

        loop {
            let game = game_rx.recv().unwrap();

            player_connection.write(&Command::Turn { game: game.clone() })
                .expect("Could not write Command::Turn.");

            player_connection.write(&Command::MakeAMove)
                .expect("Could not write Command::MakeAMove.");

            match player_connection.read()
                .expect("Could not read anything; expected Command::Move.") {
                Command::Move { direction } => {
                    println!("{:?}", Command::Move { direction: direction });
                    direction_tx.send((player.clone(), direction));
                }
                Command::Quit => {
                    println!("QUIT");
                    return;
                }
                command => {
                    player_connection.write(&Command::Error).unwrap_or(());
                    panic!(format!("Unexpected {:?}.", command));
                }
            }
        }
    });
}
