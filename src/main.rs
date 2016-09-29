#![feature(question_mark)]

extern crate ansi_term;
extern crate sirpent;
extern crate rand;
extern crate uuid;

use ansi_term::Colour::*;
use uuid::Uuid;
use std::collections::HashMap;
use std::net::TcpStream;
use std::thread;
use std::str;
use std::time;

use sirpent::*;

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

    // -----------------------------------------------------------------------

    thread::spawn(move || {
        let plain_server = SirpentServer::plain("0.0.0.0:5513").unwrap();
        plain_server.listen(move |stream: TcpStream| {
                                server_handler::<HexagonVector>(stream, game.clone())
                            },
                            None)
    });

    // -----------------------------------------------------------------------

    thread::sleep(time::Duration::from_millis(500));
    thread::spawn(move || client::<HexagonVector>());

    loop {
        thread::sleep(time::Duration::from_millis(500));
    }
}

fn server_handler<V: Vector>(stream: TcpStream, game: Game<V>) {
    // Prevent memory exhaustion: stop reading from string after 1MiB.
    // @TODO @DEBUG: Need to reset this for each new message communication.
    // let mut take = reader.clone().take(0xfffff);

    let mut player_connection = PlayerConnection::<V>::new(stream).unwrap();

    player_connection.write(&Command::version()).unwrap();

    player_connection.write(&Command::Server {
            world: None,
            timeout: None,
        })
        .unwrap();

    match player_connection.read().unwrap() {
        Command::Hello { player } => println!("{:?}", player),
        Command::Quit => {
            println!("QUIT");
            return;
        }
        command => {
            player_connection.write(&Command::Error).unwrap_or(());
            panic!(format!("Unexpected {:?}.", command));
        }
    }

    player_connection.write(&Command::NewGame).unwrap();

    player_connection.write(&Command::Turn { game: game })
        .unwrap();

    player_connection.write(&Command::MakeAMove).unwrap();

    match player_connection.read().unwrap() {
        Command::Move { direction } => {
            println!("{:?}", Command::Move::<V> { direction: direction })
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

pub fn client<V: Vector>()
    where <V as sirpent::Vector>::Direction: 'static
{
    let stream = TcpStream::connect("127.0.0.1:5513").unwrap();
    let mut player_connection = PlayerConnection::<V>::new(stream).unwrap();

    let r = player_connection.read().unwrap();
    match r {
        Command::Version { sirpent, protocol } => {
            println!("{:?}",
                     Command::Version::<V> {
                         sirpent: sirpent,
                         protocol: protocol,
                     })
        }
        command => {
            player_connection.write(&Command::Error).unwrap_or(());
            panic!(format!("Unexpected {:?}.", command));
        }
    }

    let r = player_connection.read().unwrap();
    match r {
        Command::Server { world, timeout } => {
            println!("{:?}",
                     Command::Server::<V> {
                         world: world,
                         timeout: timeout,
                     })
        }
        command => {
            player_connection.write(&Command::Error).unwrap_or(());
            panic!(format!("Unexpected {:?}.", command));
        }
    }

    player_connection.write(&Command::Hello {
            player: Player {
                name: "daenerys".to_string(),
                secret: Some("DeagOLmol3105764438410301265454621913800982laskhdasdj".to_string()),
                snake_uuid: None,
            },
        })
        .unwrap();

    match player_connection.read().unwrap() {
        Command::NewGame => println!("{:?}", Command::NewGame::<V>),
        command => {
            player_connection.write(&Command::Error).unwrap_or(());
            panic!(format!("Unexpected {:?}.", command));
        }
    }

    let mut turn_game: Game<V> = match player_connection.read().unwrap() {
        Command::Turn { game } => {
            println!("{:?}", Command::Turn::<V> { game: game.clone() });
            game
        }
        command => {
            player_connection.write(&Command::Error).unwrap_or(());
            panic!(format!("Unexpected {:?}.", command));
        }
    };

    loop {
        match player_connection.read().unwrap() {
            Command::MakeAMove => println!("{:?}", Command::MakeAMove::<V>),
            command => {
                player_connection.write(&Command::Error).unwrap_or(());
                panic!(format!("Unexpected {:?}.", command));
            }
        }

        player_connection.write(&Command::Move { direction: V::Direction::variants()[0] })
            .unwrap();

        match player_connection.read().unwrap() {
            Command::TimedOut => {
                println!("{:?}", Command::TimedOut::<V>);
                return;
            }
            Command::Died => {
                println!("{:?}", Command::Died::<V>);
                return;
            }
            Command::Won => {
                println!("{:?}", Command::Won::<V>);
                return;
            }
            Command::Turn { game } => {
                println!("{:?}", Command::Turn::<V> { game: game.clone() });
                turn_game = game;
                continue;
            }
            command => {
                player_connection.write(&Command::Error).unwrap_or(());
                panic!(format!("Unexpected {:?}.", command));
            }
        }
    }
}
