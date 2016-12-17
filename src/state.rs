use uuid::Uuid;
use std::collections::{HashSet, HashMap};
use rayon::prelude::*;
use std::error::Error;

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
                      connection: PlayerConnection,
                      snake: Snake)
                      -> ProtocolResult<PlayerName> {
        // Get desired name of this player.
        let mut player_agent = PlayerAgent::new(connection);
        player_agent.next(PlayerEvent::Versioning);
        let desired_player_name = match player_agent.next(PlayerEvent::Identifying) {
            Some(PlayerState::Identify { ref desired_player_name }) => desired_player_name.clone(),
            None => return Err(player_agent.state.unwrap_err()),
            _ => unreachable!()
        };

        // Find the final name of this player by deduping.
        let mut player_name = desired_player_name.clone();
        while self.game.players.contains_key(&player_name) {
            player_name.push('_');
        }
        let player = Player::new(player_name.clone());

        // Welcome the player.
        player_agent.next(PlayerEvent::Welcoming {
            player_name: player_name.clone(),
            grid: self.game.grid
        });

        // Check player connection is ready to start games.
        match player_agent.state {
            Ok(PlayerState::Ready) => {},
            Ok(_) => unreachable!(),
            Err(e) => return Err(e)
        }

        // Register player agent, player data and player snake.
        self.player_agents.insert(player_name.clone(), player_agent);
        self.game.players.insert(player_name.clone(), player);
        self.turn.snakes.insert(player_name.clone(), snake);

        // Return the final player name.
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

    pub fn request_moves(&mut self) -> HashMap<PlayerName, Move> {
        // 1. Tell players about the new turn.
        // 2. Get a move from all living players.
        let turn = self.turn.clone();
        self.player_agents
            .par_iter_mut()
            .for_each(|(_, mut player_agent)| {
                player_agent.next(PlayerEvent::NewTurn { turn: turn.clone() });
                player_agent.next(PlayerEvent::Move);
            });

        // Recover direction of move from all living players.
        let mut moves: HashMap<PlayerName, Move> = HashMap::new();
        for (player_name, player_agent) in self.player_agents.iter() {
            let move_ = match player_agent.state {
                Ok(PlayerState::Moving { direction }) => Ok(direction),
                Err(ref e) => {
                    let cause_of_death = CauseOfDeath::NoMoveMade(e.description().to_string());
                    Err(cause_of_death)
                }
                _ => unreachable!(),
            };
            moves.insert(player_name.clone(), move_);
        }
        moves
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
