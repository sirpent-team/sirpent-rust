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
    let (new_player_tx, new_player_rx) = chan::async();

    let game_grid = game.grid.clone();
    thread::spawn(move || {
        let plain_server = SirpentServer::plain("0.0.0.0:5513").unwrap();
        plain_server.listen(move |stream: TcpStream| {
            player_handshake_handler(stream, game_grid.clone(), new_player_tx.clone());
        });
    });

    thread::spawn(move || {
        let mut game = game.clone();

        while game.players.len() < 3 {
            let (mut player, player_connection) = new_player_rx.recv().unwrap();
            player.snake = Some(Snake::new(vec![Vector::hexagon(game.players.len() as isize,
                                                                game.players.len() as isize)]));
            let final_player_name = game.add_player(player);
            player_game_handler(player_connection,
                                final_player_name,
                                game_rx.clone(),
                                direction_tx.clone());
        }

        loop {
            for _ in 0..game.players.len() {
                game_tx.send(game.clone());

                let (player_name, direction) = direction_rx.recv()
                    .expect("Did not recieve (PlayerName,Option<Direction>) across direction_rx.");
                if direction.is_none() {
                    panic!("No direction!");
                }
                let mut p = game.players.get_mut(&player_name);
                let mut player = p.as_mut().expect("direction_rx specified unknown player.");
                let mut snake = player.snake
                    .as_mut()
                    .expect("direction_rx specified player with no snake.")
                    .clone();
                snake.step_in_direction(direction.expect("direction_rx specified None direction."));
                player.snake = Some(snake);
                println!("player.name={} snake={:?}", player.name, player.snake);
            }
        }
    });

    // -----------------------------------------------------------------------

    loop {
        thread::sleep(time::Duration::from_millis(500));
    }
}

fn player_handshake_handler(stream: TcpStream,
                            grid: Grid,
                            new_player_tx: Sender<(Player, PlayerConnection)>) {
    thread::spawn(move || {
        // Prevent memory exhaustion: stop reading from string after 1MiB.
        // @TODO @DEBUG: Need to reset this for each new message communication.
        // let mut take = reader.clone().take(0xfffff);

        let mut player_connection = PlayerConnection::new(stream, None)
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
        new_player_tx.send((player.clone(), player_connection));
    });
}

fn player_game_handler(mut player_connection: PlayerConnection,
                       player_name: PlayerName,
                       game_rx: Receiver<Game>,
                       direction_tx: Sender<(PlayerName, Option<Direction>)>) {
    thread::spawn(move || {
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
                    direction_tx.send((player_name.clone(), Some(direction)));
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
