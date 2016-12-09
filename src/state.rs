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
                eaten: HashMap::new(),
                snakes: HashMap::new(),
                directions: HashMap::new(),
                casualties: HashMap::new(),
            },
        }
    }

    pub fn add_player(&mut self,
                      mut player: Player,
                      connection: PlayerConnection,
                      snake: Snake)
                      -> PlayerName {
        // Dedupe player name.
        while self.game.players.contains_key(&player.name) {
            player.name.push('_');
        }

        let player_name = player.name.clone();
        self.game.players.insert(player_name.clone(), player);
        self.player_conns.insert(player_name.clone(), connection);
        self.turn.snakes.insert(player_name.clone(), snake);

        player_name
    }

    pub fn request_moves(&mut self) -> HashMap<PlayerName, Move> {
        // Aggregate move responses.
        let mut moves: Vec<Option<Move>> = Vec::with_capacity(self.turn.snakes.len());

        let turn = self.turn.clone();
        self.player_conns
            .par_iter_mut()
            .map(|(player_name, mut player_conn)| {
                let player_name = player_name.clone();
                if turn.snakes.contains_key(&player_name) {
                    // If player alive, try sending turn. If that succeeds, try and read a move.
                    match player_conn.tell_turn(turn.clone()) {
                        Ok(_) => Some(player_conn.ask_next_move()),
                        Err(e) => Some(Err(e)),
                    }
                } else {
                    // If player is dead, send turn but ignore errors.
                    // @TODO: If errors then close connection?
                    match player_conn.tell_turn(turn.clone()) {
                        _ => None,
                    }
                }
            })
            .collect_into(&mut moves);

        // For unclear reasons, par_iter's filter_map does not have collect/collect_into defined.
        self.game
            .players
            .keys()
            .cloned()
            .zip(moves.into_iter())
            .filter_map(|(player_name, maybe_move)| {
                match maybe_move {
                    Some(move_) => Some((player_name, move_)),
                    None => None,
                }
            })
            .collect()
    }

    pub fn living_players(&self) -> HashMap<PlayerName, (Player, Snake)> {
        self.turn
            .snakes
            .iter()
            .map(|(player_name, snake)| {
                let player = self.game.players[player_name].clone();
                (player_name.clone(), (player, snake.clone()))
            })
            .collect()
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
    pub eaten: HashMap<PlayerName, Vector>,
    pub snakes: HashMap<PlayerName, Snake>,
    pub directions: HashMap<PlayerName, Direction>,
    pub casualties: HashMap<PlayerName, (CauseOfDeath, Snake)>,
}
