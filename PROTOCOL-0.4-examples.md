```
# ======== SERVER COMMS MODEL ========= #
#     ↑↓      PLAYER         SPECTATOR  #
# 01  T       version        version    #
# 02   R      register       register   #
# 03  T       welcome        welcome    #
# -------------  [...]  --------------- #
# 04  T       game           game       #
# 05  T       round          round      #
# 06   R      move           move       #
# --------  JMP 05 if ongoing  -------- #
# 07  T       outcome        outcome    #
# -------------  JMP 04  -------------- #
# ===================================== #

# ======== SERVER COMMS MODEL ========= #
#     ↑↓      PLAYER         SPECTATOR  #
# 01  R       version        version    #
# 02   T      register       register   #
# 03  R       welcome        welcome    #
# -------------  [...]  --------------- #
# 04  R       game           game       #
# 05  R       round          round      #
# 06   T      move           move       #
# -----------  CAN JMP 05  ------------ #
# 07  R       outcome        outcome    #
# -------------  JMP 04  -------------- #
# ===================================== #
```

{"kind": "register", "data": {"desired_name": "play", "kind": "player"}}
{"kind": "move", "data": {"direction": "north"}}
{"kind": "register", "data": {"desired_name": "spectate", "kind": "spectator"}}

``` json
{"kind": "version", "data": {"sirpent": "X.X.X", "protocol": "0.4"}}
{"kind": "register", "data": {"desired_name": "your_players_name", "kind": "player"}}
{"kind": "register", "data": {"desired_name": "your_players_name", "kind": "spectator"}}
{"kind": "welcome", "data": {"name": "your_players_name_", "grid": _, "timeout_millis": 5000}}
{"kind": "game", "data": {"game": _}}
{"kind": "round", "data": {"round": _, "game_uuid": "123e4567-e89b-12d3-a456-426655440000"}}
{"kind": "move", "data": {"direction": "north"}}
{"kind": "outcome", "data": {"winners": ["player1"], "conclusion": _, "game_uuid": "123e4567-e89b-12d3-a456-426655440000"}}
```

``` json
GRID CONFIG: `welcome.grid` and `game.game.grid`
{
  "tiling": "hexagon",
  "radius": 25
}

GAME STATE: `game.game`
{
  "uuid": "2e44d843-a320-41ae-b00d-c524275c1590",
  "grid": {
    "tiling": "hexagon",
    "radius": 25
  },
  "players": [
    "your_players_name__",
    "your_players_name___"
  ]
}

ROUND STATE: `round.round` and `outcome.conclusion`
{
  "round_number": 0,
  "food": [{"x": -11, "y": 2}],
  "eaten": {},
  "snakes": {
    "living_player_1": {
      "segments": [{"x": 7, "y": 3}]
    },
    "living_player_2": {
      "segments": [{"x": -10, "y": 16}]
    }
  },
  "directions": {
    "living_player_1": "north",
    "living_player_2": "northeast",
    "dead_player_1": "southwest"
  },
  "casualties": {
    "dead_player_1": "no_move_made",
    "dead_player_2": "collided_with_snake",
    "dead_player_3": "collided_with_bounds"
  }
}
```
