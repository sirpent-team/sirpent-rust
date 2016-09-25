use uuid::Uuid;

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize)]
pub struct Player {
    pub name: String,
    #[serde(skip_serializing)]
    pub secret: Option<String>,
    pub snake_uuid: Uuid,
}

impl Player {
    pub fn new(name: String, secret: Option<String>, snake_uuid: Uuid) -> Player {
        Player {
            name: name,
            secret: secret,
            snake_uuid: snake_uuid,
        }
    }
}
