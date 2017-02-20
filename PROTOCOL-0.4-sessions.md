### A spectator watching a game:

``` json
> {"version":{"sirpent":"0.1.1","protocol":"0.4"}}
{"register": {"desired_name": "your_players_name", "kind": "spectator"}}
> {"welcome":{"name":"your_players_name","grid":{"tiling":"hexagon","radius":25},"timeout_millis":5000}}
> {"game":{"game":{"uuid":"cf0e78d3-04e4-4b66-8191-dacafc299697","grid":{"tiling":"hexagon","radius":25},"players":["your_players_name_","your_players_name__"]}}}
> {"round":{"round":{"round_number":0,"food":[{"x":-3,"y":-15}],"eaten":{},"snakes":{"your_players_name__":{"segments":[{"x":-4,"y":19}]},"your_players_name_":{"segments":[{"x":-22,"y":21}]}},"directions":{},"casualties":{}},"game_uuid":"cf0e78d3-04e4-4b66-8191-dacafc299697"}}
> {"round":{"round":{"round_number":1,"food":[{"x":-3,"y":-15}],"eaten":{},"snakes":{"your_players_name__":{"segments":[{"x":-4,"y":18}]},"your_players_name_":{"segments":[{"x":-22,"y":20}]}},"directions":{"your_players_name_":"north","your_players_name__":"north"},"casualties":{}},"game_uuid":"cf0e78d3-04e4-4b66-8191-dacafc299697"}}
> {"round":{"round":{"round_number":2,"food":[{"x":-3,"y":-15}],"eaten":{},"snakes":{"your_players_name__":{"segments":[{"x":-4,"y":17}]},"your_players_name_":{"segments":[{"x":-22,"y":19}]}},"directions":{"your_players_name_":"north","your_players_name__":"north"},"casualties":{}},"game_uuid":"cf0e78d3-04e4-4b66-8191-dacafc299697"}}
> {"round":{"round":{"round_number":3,"food":[{"x":-3,"y":-15}],"eaten":{},"snakes":{"your_players_name__":{"segments":[{"x":-4,"y":16}]},"your_players_name_":{"segments":[{"x":-22,"y":18}]}},"directions":{"your_players_name_":"north","your_players_name__":"north"},"casualties":{}},"game_uuid":"cf0e78d3-04e4-4b66-8191-dacafc299697"}}
> {"round":{"round":{"round_number":4,"food":[{"x":-3,"y":-15}],"eaten":{},"snakes":{"your_players_name__":{"segments":[{"x":-4,"y":15}]},"your_players_name_":{"segments":[{"x":-22,"y":17}]}},"directions":{"your_players_name_":"north","your_players_name__":"north"},"casualties":{}},"game_uuid":"cf0e78d3-04e4-4b66-8191-dacafc299697"}}
> {"outcome":{"winners":["your_players_name__"],"conclusion":{"round_number":5,"food":[{"x":-3,"y":-15}],"eaten":{},"snakes":{"your_players_name__":{"segments":[{"x":-4,"y":14}]}},"directions":{"your_players_name__":"north"},"casualties":{"your_players_name_":"no_move_made"}},"game_uuid":"cf0e78d3-04e4-4b66-8191-dacafc299697"}}
```

### A player failing to send a turn in time:

``` json
> {"version":{"sirpent":"0.1.1","protocol":"0.4"}}
{"register": {"desired_name": "your_players_name", "kind": "player"}}
> {"welcome":{"name":"your_players_name_","grid":{"tiling":"hexagon","radius":25},"timeout_millis":5000}}
> {"game":{"game":{"uuid":"cf0e78d3-04e4-4b66-8191-dacafc299697","grid":{"tiling":"hexagon","radius":25},"players":["your_players_name_","your_players_name__"]}}}
> {"round":{"round":{"round_number":0,"food":[{"x":-3,"y":-15}],"eaten":{},"snakes":{"your_players_name__":{"segments":[{"x":-4,"y":19}]},"your_players_name_":{"segments":[{"x":-22,"y":21}]}},"directions":{},"casualties":{}},"game_uuid":"cf0e78d3-04e4-4b66-8191-dacafc299697"}}
{"move": {"direction": "north"}}
> {"round":{"round":{"round_number":1,"food":[{"x":-3,"y":-15}],"eaten":{},"snakes":{"your_players_name__":{"segments":[{"x":-4,"y":18}]},"your_players_name_":{"segments":[{"x":-22,"y":20}]}},"directions":{"your_players_name_":"north","your_players_name__":"north"},"casualties":{}},"game_uuid":"cf0e78d3-04e4-4b66-8191-dacafc299697"}}
{"move": {"direction": "north"}}
> {"round":{"round":{"round_number":2,"food":[{"x":-3,"y":-15}],"eaten":{},"snakes":{"your_players_name__":{"segments":[{"x":-4,"y":17}]},"your_players_name_":{"segments":[{"x":-22,"y":19}]}},"directions":{"your_players_name_":"north","your_players_name__":"north"},"casualties":{}},"game_uuid":"cf0e78d3-04e4-4b66-8191-dacafc299697"}}
{"move": {"direction": "north"}}
> {"round":{"round":{"round_number":3,"food":[{"x":-3,"y":-15}],"eaten":{},"snakes":{"your_players_name__":{"segments":[{"x":-4,"y":16}]},"your_players_name_":{"segments":[{"x":-22,"y":18}]}},"directions":{"your_players_name_":"north","your_players_name__":"north"},"casualties":{}},"game_uuid":"cf0e78d3-04e4-4b66-8191-dacafc299697"}}
{"move": {"direction": "north"}}
> {"round":{"round":{"round_number":4,"food":[{"x":-3,"y":-15}],"eaten":{},"snakes":{"your_players_name__":{"segments":[{"x":-4,"y":15}]},"your_players_name_":{"segments":[{"x":-22,"y":17}]}},"directions":{"your_players_name_":"north","your_players_name__":"north"},"casualties":{}},"game_uuid":"cf0e78d3-04e4-4b66-8191-dacafc299697"}}
Connection closed by foreign host.
```

