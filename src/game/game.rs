use std::sync::Arc;
use std::time::Instant;

use riker::actor::{Actor, ActorRef, BasicActorRef};
use riker::actors::{ActorFactoryArgs, ActorRefFactory, Context, Sender, Strategy, Tell};
use uuid::Uuid;

use crate::data::data::Data;
use crate::data::id::Id;
use crate::game::map::{Map, RenderContext};
use crate::game::ticking::MAX_ALLOWED_TICK_INTERVAL;
use crate::game::tile::{Tile, TileCoord, TileMsg};
use crate::util::init::InitData;

#[derive(Debug, Clone)]
pub struct Ticked;



#[derive(Debug, Clone, Copy)]
pub struct GameState {
    pub tick_count: usize,
}

#[derive(Debug, Clone)]
pub struct Game {
    tick_count: usize,

    map: Arc<Map>,
}

impl ActorFactoryArgs<Arc<Map>> for Game { // TODO dont clone Map
    fn create_args(args: Arc<Map>) -> Self {
        Self::new(args)
    }
}

#[derive(Debug, Clone)]
pub enum GameMsg {
    Tick {
        init_data: Arc<InitData>,
    },
    RenderInfoRequest {
        context: RenderContext,
    },
    PlaceTile {
        coord: TileCoord,
        id: Id,
        none: Id
    },
    GetTile(TileCoord),
    SendMsgToTile(TileCoord, TileMsg),
}

impl Actor for Game {
    type Msg = GameMsg;

    fn supervisor_strategy(&self) -> Strategy {
        Strategy::Stop
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        let myself = Some(ctx.myself().into());

        match msg {
            GameMsg::Tick { init_data } => {
                self.tick(init_data);
            }
            GameMsg::RenderInfoRequest { context } => {
                let render_info = self.map.render_info(&context);

                sender.inspect(|v| v.try_tell(render_info, myself).unwrap());
            }
            GameMsg::PlaceTile { coord, id, none } => {
                let map = Arc::make_mut(&mut self.map);

                if let Some((_, tile)) = map.tiles.get(&coord) {
                    ctx.system.stop(tile);
                }

                if id == none {
                    map.tiles.remove_entry(&coord);
                } else {
                    let tile = ctx.system.actor_of_args::<Tile, (BasicActorRef, Id, TileCoord, Data)>(&Uuid::new_v4().to_string(), (ctx.myself().into(), id, coord, Data::default())).unwrap();

                    map.tiles.insert(coord, (id, tile));
                }
            }
            GameMsg::GetTile(coord) => {
                sender.inspect(|v| {
                    v.try_tell(self.map.tiles.get(&coord).cloned(), myself).unwrap();
                });
            }
            GameMsg::SendMsgToTile(coord, msg) => {
                if let Some((_, tile)) = self.map.tiles.get(&coord) {
                    tile.tell(msg, sender);
                }
            }
        }
    }
}

impl Game {
    pub fn new(
        map: Arc<Map>,
    ) -> Self {
        Self {
            tick_count: 0,

            map,
        }
    }

    fn inner_tick(&mut self, init_data: Arc<InitData>) {
        for (_, (_, tile)) in self.map.tiles.iter() {
            tile.tell(TileMsg::Tick { init_data: init_data.clone(), tick_count: self.tick_count }, None);
        }

        self.tick_count = self.tick_count.overflowing_add(1).0;
    }

    pub fn tick(&mut self, init_data: Arc<InitData>) {
        let start = Instant::now();
        self.inner_tick(init_data);
        let finish = Instant::now();

        let tick_time = finish - start;

        if tick_time >= MAX_ALLOWED_TICK_INTERVAL {
            log::warn!(
                "tick took longer than allowed maximum! tick_time: {:?}, maximum: {:?}",
                tick_time,
                MAX_ALLOWED_TICK_INTERVAL
            );
        }
    }
}
