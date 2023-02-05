use futures_executor::block_on;
use std::fmt::Debug;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::{collections::HashMap, path::PathBuf, sync::Arc};

use crate::game::inventory::{Inventory, InventoryRaw};
use crate::game::{Game, GameMsg};
use riker::actor::ActorRef;
use riker::actors::{ActorSystem, Context};
use riker_patterns::ask::ask;
use serde::{Deserialize, Serialize};
use zstd::{Decoder, Encoder};

use crate::game::tile::coord::TileCoord;
use crate::game::tile::entity::TileEntityMsg::{
    GetData, GetScript, GetTarget, SetData, SetScript, SetTarget,
};
use crate::game::tile::entity::{StateUnit, TileEntityMsg};
use crate::render::data::InstanceData;
use crate::resource::ResourceManager;
use crate::util::id::{Id, IdRaw, Interner};

pub const MAP_PATH: &str = "map";

const MAP_BUFFER_SIZE: usize = 128 * 1024;

#[derive(Clone, Debug)]
pub struct RenderContext {
    pub resource_man: Arc<ResourceManager>,
}

#[derive(Clone, Debug)]
pub struct MapRenderInfo {
    pub instances: HashMap<TileCoord, InstanceData>,
}

#[derive(Debug, Clone)]
pub struct Map {
    pub map_name: String,

    pub tiles: HashMap<TileCoord, (ActorRef<TileEntityMsg>, Id, StateUnit)>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TileData(
    IdRaw,
    StateUnit,
    InventoryRaw,
    Option<TileCoord>,
    Option<IdRaw>,
);

impl Map {
    pub fn render_info(&self, RenderContext { resource_man }: &RenderContext) -> MapRenderInfo {
        // TODO cache this
        let instances = self
            .tiles
            .iter()
            .map(|(a, b)| (*a, b))
            .flat_map(|(pos, (_, id, tile_state))| {
                InstanceData::from_id(id, pos, *tile_state, resource_man.clone())
            })
            .collect();

        MapRenderInfo { instances }
    }

    pub fn new_empty(map_name: String) -> Self {
        Self {
            map_name,
            tiles: HashMap::new(),
        }
    }

    pub fn path(map_name: &str) -> PathBuf {
        PathBuf::from(format!("{MAP_PATH}/{map_name}.bin"))
    }

    pub fn save(&self, sys: &ActorSystem, interner: &Interner) {
        drop(std::fs::create_dir(MAP_PATH));

        let path = Self::path(&self.map_name);

        let file = File::create(path).unwrap();

        let writer = BufWriter::with_capacity(MAP_BUFFER_SIZE, file);
        let mut encoder = Encoder::new(writer, 0).unwrap();

        let raw = self
            .tiles
            .iter()
            .map(|(coord, (tile, id, tile_state))| {
                let id = IdRaw::parse(interner.resolve(*id).unwrap());

                let target: Option<TileCoord> = block_on(ask(sys, tile, GetTarget));

                let script: Option<Id> = block_on(ask(sys, tile, GetScript));
                let script = script.map(|script| IdRaw::parse(interner.resolve(script).unwrap()));

                let data: Inventory = block_on(ask(sys, tile, GetData));
                let data = InventoryRaw::from_inventory(data, interner);

                (coord, TileData(id, *tile_state, data, target, script))
            })
            .collect::<Vec<_>>();

        serde_json::to_writer(&mut encoder, &raw).unwrap();

        encoder.do_finish().unwrap();
    }

    pub fn load(
        ctx: &Context<GameMsg>,
        resource_man: Arc<ResourceManager>,
        map_name: String,
    ) -> Self {
        let path = Self::path(&map_name);

        let file = if let Ok(file) = File::open(path) {
            file
        } else {
            return Map::new_empty(map_name);
        };

        let reader = BufReader::with_capacity(MAP_BUFFER_SIZE, file);
        let decoder = Decoder::new(reader).unwrap();

        let raw: Vec<(TileCoord, TileData)> = serde_json::from_reader(decoder).unwrap();

        let tiles = raw
            .into_iter()
            .flat_map(|(coord, TileData(id, tile_state, data, target, script))| {
                if let Some(id) = resource_man.interner.get(id.to_string()) {
                    let tile = Game::new_tile(ctx, coord, id, tile_state);

                    tile.send_msg(SetTarget(target), None);

                    if let Some(script) = script {
                        if let Some(script) = resource_man.interner.get(script.to_string()) {
                            tile.send_msg(SetScript(script, resource_man.clone()), None);
                        }
                    }

                    tile.send_msg(SetData(data.to_inventory(&resource_man.interner)), None);

                    Some((coord, (tile, id, tile_state)))
                } else {
                    None
                }
            })
            .collect::<HashMap<_, _>>();

        Self { map_name, tiles }
    }
}
