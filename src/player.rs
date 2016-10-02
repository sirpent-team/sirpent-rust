use snake::*;

pub type PlayerName = String;

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize)]
pub struct Player {
    pub name: PlayerName,
    #[serde(skip_serializing)]
    pub secret: Option<String>,
    pub snake: Option<Snake>,
}

impl Player {
    pub fn new(name: PlayerName, secret: Option<String>, snake: Option<Snake>) -> Player {
        Player {
            name: name,
            secret: secret,
            snake: snake,
        }
    }
}
