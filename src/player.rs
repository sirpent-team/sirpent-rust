use snake::*;

pub type PlayerName = String;

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize)]
pub struct Player {
    pub name: PlayerName,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snake: Option<Snake>,
}

impl Player {
    pub fn new(name: PlayerName, snake: Option<Snake>) -> Player {
        Player {
            name: name,
            snake: snake,
        }
    }
}
