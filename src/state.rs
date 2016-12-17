use uuid::Uuid;
use std::collections::{HashSet, HashMap};
use rayon::prelude::*;

use grid::*;
use snake::*;
use player::*;
use protocol::*;

#[derive(Debug)]
pub struct State {
    pub game: GameState,
    pub player_agents: HashMap<PlayerName, PlayerAgent>,
    pub turn: TurnState,
}

impl State {
    pub fn new(grid: Grid) -> State {
        State {
            game: GameState::new(grid),
            player_agents: HashMap::new(),
            turn: TurnState::new(),
        }
    }

    pub fn add_player(&mut self,
                      mut player: Player,
                      mut connection: PlayerConnection,
                      snake: Snake)
                      -> ProtocolResult<PlayerName> {
        // Dedupe player name.
        while self.game.players.contains_key(&player.name) {
            player.name.push('_');
        }

        let player_name = player.name.clone();
        // self.game.players.insert(player_name.clone(), player);
        // connection.identified(player_name.clone())?;
        // self.player_conns.insert(player_name.clone(), connection);
        // self.turn.snakes.insert(player_name.clone(), snake);

        Ok(player_name)
    }

    pub fn new_game(&mut self) {
        let game = self.game.clone();
        self.player_agents
            .par_iter_mut()
            .for_each(|(_, mut player_agent)| {
                player_agent.next(PlayerEvent::NewGame { game: game.clone() });
            });
    }

    pub fn request_moves(&mut self) -> HashMap<PlayerName, Direction> {
        // Tell players about the new turn.
        let turn = self.turn.clone();
        self.player_agents
            .par_iter_mut()
            .for_each(|(_, mut player_agent)| {
                player_agent.next(PlayerEvent::NewTurn { turn: turn.clone() });
            });

        // Get a move from all living players.
        self.player_agents
            .par_iter_mut()
            .filter(|&(_, ref player_agent)| player_agent.state.is_ok())
            .for_each(|(_, mut player_agent)| {
                player_agent.next(PlayerEvent::Move);
            });

        // Recover direction of move from all living players.
        self.player_agents
            .iter()
            .filter_map(|(player_name, player_agent)| {
                match player_agent.state {
                    Ok(PlayerState::Moving { direction }) => Some((player_name.clone(), direction)),
                    _ => None,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameState {
    pub uuid: Uuid,
    pub grid: Grid,
    pub players: HashMap<PlayerName, Player>,
}

impl GameState {
    pub fn new(grid: Grid) -> GameState {
        GameState {
            uuid: Uuid::new_v4(),
            grid: grid,
            players: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TurnState {
    pub turn_number: usize,
    pub food: HashSet<Vector>,
    pub eaten: HashMap<PlayerName, Vector>,
    pub snakes: HashMap<PlayerName, Snake>,
    pub directions: HashMap<PlayerName, Direction>,
    pub casualties: HashMap<PlayerName, (CauseOfDeath, Snake)>,
}

impl TurnState {
    pub fn new() -> TurnState {
        TurnState {
            turn_number: 0,
            food: HashSet::new(),
            eaten: HashMap::new(),
            snakes: HashMap::new(),
            directions: HashMap::new(),
            casualties: HashMap::new(),
        }
    }
}
