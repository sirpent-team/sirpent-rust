extern crate ansi_term;
extern crate sirpent;
extern crate rand;
extern crate uuid;
#[macro_use(chan_select)]
extern crate chan;
extern crate rayon;

use ansi_term::Colour::*;
use uuid::Uuid;
use std::collections::{HashMap, HashSet};
use std::net::TcpStream;
use std::thread;
use std::str;
use std::time;
use chan::{Receiver, Sender};
use std::io::{Error, ErrorKind};
use std::sync::Arc;
use std::iter::FromIterator;
use rayon::prelude::*;
use std::cell::RefCell;

use sirpent::*;

fn main() {
    println!("{}", Yellow.bold().paint("Sirpent"));

    let mut game = Game {
        uuid: Uuid::new_v4(),
        grid: Grid { radius: 15 },
        players: HashMap::new(),
        food: Vector { x: 9, y: 13 },
    };

    let snake = Snake::new(vec![Vector { x: 3, y: 8 }]);
    game.add_player(Player::new("abserde".to_string(), Some(snake)));

    // -----------------------------------------------------------------------

    // -----------------------------------------------------------------------

    loop {
        thread::sleep(time::Duration::from_millis(500));
    }
}

type PlayerBox = Box<Player>;
pub struct GameContext {
    food: HashSet<Vector>,
    snakes: HashMap<PlayerName, Snake>,
}
pub struct GameState {
    uuid: Uuid,
    grid: Grid,
    players: HashMap<PlayerName, PlayerBox>,

    context: GameContext,
    snakes_to_create: HashSet<PlayerName>,
    snake_plans: HashMap<PlayerName, Direction>,
    snakes_to_remove: HashSet<PlayerName>,

    turn_number: u32,
    debug: bool,
}

impl GameState {
    fn new(grid: Grid, debug: bool) -> GameState {
        GameState{
            uuid: Uuid::new_v4(),
            grid: grid,
            players: HashMap::new(),
            context: GameContext{
                food: HashSet::new(),
                snakes: HashMap::new()
            },
            snakes_to_create: HashSet::new(),
            snake_plans: HashMap::new(),
            snakes_to_remove: HashSet::new(),
            turn_number: 0,
            debug: debug
        }
    }

    fn add_player(&mut self, player: Player) {
        let player_box = Box::new(player.clone());
        self.snakes_to_create.insert(player_box.name.clone());
        self.players.insert(player_box.name.clone(), player_box);
    }

    fn simulate_next_turn(&mut self) {
        if self.debug {
            println!("Simulating next turn");
        }

        // Create new snakes.
        // @TODO: Don't put a snake where a snake already is.
        for player_name in self.snakes_to_create.iter() {
            // @TODO: Use self.grid.random_cell()
            let snake = Snake::new(vec![]);
            self.context.snakes.insert(player_name.clone(), snake);
        }

        // Apply movement and remove unmoved nskaes.
        for (player_name, snake) in self.context.snakes.iter_mut() {
            if self.snake_plans.contains_key(player_name) {
                let plan = self.snake_plans.get(player_name).unwrap();
                snake.step_in_direction(*plan);
            } else {
                // Snakes which weren't moved turn into food and die.
                self.context.food.extend(snake.segments.iter());
                self.snakes_to_remove.insert(player_name.clone());
            }
        }
        for player_name in self.snakes_to_remove.drain() {
            self.context.snakes.remove(&player_name);
        }

        // Detect collisions with food.
        for (player_name, snake) in self.context.snakes.iter_mut() {
            if self.context.food.contains(&snake.segments[0]) {
                snake.grow();
            }
        }

        // Detect collisions with snakes and remove colliding snakes.
        for (player_name, snake) in self.context.snakes.iter() {
            for (_, snake2) in self.context.snakes.iter() {
                if (snake.has_collided_into(snake2)) {
                    self.context.food.extend(snake.segments.iter());
                    self.snakes_to_remove.insert(player_name.clone());
                    break;
                }
            }
        }
        for player_name in self.snakes_to_remove.drain() {
            self.context.snakes.remove(&player_name);
        }

        self.turn_number += 1;
    }
}

/*
fn game_manager(mut g: Arc<RefCell<Game>>, mut connections: HashMap<PlayerName, PlayerConnection>) {
    // let mut game: Arc<Game> = Arc::new(game);

    let mut game = g.borrow_mut();

    connections
        .par_iter()
        .map(|(player_name, mut player_connection)| player_connection.write(&Command::NewGame));

    let gp = game.players.clone();
    let commands = gp
        .par_iter()
        .map(|(player_name, player)| {
            let mut player_connection = connections.get_mut(player_name).unwrap();
            let command = player_connection.write(&Command::Turn { game: game.clone() })
                .and_then(|_| player_connection.write(&Command::MakeAMove))
                .and_then(|_| player_connection.read())
                .and_then(|command| {
                    match command {
                        Command::Move { direction } => {
                            // println!("{:?}", Command::Move { direction: direction });
                            // direction_tx.send((player_name.clone(), Some(direction)));
                            Ok(direction)
                        }
                        command => {
                            player_connection.write(&Command::Error).unwrap_or(());
                            Err(Error::new(ErrorKind::Other,
                                           format!("Unexpected command {:?}", command)))
                        }
                    }
                });
            (player_name, command)
        });

    let snakes = commands.map(|(player_name, direction)| {
        match direction {
            Ok(direction) => {
                let mut snake = game.players.get(player_name).unwrap().snake.clone().unwrap();
                snake.step_in_direction(direction);
                (player_name.clone(), Some(snake))
            },
            Err(err) => {
                println!("Player {:?} move error: {:?}", player_name.clone(), err);
                (player_name.clone(), None)
            }
        }
    });

    snakes.map(|(player_name, snake)| {
        game.players.get_mut(&player_name).unwrap().snake = snake;
    });
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
                       game: &Game,
                       direction_tx: Sender<Option<Direction>>) {
    loop {
        let command = player_connection.write(&Command::Turn { game: game.clone() })
            .and_then(|_| player_connection.write(&Command::MakeAMove))
            .and_then(|_| player_connection.read())
            .and_then(|command| {
                match command {
                    Command::Move { direction } => {
                        // println!("{:?}", Command::Move { direction: direction });
                        // direction_tx.send((player_name.clone(), Some(direction)));
                        Ok(direction)
                    }
                    command => {
                        player_connection.write(&Command::Error).unwrap_or(());
                        Err(Error::new(ErrorKind::Other,
                                       format!("Unexpected command {:?}", command)))
                    }
                }
            });
        direction_tx.send(command.ok());
    }
}
*/
