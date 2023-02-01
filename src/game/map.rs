use std::fmt::Debug;
use std::{collections::HashMap, path::PathBuf, sync::Arc};

use riker::actor::ActorRef;

use crate::game::tile::TileMsg;
use crate::render::data::InstanceData;
use crate::util::id::Id;
use crate::util::init::InitData;

use super::tile::TileCoord;

pub const MAP_PATH: &str = "map";

const MAP_BUFFER_SIZE: usize = 128 * 1024;

#[derive(Clone, Debug)]
pub struct RenderContext {
    pub init_data: Arc<InitData>,
}

#[derive(Clone, Debug)]
pub struct MapRenderInfo {
    pub instances: HashMap<TileCoord, InstanceData>,
}

#[derive(Debug, Clone)]
pub struct Map {
    pub map_name: String,

    pub tiles: HashMap<TileCoord, (Id, ActorRef<TileMsg>)>,
}

impl Map {
    pub fn render_info(&self, RenderContext { init_data }: &RenderContext) -> MapRenderInfo {
        // TODO cache this
        let instances = self
            .tiles
            .iter()
            .map(|(a, b)| (a.clone(), b))
            .flat_map(|(pos, (id, _))| InstanceData::from_id(&id, pos, init_data.clone()))
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
        PathBuf::from(format!("{}/{}.bin", MAP_PATH, map_name))
    }

    /*
    pub fn unload(self) {
        drop(std::fs::create_dir(MAP_PATH));

        let path = Self::path(&self.map_name);

        let file = File::create(path).unwrap();

        let writer = BufWriter::with_capacity(MAP_BUFFER_SIZE, file);
        let mut encoder = Encoder::new(writer, 0).unwrap();

        serde_json::to_writer(&mut encoder, &self).unwrap();

        encoder.do_finish().unwrap();
    }

    pub fn load(map_name: String) -> Self {
        let path = Self::path(&map_name);

        let file = if let Ok(file) = File::open(path) {
            file
        } else {
            return Map::new_empty(map_name);
        };

        let reader = BufReader::with_capacity(MAP_BUFFER_SIZE, file);
        let decoder = Decoder::new(reader).unwrap();

        serde_json::from_reader(decoder).unwrap()
    }
     */
}