### A player winning a game:

``` json
> {"version":{"sirpent":"0.1.1","protocol":"0.4"}}
{"register": {"desired_name": "your_players_name", "kind": "player"}}
> {"welcome":{"name":"your_players_name__","grid":{"tiling":"hexagon","radius":25},"timeout_millis":5000}}
> {"game":{"game":{"uuid":"cf0e78d3-04e4-4b66-8191-dacafc299697","grid":{"tiling":"hexagon","radius":25},"players":["your_players_name_","your_players_name__"]}}}
> {"round":{"round":{"round_number":0,"food":[{"x":-3,"y":-15}],"eaten":{},"snakes":{"your_players_name__":{"segments":[{"x":-4,"y":19}]},"your_players_name_":{"segments":[{"x":-22,"y":21}]}},"directions":{},"casualties":{}},"game_uuid":"cf0e78d3-04e4-4b66-8191-dacafc299697"}}
{"move": {"direction": "north"}}
> {"round":{"round":{"round_number":1,"food":[{"x":-3,"y":-15}],"eaten":{},"snakes":{"your_players_name__":{"segments":[{"x":-4,"y":18}]},"your_players_name_":{"segments":[{"x":-22,"y":20}]}},"directions":{"your_players_name_":"north","your_players_name__":"north"},"casualties":{}},"game_uuid":"cf0e78d3-04e4-4b66-8191-dacafc299697"}}
{"move": {"direction": "north"}}
> {"round":{"round":{"round_number":2,"food":[{"x":-3,"y":-15}],"eaten":{},"snakes":{"your_players_name__":{"segments":[{"x":-4,"y":17}]},"your_players_name_":{"segments":[{"x":-22,"y":19}]}},"directions":{"your_players_name_":"north","your_players_name__":"north"},"casualties":{}},"game_uuid":"cf0e78d3-04e4-4b66-8191-dacafc299697"}}
{"move": {"direction": "north"}}
> {"round":{"round":{"round_number":3,"food":[{"x":-3,"y":-15}],"eaten":{},"snakes":{"your_players_name__":{"segments":[{"x":-4,"y":16}]},"your_players_name_":{"segments":[{"x":-22,"y":18}]}},"directions":{"your_players_name_":"north","your_players_name__":"north"},"casualties":{}},"game_uuid":"cf0e78d3-04e4-4b66-8191-dacafc299697"}}
{"move": {"direction": "north"}}
> {"round":{"round":{"round_number":4,"food":[{"x":-3,"y":-15}],"eaten":{},"snakes":{"your_players_name__":{"segments":[{"x":-4,"y":15}]},"your_players_name_":{"segments":[{"x":-22,"y":17}]}},"directions":{"your_players_name_":"north","your_players_name__":"north"},"casualties":{}},"game_uuid":"cf0e78d3-04e4-4b66-8191-dacafc299697"}}
{"move": {"direction": "north"}}
> {"outcome":{"winners":["your_players_name__"],"conclusion":{"round_number":5,"food":[{"x":-3,"y":-15}],"eaten":{},"snakes":{"your_players_name__":{"segments":[{"x":-4,"y":14}]}},"directions":{"your_players_name__":"north"},"casualties":{"your_players_name_":"no_move_made"}},"game_uuid":"cf0e78d3-04e4-4b66-8191-dacafc299697"}}
> {"game":{"game":{"uuid":"cf0e78d3-04e4-4b66-8191-dacafc299697","grid":{"tiling":"hexagon","radius":25},"players":["your_players_name_","your_players_name__"]}}}
```
