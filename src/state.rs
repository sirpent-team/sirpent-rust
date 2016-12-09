use uuid::Uuid;
use std::collections::{HashSet, HashMap};
use rayon::prelude::*;

use grid::*;
use snake::*;
use player::*;

#[derive(Debug)]
pub struct State {
    pub game: GameState,
    pub player_conns: HashMap<PlayerName, PlayerConnection>,
    pub turn: TurnState,
}

impl State {
    pub fn new(grid: Grid) -> State {
        State {
            game: GameState {
                uuid: Uuid::new_v4(),
                grid: grid,
                players: HashMap::new(),
            },
            player_conns: HashMap::new(),
            turn: TurnState {
                turn_number: 0,
                food: HashSet::new(),
                snakes: HashMap::new(),
            },
        }
    }

    pub fn add_player(&mut self, player: Player, connection: PlayerConnection, snake: Snake) {
        let player_name = player.name.clone();
        self.game.players.insert(player_name.clone(), player);
        self.player_conns.insert(player_name.clone(), connection);
        self.turn.snakes.insert(player_name.clone(), snake);
    }

    pub fn turn(&mut self) {
        let new_turn = self.turn.clone();
    }

    fn request_moves(&mut self) -> HashMap<PlayerName, Result<Direction, MoveError>> {
        // Aggregate move responses.
        let mut moves: Vec<(PlayerName, Result<Direction, MoveError>)> =
            Vec::with_capacity(self.turn.snakes.len());

        let turn = self.turn.clone();
        self.player_conns.par_iter_mut()
            .map(|(player_name, mut player_conn)| {
                match player_conn.tell_turn(turn.clone()) {
                    Err(e) => return (player_name.clone(), Err(From::from(e))),
                    _ => {}
                };
                let move_ = player_conn.ask_next_move();
                (player_name.clone(), move_)
            })
            .collect_into(&mut moves);

        /*let living_player_names: Vec<PlayerName> = self.turn.snakes.keys().cloned().collect();
        living_player_names.par_iter()
            .map(|player_name| {
                let player_name: PlayerName = player_name.clone();
                let move_ = self.player_conns.get_mut(&player_name).unwrap().ask_next_move();
                (player_name, move_)
            })
            .collect_into(&mut moves);*/

        moves.into_iter().collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub uuid: Uuid,
    pub grid: Grid,
    pub players: HashMap<PlayerName, Player>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnState {
    pub turn_number: usize,
    pub food: HashSet<Vector>,
    pub snakes: HashMap<PlayerName, Snake>,
}
