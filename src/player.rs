pub type PlayerName = String;

pub type PlayerBox = Box<Player>;

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize)]
pub struct Player {
    pub name: PlayerName,
}

impl Player {
    pub fn new(name: PlayerName) -> Player {
        Player { name: name }
    }
}
